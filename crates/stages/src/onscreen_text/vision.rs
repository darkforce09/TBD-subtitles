//! Bounded concurrent image fallback for prepared visible-text translations.
//!
//! **Role:** resolve uncertain crops after callers release their local GPU model.
//! **Position:** private helper of the visual translation stage.
//! **Signals and state:** four workers, per-key cache locks and owned request metadata.
//! **Invariants:** each worker loads one crop at a time; duplicate requests share their cache;
//! cancellation stops admission and fails the phase even for cached answers. Claude's shared
//! process cap, tool isolation and logging remain in its backend.

use super::{Answer, PreparedTranslations, SYSTEM, TextResult, read_cache, valid, write_cache};
use base64::{Engine, engine::general_purpose::STANDARD};
use inference::llm::{LanguageModel, claude_cli::ClaudeCli};
use job_model::onscreen::TextDocument;
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const MAX_PARALLEL_CALLS: usize = 4;
const IMAGE_INSTRUCTIONS: &str = "Inspect the actual image independently before using the supplied OCR reading. OCR may be wrong or describe non-text artwork. Correct the Japanese reading only from visible image evidence. If the image contains no readable writing, return an empty Japanese string, null English, and a reason; never complete missing or obscured characters from context.";

pub(super) struct Request {
    pub index: usize,
    pub id: String,
    pub crop: Option<PathBuf>,
    pub prompt: String,
    pub hash: DefaultHasher,
}

enum Outcome {
    Answer(Answer),
    Warning(String),
}

pub(super) fn resolve(
    document: &mut TextDocument,
    prepared: &mut PreparedTranslations,
    backend: Option<&ClaudeCli>,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> TextResult<()> {
    if let Some(backend) = backend {
        check_cancel(backend)?;
    }
    let pending = std::mem::take(&mut prepared.pending);
    let count = pending.len();
    let completed = Mutex::new(document.occurrences.len());
    let total = document.occurrences.len() + count;
    let Some(backend) = backend else {
        for request in pending {
            document.occurrences[request.index]
                .warnings
                .push("Claude fallback is unavailable. Check Claude sign-in in Settings.".into());
        }
        return Ok(());
    };
    let queue = Mutex::new(pending.into_iter());
    let locks = Mutex::new(HashMap::<u64, Arc<Mutex<()>>>::new());
    let stopped = AtomicBool::new(false);
    let span = tracing::Span::current();
    let dispatch = tracing::dispatcher::get_default(Clone::clone);
    let results = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..count.min(MAX_PARALLEL_CALLS) {
            let mut worker = copy_backend(backend);
            let queue = &queue;
            let locks = &locks;
            let stopped = &stopped;
            let completed = &completed;
            let cache = &prepared.cache;
            let schema = &prepared.schema;
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
                        let Some(request) = next else { break };
                        let index = request.index;
                        let result = resolve_one(request, &mut worker, cache, schema, locks);
                        if result.is_err() {
                            stopped.store(true, Ordering::Release);
                        }
                        results.push((index, result));
                        let mut done = completed
                            .lock()
                            .map_err(|_| "visual progress lock failed")?;
                        *done += 1;
                        progress(*done, total);
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
    for (index, outcome) in results.into_iter().flatten() {
        let item = &mut document.occurrences[index];
        match outcome? {
            Outcome::Answer(answer) => {
                item.confidence = answer.confidence;
                item.provenance.backend = backend.name();
                prepared.answers[index] = answer;
            }
            Outcome::Warning(warning) => item.warnings.push(warning),
        }
    }
    check_cancel(backend)
}

fn check_cancel(backend: &ClaudeCli) -> TextResult<()> {
    if backend
        .cancel
        .as_ref()
        .is_some_and(|cancel| cancel.load(Ordering::Acquire))
    {
        Err(inference::llm::LlmError("cancelled".into()).into())
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
    request: Request,
    backend: &mut ClaudeCli,
    cache: &Path,
    schema: &serde_json::Value,
    locks: &Mutex<HashMap<u64, Arc<Mutex<()>>>>,
) -> TextResult<Outcome> {
    check_cancel(backend)?;
    let crop = request.crop.ok_or("uncertain occurrence has no crop")?;
    let encoded = STANDARD.encode(std::fs::read(crop)?);
    let system = format!("{SYSTEM} {IMAGE_INSTRUCTIONS}");
    let mut hash = request.hash;
    (backend.name(), &encoded, &system).hash(&mut hash);
    let key = hash.finish();
    let path = cache.join(format!("claude-{key:016x}.json"));
    let lock = locks
        .lock()
        .map_err(|_| "visual cache map lock failed")?
        .entry(key)
        .or_default()
        .clone();
    let _cache_guard = lock.lock().map_err(|_| "visual cache key lock failed")?;
    check_cancel(backend)?;
    if let Some(answer) = read_cache(&path, None) {
        check_cancel(backend)?;
        return Ok(Outcome::Answer(answer));
    }
    let _purpose = inference::llm::purpose(format!("uncertain on-screen crop {}", request.id));
    let result = backend.complete_image_json(&system, &request.prompt, schema, &encoded);
    check_cancel(backend)?;
    match result {
        Ok(completion) => match serde_json::from_value::<Answer>(completion.json) {
            Ok(answer) if valid(&answer) => {
                write_cache(&path, &answer)?;
                Ok(Outcome::Answer(answer))
            }
            _ => Ok(Outcome::Warning(
                "Claude returned an invalid visual response.".into(),
            )),
        },
        Err(error) if error.0 == "cancelled" => Err(error.into()),
        Err(error) => {
            tracing::warn!(occurrence=%request.id,error=%error,"Visual Claude fallback unavailable");
            Ok(Outcome::Warning(format!(
                "Claude fallback unavailable: {error}"
            )))
        }
    }
}

#[cfg(test)]
#[path = "tests/vision.rs"]
mod tests;
