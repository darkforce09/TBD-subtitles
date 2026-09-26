//! `tbd-subtitles worker <step> <job_dir>`: one step of one job, in its own process, started by
//! the job runner.

use std::path::Path;

use job_model::StepName;
use pipeline::graph::{self, Binary, Placement};

/// Run `step` over the job in `job_dir`.
pub(super) fn run(step: StepName, job_dir: &Path) -> anyhow::Result<()> {
    pipeline::tasks::worker_main(step, job_dir, Binary::Main)?;
    Ok(())
}

/// Accept a step name unless the step runs in the ggml worker binary.
pub(super) fn parse_step(text: &str) -> Result<StepName, String> {
    let step: StepName = text.parse().map_err(|error| format!("{error}"))?;
    if graph::placement(step) == Placement::Worker(Binary::Ggml) {
        return Err(format!(
            "`{step}` runs in `tbd-subtitles-ggml`, the Whisper worker"
        ));
    }
    Ok(step)
}
