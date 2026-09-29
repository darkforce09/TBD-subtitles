//! Bounded concurrent Claude requests for whole keyframe stills.
//!
//! **Role:** send each keyframe request with its whole-frame still and region crops, and return
//! one checked outcome per request.
//! **Position:** private helper of the visual translation stage; runs before any local model
//! opens.
//! **Signals and state:** up to `parallel_calls` workers, per-key cache locks, a stop flag and
//! the image bytes of the one request each worker holds.
//! **Invariants:** duplicate requests share their cache; only checked answers are cached;
//! cancellation stops admission and fails the phase even for cached answers. Claude's shared
//! process cap, tool isolation and logging remain in its backend.

use super::keyframe_requests::{self, INVALID_RESPONSE, KeyframeAnswer, Outcome, Request};
use super::{TRANSLATION_CACHE_REVISION, TextResult};
use base64::{Engine, engine::general_purpose::STANDARD};
use inference::llm::{Completion, LanguageModel, LlmError, claude_cli::ClaudeCli};
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Sends one request: the backend, system prompt, user prompt, schema and base64 PNGs in order.
pub(super) type Ask<'a> =
    dyn Fn(&mut ClaudeCli, &str, &str, &serde_json::Value, &[String]) -> Answered + Sync + 'a;

/// What Claude answered, or why it could not.
type Answered = Result<Completion, LlmError>;

type CacheLocks = Mutex<HashMap<u64, Arc<Mutex<()>>>>;

/// A request's images: the keyframe still, then each region's crop in region order.
pub(super) struct Images {
    keyframe: Vec<u8>,
    crops: Vec<Vec<u8>>,
}

impl Images {
    pub(super) fn load(request: &Request) -> TextResult<Self> {
        Ok(Self {
            keyframe: read_image(&request.keyframe)?,
            crops: request
                .regions
                .iter()
                .map(|region| read_image(&region.crop))
                .collect::<TextResult<_>>()?,
        })
    }

    fn encoded(self) -> Vec<String> {
        std::iter::once(self.keyframe)
            .chain(self.crops)
            .map(|bytes| STANDARD.encode(bytes))
            .collect()
    }
}

/// The cache identity of a request: prompts, backend, schema, retry generation and every image
/// byte in order.
pub(super) fn cache_key(
    request: &Request,
    backend: &str,
    system: &str,
    schema: &serde_json::Value,
    images: &Images,
) -> u64 {
    let mut hash = DefaultHasher::new();
    (
        TRANSLATION_CACHE_REVISION,
        system,
        &request.prompt,
        backend,
        schema.to_string(),
        request.retry_generation,
    )
        .hash(&mut hash);
    images.keyframe.hash(&mut hash);
    images.crops.hash(&mut hash);
    hash.finish()
}

/// One outcome per request, in request order. `progress` receives the number of finished
/// requests.
pub(super) fn resolve(
    requests: &[Request],
    backend: &ClaudeCli,
    ask: &Ask<'_>,
    parallel_calls: usize,
    cache: &Path,
    progress: &(dyn Fn(usize) + Sync),
) -> TextResult<Vec<Outcome>> {
    check_cancel(backend)?;
    let system = keyframe_requests::system();
    let schema = keyframe_requests::schema();
    let queue = Mutex::new(requests.iter().enumerate());
    let locks = CacheLocks::default();
    let stopped = AtomicBool::new(false);
    let finished = Mutex::new(0_usize);
    let span = tracing::Span::current();
    let dispatch = tracing::dispatcher::get_default(Clone::clone);
    let results = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..parallel_calls.max(1).min(requests.len()) {
            let mut worker = copy_backend(backend);
            let queue = &queue;
            let locks = &locks;
            let stopped = &stopped;
            let finished = &finished;
            let system = system.as_str();
            let schema = &schema;
            let span = span.clone();
            let dispatch = dispatch.clone();
            handles.push(scope.spawn(move || {
                tracing::dispatcher::with_default(&dispatch, || {
                    let _span = span.enter();
                    let mut results = Vec::new();
                    while !stopped.load(Ordering::Acquire) {
                        if let Err(error) = check_cancel(&worker) {
                            stopped.store(true, Ordering::Release);
                            return Err(error);
                        }
                        let next = queue
                            .lock()
                            .map_err(|_| "visual request queue lock failed")?
                            .next();
                        let Some((position, request)) = next else {
                            break;
                        };
                        let result =
                            resolve_one(request, &mut worker, ask, system, schema, cache, locks);
                        if result.is_err() {
                            stopped.store(true, Ordering::Release);
                        }
                        results.push((position, result));
                        let mut done =
                            finished.lock().map_err(|_| "visual progress lock failed")?;
                        *done += 1;
                        progress(*done);
                    }
                    Ok::<_, inference::ocr::OcrError>(results)
                })
            }));
        }
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|_| "visual translation worker panicked")?
            })
            .collect::<TextResult<Vec<_>>>()
    })?;
    check_cancel(backend)?;
    let mut outcomes = requests.iter().map(|_| None).collect::<Vec<_>>();
    for (position, result) in results.into_iter().flatten() {
        outcomes[position] = Some(result?);
    }
    outcomes
        .into_iter()
        .map(|outcome| {
            outcome.ok_or_else(|| "a keyframe request finished without an outcome".into())
        })
        .collect()
}

fn check_cancel(backend: &ClaudeCli) -> TextResult<()> {
    if backend
        .cancel
        .as_ref()
        .is_some_and(|cancel| cancel.load(Ordering::Acquire))
    {
        Err(LlmError("cancelled".into()).into())
    } else {
        Ok(())
    }
}

fn copy_backend(backend: &ClaudeCli) -> ClaudeCli {
    ClaudeCli {
        program: backend.program.clone(),
        model: backend.model.clone(),
        timeout: backend.timeout,
        cwd: backend.cwd.clone(),
        cancel: backend.cancel.clone(),
    }
}

fn resolve_one(
    request: &Request,
    backend: &mut ClaudeCli,
    ask: &Ask<'_>,
    system: &str,
    schema: &serde_json::Value,
    cache: &Path,
    locks: &CacheLocks,
) -> TextResult<Outcome> {
    check_cancel(backend)?;
    let images = Images::load(request)?;
    let key = cache_key(request, &backend.name(), system, schema, &images);
    let path = cache.join(format!("claude-{key:016x}.json"));
    let lock = locks
        .lock()
        .map_err(|_| "visual cache map lock failed")?
        .entry(key)
        .or_default()
        .clone();
    let _cache_guard = lock.lock().map_err(|_| "visual cache key lock failed")?;
    check_cancel(backend)?;
    if let Some(answer) = keyframe_requests::read_cache(&path, request) {
        check_cancel(backend)?;
        return Ok(Outcome::Answer(answer));
    }
    let still = request
        .keyframe
        .file_name()
        .unwrap_or(request.keyframe.as_os_str())
        .to_string_lossy();
    let _purpose = inference::llm::purpose(format!(
        "keyframe {still} ({} regions)",
        request.regions.len()
    ));
    let result = ask(backend, system, &request.prompt, schema, &images.encoded());
    check_cancel(backend)?;
    match result {
        Ok(completion) => match serde_json::from_value::<KeyframeAnswer>(completion.json)
            .ok()
            .and_then(|answer| keyframe_requests::checked(request, answer))
        {
            Some(answer) => {
                keyframe_requests::write_cache(&path, &answer)?;
                Ok(Outcome::Answer(answer))
            }
            None => {
                tracing::warn!(keyframe = %still, "Claude returned an invalid keyframe answer");
                Ok(Outcome::Warning(INVALID_RESPONSE.into()))
            }
        },
        Err(error) if error.0 == "cancelled" => Err(error.into()),
        Err(error) => {
            tracing::warn!(keyframe = %still, error = %error, "Visual Claude request unavailable");
            Ok(Outcome::Warning(format!(
                "Claude fallback unavailable: {error}"
            )))
        }
    }
}

fn read_image(path: &Path) -> TextResult<Vec<u8>> {
    std::fs::read(path)
        .map_err(|error| format!("read visual image {}: {error}", path.display()).into())
}

#[cfg(test)]
#[path = "tests/vision.rs"]
mod tests;
