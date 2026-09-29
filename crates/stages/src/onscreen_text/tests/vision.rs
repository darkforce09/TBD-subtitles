use super::*;
use job_model::onscreen::{TextOccurrence, TextPresentation, TextProvenance};
use std::sync::atomic::AtomicUsize;

struct Temporary(PathBuf);

impl Temporary {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "tbd-visual-cancel-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn cached(
        &self,
        count: usize,
        cancel: Arc<AtomicBool>,
    ) -> (TextDocument, PreparedTranslations, ClaudeCli) {
        let mut backend = ClaudeCli::new("test", self.0.join("claude")).with_cancel(cancel);
        backend.program = self.0.join("must-not-run").to_string_lossy().into_owned();
        let crop = self.0.join("crop.png");
        let png = b"cached crop content; no decoder or external model runs";
        std::fs::write(&crop, png).unwrap();
        let mut hash = DefaultHasher::new();
        "independent cached fixture".hash(&mut hash);
        let mut vision_hash = hash.clone();
        (
            backend.name(),
            STANDARD.encode(png),
            format!("{SYSTEM} {IMAGE_INSTRUCTIONS}"),
        )
            .hash(&mut vision_hash);
        let answer = Answer {
            japanese: "作戦".into(),
            english: Some("Operation".into()),
            confidence: 0.97,
            reason: "Visible Japanese confirmed.".into(),
        };
        write_cache(
            &self
                .0
                .join(format!("claude-{:016x}.json", vision_hash.finish())),
            &answer,
        )
        .unwrap();
        let mut document = TextDocument::default();
        let mut pending = Vec::new();
        for index in 0..count {
            let id = format!("text-{index}");
            document.occurrences.push(TextOccurrence {
                id: id.clone(),
                start_s: index as f64,
                end_s: index as f64 + 1.0,
                japanese: "作戦".into(),
                english: None,
                confidence: 0.4,
                crops: vec![crop.clone()],
                frames: Vec::new(),
                provenance: TextProvenance {
                    backend: "local".into(),
                    ..TextProvenance::default()
                },
                presentation: TextPresentation::default(),
                warnings: Vec::new(),
                reviewed: false,
                rendered: None,
                source_fingerprint: None,
            });
            pending.push(Request {
                index,
                id,
                crop: Some(crop.clone()),
                prompt: "Verify the sign.".into(),
                hash: hash.clone(),
            });
        }
        let prepared = PreparedTranslations {
            answers: vec![answer; count],
            pending,
            references: Vec::new(),
            cache: self.0.clone(),
            schema: serde_json::json!({"type":"object"}),
        };
        (document, prepared, backend)
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn an_uncancelled_cached_answer_needs_no_cli_program() {
    let temporary = Temporary::new();
    let (mut document, mut prepared, backend) =
        temporary.cached(1, Arc::new(AtomicBool::new(false)));
    resolve(&mut document, &mut prepared, Some(&backend), &|_, _| {}).unwrap();
    assert_eq!(document.occurrences[0].confidence, 0.97);
    assert_eq!(
        document.occurrences[0].provenance.backend,
        "claude-cli/test"
    );
    assert!(document.occurrences[0].warnings.is_empty());
}

#[test]
fn already_cancelled_cached_answers_are_errors_without_applied_results() {
    let temporary = Temporary::new();
    let (mut document, mut prepared, backend) =
        temporary.cached(8, Arc::new(AtomicBool::new(true)));
    let original = document.clone();
    let progress = AtomicUsize::new(0);
    let result = resolve(&mut document, &mut prepared, Some(&backend), &|_, _| {
        progress.fetch_add(1, Ordering::Relaxed);
    });
    assert_eq!(result.unwrap_err().to_string(), "cancelled");
    assert_eq!(document, original);
    assert_eq!(progress.load(Ordering::Relaxed), 0);
}

#[test]
fn cancellation_after_a_cached_response_stops_the_phase_and_remaining_queue() {
    for count in [1, 64] {
        let temporary = Temporary::new();
        let cancel = Arc::new(AtomicBool::new(false));
        let (mut document, mut prepared, backend) = temporary.cached(count, cancel.clone());
        let original = document.clone();
        let completed = AtomicUsize::new(0);
        let result = resolve(&mut document, &mut prepared, Some(&backend), &|_, _| {
            completed.fetch_add(1, Ordering::Relaxed);
            cancel.store(true, Ordering::Release);
        });
        assert_eq!(result.unwrap_err().to_string(), "cancelled");
        assert_eq!(document, original);
        assert!(completed.load(Ordering::Relaxed) <= MAX_PARALLEL_CALLS);
    }
}
