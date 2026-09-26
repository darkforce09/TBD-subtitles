//! From the owner's settings to what a job runs with: the glossary read, the models and work
//! folders resolved.

use std::path::PathBuf;

use anyhow::Context;
use job_model::job::JobSettings;
use stages::adjudication::glossary;

use crate::settings::models::app_settings::{AppSettings, NO_GLOSSARY, ONE_PIECE};

/// The job settings `settings` ask for, with the glossary read.
pub(crate) fn job_settings(settings: &AppSettings) -> anyhow::Result<JobSettings> {
    let terms = match settings.glossary.as_str() {
        ONE_PIECE => glossary::one_piece(),
        NO_GLOSSARY => Vec::new(),
        path => {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("cannot read the glossary {path}"))?;
            glossary::parse(&text)
                .map_err(anyhow::Error::msg)
                .with_context(|| format!("glossary {path}"))?
        }
    };
    let mut job = JobSettings::with_glossary(terms);
    job.separator = settings.engines.separator;
    job.whisper = settings.engines.whisper;
    job.cut_score = settings.cut_score;
    job.llm_model = settings.language_model.model.clone();
    job.llm_processes = settings.language_model.processes.max(1);
    job.output_format = settings.output_format;
    Ok(job)
}

/// The models folder the settings name, else the default.
pub(crate) fn models_dir(settings: &AppSettings) -> anyhow::Result<PathBuf> {
    match &settings.models_dir {
        Some(dir) => Ok(dir.clone()),
        None => Ok(pipeline::models::default_dir()?),
    }
}

/// The work folder the settings name, else the default.
pub(crate) fn work_root(settings: &AppSettings) -> anyhow::Result<PathBuf> {
    match &settings.work_root {
        Some(dir) => Ok(dir.clone()),
        None => Ok(pipeline::work_dir::default_root()?),
    }
}

#[cfg(test)]
#[path = "tests/job_settings.rs"]
mod tests;
