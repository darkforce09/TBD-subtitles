//! `tbd-subtitles worker <stage> <job_dir>`: one GPU stage of one job, in its own process.

use std::path::Path;

use anyhow::bail;
use job_model::StageName;

/// Run `stage` over the job in `job_dir`.
pub(super) fn run(stage: StageName, job_dir: &Path) -> anyhow::Result<()> {
    bail!(
        "the {stage} stage is not built yet; nothing was run for {}",
        job_dir.display()
    )
}
