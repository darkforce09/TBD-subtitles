use super::super::tests::{Temporary, cache_answer, claude_backend, claude_files, region};
use super::keyframe_requests::Region;
use super::*;
use serde_json::json;
use std::sync::atomic::AtomicUsize;

/// A request for `regions` crops of the still `name`, with its image files written.
fn request(root: &Path, name: &str, regions: usize) -> Request {
    let keyframe = root.join(format!("{name}.png"));
    std::fs::write(&keyframe, format!("still {name}")).unwrap();
    Request {
        keyframe,
        regions: (0..regions)
            .map(|position| {
                let crop = root.join(format!("{name}-crop-{position}.png"));
                std::fs::write(&crop, format!("crop {position} of {name}")).unwrap();
                Region {
                    index: position,
                    id: format!("text-{position}"),
                    crop,
                    bbox: [0.1, 0.1, 0.3, 0.2],
                    ocr_reading: "作戦".into(),
                    reading_confidence: 0.4,
                }
            })
            .collect(),
        prompt: format!("Read the signs on {name}."),
        retry_generation: 0,
    }
}

fn answer(regions: usize) -> KeyframeAnswer {
    KeyframeAnswer {
        regions: (0..regions)
            .map(|position| {
                let id = format!("r{}", position + 1);
                region(&id, "作戦", Some("Operation"), 0.97, [0.1, 0.1, 0.3, 0.2])
            })
            .collect(),
        other_text: Vec::new(),
    }
}

fn completion(json: serde_json::Value) -> Completion {
    Completion {
        json,
        input_tokens: 1,
        output_tokens: 1,
        cost_usd: None,
    }
}

fn never(
    _: &mut ClaudeCli,
    _: &str,
    _: &str,
    _: &serde_json::Value,
    _: &[String],
) -> Result<Completion, LlmError> {
    panic!("Claude must not be asked")
}

#[test]
fn an_uncancelled_cached_answer_needs_no_cli_program() {
    let temporary = Temporary::new();
    let backend = claude_backend(&temporary.0, Arc::new(AtomicBool::new(false)));
    let requests = [request(&temporary.0, "still", 2)];
    cache_answer(&temporary.0, &requests[0], &backend, &answer(2));
    let outcomes = resolve(&requests, &backend, &never, 4, &temporary.0, &|_| {}).unwrap();
    assert!(matches!(&outcomes[..], [Outcome::Answer(found)] if *found == answer(2)));
}

#[test]
fn no_requests_need_no_workers_but_still_honour_cancellation() {
    let temporary = Temporary::new();
    let backend = claude_backend(&temporary.0, Arc::new(AtomicBool::new(false)));
    assert!(
        resolve(&[], &backend, &never, 4, &temporary.0, &|_| {})
            .unwrap()
            .is_empty()
    );
    let cancelled = claude_backend(&temporary.0, Arc::new(AtomicBool::new(true)));
    let error = resolve(&[], &cancelled, &never, 4, &temporary.0, &|_| {}).unwrap_err();
    assert_eq!(error.to_string(), "cancelled");
}

#[test]
fn already_cancelled_cached_requests_are_errors_without_progress() {
    let temporary = Temporary::new();
    let backend = claude_backend(&temporary.0, Arc::new(AtomicBool::new(true)));
    let requests = (0..8)
        .map(|_| request(&temporary.0, "still", 1))
        .collect::<Vec<_>>();
    cache_answer(&temporary.0, &requests[0], &backend, &answer(1));
    let progress = AtomicUsize::new(0);
    let result = resolve(&requests, &backend, &never, 4, &temporary.0, &|_| {
        progress.fetch_add(1, Ordering::Relaxed);
    });
    assert_eq!(result.unwrap_err().to_string(), "cancelled");
    assert_eq!(progress.load(Ordering::Relaxed), 0);
}

#[test]
fn cancellation_after_a_cached_response_stops_the_phase_and_remaining_queue() {
    for count in [1, 64] {
        let temporary = Temporary::new();
        let cancel = Arc::new(AtomicBool::new(false));
        let backend = claude_backend(&temporary.0, cancel.clone());
        let requests = (0..count)
            .map(|_| request(&temporary.0, "still", 1))
            .collect::<Vec<_>>();
        cache_answer(&temporary.0, &requests[0], &backend, &answer(1));
        let completed = AtomicUsize::new(0);
        let result = resolve(&requests, &backend, &never, 4, &temporary.0, &|_| {
            completed.fetch_add(1, Ordering::Relaxed);
            cancel.store(true, Ordering::Release);
        });
        assert_eq!(result.unwrap_err().to_string(), "cancelled");
        assert!(completed.load(Ordering::Relaxed) <= 4);
    }
}

#[test]
fn identical_requests_share_one_call_and_its_cache() {
    let temporary = Temporary::new();
    let backend = claude_backend(&temporary.0, Arc::new(AtomicBool::new(false)));
    let requests = [
        request(&temporary.0, "still", 2),
        request(&temporary.0, "still", 2),
    ];
    let asked = AtomicUsize::new(0);
    let ask = |_: &mut ClaudeCli,
               _: &str,
               _: &str,
               _: &serde_json::Value,
               _: &[String]|
     -> Result<Completion, LlmError> {
        asked.fetch_add(1, Ordering::SeqCst);
        Ok(completion(serde_json::to_value(answer(2)).unwrap()))
    };
    let finished = Mutex::new(Vec::new());
    let outcomes = resolve(&requests, &backend, &ask, 2, &temporary.0, &|done| {
        finished.lock().unwrap().push(done)
    })
    .unwrap();
    assert_eq!(asked.load(Ordering::SeqCst), 1);
    assert!(
        outcomes
            .iter()
            .all(|outcome| matches!(outcome, Outcome::Answer(found) if *found == answer(2)))
    );
    assert_eq!(finished.into_inner().unwrap(), [1, 2]);
    resolve(&requests, &backend, &ask, 2, &temporary.0, &|_| {}).unwrap();
    assert_eq!(asked.load(Ordering::SeqCst), 1);
    assert_eq!(claude_files(&temporary.0), 1);
}

#[test]
fn claude_sees_the_keyframe_first_then_each_crop_in_region_order() {
    let temporary = Temporary::new();
    let backend = claude_backend(&temporary.0, Arc::new(AtomicBool::new(false)));
    let requests = [request(&temporary.0, "still", 2)];
    let sent = Mutex::new(Vec::new());
    let ask = |_: &mut ClaudeCli,
               system: &str,
               user: &str,
               schema: &serde_json::Value,
               pngs: &[String]|
     -> Result<Completion, LlmError> {
        let call = (
            system.to_owned(),
            user.to_owned(),
            schema.clone(),
            pngs.to_vec(),
        );
        sent.lock().unwrap().push(call);
        Ok(completion(serde_json::to_value(answer(2)).unwrap()))
    };
    resolve(&requests, &backend, &ask, 4, &temporary.0, &|_| {}).unwrap();
    let sent = sent.into_inner().unwrap();
    let [(system, user, schema, pngs)] = &sent[..] else {
        panic!("one call expected, got {}", sent.len())
    };
    assert_eq!(*system, keyframe_requests::system());
    assert!(system.starts_with(super::super::SYSTEM));
    assert!(system.contains("other_text"));
    assert_eq!(*user, requests[0].prompt);
    assert_eq!(*schema, keyframe_requests::schema());
    let decoded = pngs
        .iter()
        .map(|png| String::from_utf8(STANDARD.decode(png).unwrap()).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        decoded,
        ["still still", "crop 0 of still", "crop 1 of still"]
    );
}

#[test]
fn invalid_answers_warn_and_are_never_cached() {
    let mut missing = answer(2);
    missing.regions.pop();
    let mut inverted = answer(2);
    inverted.regions[1].bbox = [0.3, 0.1, 0.1, 0.2];
    let mut null = answer(2);
    null.regions[0].english = Some("null".into());
    let mut extra = serde_json::to_value(answer(2)).unwrap();
    extra["regions"][0]["note"] = json!("more");
    for json in [
        serde_json::to_value(missing).unwrap(),
        serde_json::to_value(inverted).unwrap(),
        serde_json::to_value(null).unwrap(),
        extra,
        json!({"regions": "r1", "other_text": []}),
    ] {
        let temporary = Temporary::new();
        let backend = claude_backend(&temporary.0, Arc::new(AtomicBool::new(false)));
        let requests = [request(&temporary.0, "still", 2)];
        let ask = |_: &mut ClaudeCli,
                   _: &str,
                   _: &str,
                   _: &serde_json::Value,
                   _: &[String]|
         -> Result<Completion, LlmError> { Ok(completion(json.clone())) };
        let outcomes = resolve(&requests, &backend, &ask, 4, &temporary.0, &|_| {}).unwrap();
        assert!(
            matches!(&outcomes[..], [Outcome::Warning(warning)] if warning == INVALID_RESPONSE),
            "{json}"
        );
        assert_eq!(claude_files(&temporary.0), 0);
    }
}

#[test]
fn backend_errors_warn_but_cancellation_fails_the_phase() {
    let temporary = Temporary::new();
    let backend = claude_backend(&temporary.0, Arc::new(AtomicBool::new(false)));
    let requests = [request(&temporary.0, "still", 1)];
    let failing = |message: &'static str| {
        move |_: &mut ClaudeCli,
              _: &str,
              _: &str,
              _: &serde_json::Value,
              _: &[String]|
              -> Result<Completion, LlmError> { Err(LlmError(message.into())) }
    };
    let outcomes = resolve(
        &requests,
        &backend,
        &failing("rate limited"),
        4,
        &temporary.0,
        &|_| {},
    )
    .unwrap();
    assert!(matches!(
        &outcomes[..],
        [Outcome::Warning(warning)] if warning == "Claude fallback unavailable: rate limited"
    ));
    let error = resolve(
        &requests,
        &backend,
        &failing("cancelled"),
        4,
        &temporary.0,
        &|_| {},
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "cancelled");
    assert_eq!(claude_files(&temporary.0), 0);
}

#[test]
fn the_cache_key_follows_prompts_generation_backend_and_every_image_byte() {
    let temporary = Temporary::new();
    let key = |request: &Request, backend: &str| {
        let images = Images::load(request).unwrap();
        let system = keyframe_requests::system();
        cache_key(
            request,
            backend,
            &system,
            &keyframe_requests::schema(),
            &images,
        )
    };
    let base = request(&temporary.0, "still", 2);
    let first = key(&base, "claude-cli/test");
    assert_eq!(first, key(&base, "claude-cli/test"));
    assert_ne!(first, key(&base, "claude-cli/other"));
    let retried = Request {
        retry_generation: 1,
        ..request(&temporary.0, "still", 2)
    };
    assert_ne!(first, key(&retried, "claude-cli/test"));
    let reworded = Request {
        prompt: "Other dialogue nearby.".into(),
        ..request(&temporary.0, "still", 2)
    };
    assert_ne!(first, key(&reworded, "claude-cli/test"));
    std::fs::write(&base.regions[1].crop, "a changed crop").unwrap();
    assert_ne!(first, key(&base, "claude-cli/test"));
    let base = request(&temporary.0, "still", 2);
    assert_eq!(first, key(&base, "claude-cli/test"));
    std::fs::write(&base.keyframe, "a changed still").unwrap();
    assert_ne!(first, key(&base, "claude-cli/test"));
}

#[test]
fn a_missing_image_is_an_explicit_error() {
    let temporary = Temporary::new();
    let backend = claude_backend(&temporary.0, Arc::new(AtomicBool::new(false)));
    let requests = [request(&temporary.0, "still", 1)];
    std::fs::remove_file(&requests[0].regions[0].crop).unwrap();
    let error = resolve(&requests, &backend, &never, 4, &temporary.0, &|_| {}).unwrap_err();
    assert!(error.to_string().contains("still-crop-0.png"), "{error}");
}
