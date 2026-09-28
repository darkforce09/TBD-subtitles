//! Fix It on a thread of its own: the pipeline's `fix_video` run for one video, its progress and
//! its outcome sent back to the window, and Stop.
//!
//! **Role:** start one Fix It run, forward each progress step, keep the latest, and hand over the
//! outcome once the run ends.
//!
//! **Position:** started and polled by the application's Fix It actions; the run is
//! `pipeline::fix_it::fix_video`, or a stand-in in the tests.
//!
//! **Signals and state:** one thread per run and one channel back; the run's `CancelToken`.
//! Each new stage and the end are logged under the `fix_it` target, in spans naming the video and
//! the step `fix_it`, which its model calls inherit.
//!
//! **Invariants:** the thread lives until its run ends, so the `claude` processes it starts die
//! with it and never outlive it; after Stop the run ends as cancelled and changes nothing; the
//! window wakes after each message.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

use pipeline::fix_it::{FixOptions, FixOutcome, FixProgress, FixStage};
use pipeline::{CancelToken, PipelineError};

use crate::core::background::Wake;

/// How Fix It is run: the pipeline's `fix_video`, or a stand-in in the tests.
pub(crate) type FixVideo = Arc<
    dyn Fn(&Path, &FixOptions, &(dyn Fn(FixProgress) + Sync)) -> Result<FixOutcome, PipelineError>
        + Send
        + Sync,
>;

/// The pipeline's own Fix It.
pub(crate) fn pipeline_fix() -> FixVideo {
    Arc::new(|video, options, progress| pipeline::fix_it::fix_video(video, options, progress))
}

/// What the thread sends back.
#[derive(Debug)]
enum FixEvent {
    Progress(FixProgress),
    Ended(Box<Result<FixOutcome, PipelineError>>),
}

/// One Fix It run under way.
pub(crate) struct Fixing {
    pub(crate) video: PathBuf,
    /// The model as the window names it, such as "Claude Opus".
    pub(crate) model: String,
    /// The latest progress; none before the first.
    pub(crate) progress: Option<FixProgress>,
    /// Stop was pressed.
    pub(crate) stopping: bool,
    events: Receiver<FixEvent>,
    cancel: CancelToken,
}

/// Start Fix It on `video` with `options` through `run`; `model` is the model's name as the window
/// shows it.
pub(crate) fn start(
    run: FixVideo,
    video: PathBuf,
    options: FixOptions,
    model: String,
    wake: Wake,
) -> Fixing {
    let (sender, events) = channel();
    let cancel = options.cancel.clone();
    let path = video.clone();
    std::thread::spawn(move || {
        let name = path
            .file_stem()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        // Its lines and model calls show under the video, as the step `fix_it`.
        let job = tracing::info_span!("job", video = name.as_str());
        let _in_job = job.enter();
        let fix = tracing::info_span!("step", step = "fix_it");
        let _in_fix = fix.enter();
        tracing::info!(target: "fix_it", "Fix It started");
        let stage = Mutex::new(None);
        let progress = |step: FixProgress| {
            log_progress(&stage, step);
            let _ = sender.send(FixEvent::Progress(step));
            wake();
        };
        let outcome = run(&path, &options, &progress);
        log_end(&outcome);
        let _ = sender.send(FixEvent::Ended(Box::new(outcome)));
        wake();
    });
    Fixing {
        video,
        model,
        progress: None,
        stopping: false,
        events,
        cancel,
    }
}

/// Log a new stage at info, and each call within it at debug.
fn log_progress(stage: &Mutex<Option<FixStage>>, step: FixProgress) {
    let mut last = stage
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if *last != Some(step.stage) {
        *last = Some(step.stage);
        tracing::info!(
            target: "fix_it",
            "Pass {}: {}, {} calls",
            step.stage.pass(),
            stage_words(step.stage),
            step.total
        );
    } else {
        tracing::debug!(target: "fix_it", "{} of {} calls done", step.done, step.total);
    }
}

/// What Fix It does in `stage`, in plain words.
fn stage_words(stage: FixStage) -> String {
    match stage {
        FixStage::Reading => "reading the whole video for context".to_string(),
        FixStage::Fixing(family) => format!("fixing {}", family.describe()),
        FixStage::Checking => "checking each change".to_string(),
        FixStage::Saving => "saving the kept changes".to_string(),
    }
}

fn log_end(outcome: &Result<FixOutcome, PipelineError>) {
    match outcome {
        Ok(done) => tracing::info!(
            target: "fix_it",
            "Fix It finished: {} lines changed, {} of yours kept",
            done.changed.len(),
            done.kept_yours.len()
        ),
        Err(error) => tracing::error!(target: "fix_it", "Fix It stopped: {error}"),
    }
}

impl Fixing {
    /// Stop the run: no call starts and the running `claude` processes are killed.
    pub(crate) fn stop(&mut self) {
        self.stopping = true;
        self.cancel.cancel();
    }

    /// Fold in what the thread sent; the outcome once the run ended. A thread gone without an
    /// outcome ended in a failure.
    pub(crate) fn poll(&mut self) -> Option<Result<FixOutcome, PipelineError>> {
        loop {
            match self.events.try_recv() {
                Ok(FixEvent::Progress(step)) => self.progress = Some(step),
                Ok(FixEvent::Ended(outcome)) => return Some(*outcome),
                Err(std::sync::mpsc::TryRecvError::Empty) => return None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    return Some(Err(PipelineError::new(
                        "Fix It",
                        "the run ended unexpectedly",
                    )));
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/fix_it.rs"]
mod tests;
