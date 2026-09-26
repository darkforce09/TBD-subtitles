//! Where the window finds its files, how it runs jobs and how its threads reach it: the real
//! paths and the pipeline when it runs, scratch paths and a stand-in runner in the tests, which
//! never touch the owner's home or start a worker.

use std::path::PathBuf;

use crate::core::background::Wake;
use crate::job_queue::services::job_runner::{self, RunJob};
use crate::settings::services::settings_file;

/// The paths, the job runner and the wake the application uses.
pub(crate) struct Environment {
    /// The settings file.
    pub(crate) settings_path: PathBuf,
    /// The queue kept across windows.
    pub(crate) queue_path: PathBuf,
    /// The machine-wide GPU lock.
    pub(crate) gpu_lock: PathBuf,
    /// The runtime folder holding the CUDA libraries.
    pub(crate) runtime_dir: PathBuf,
    /// The running binary's folder, where the Whisper worker sits.
    pub(crate) exe_dir: Option<PathBuf>,
    /// Asks for a frame; threads call it after sending news.
    pub(crate) wake: Wake,
    /// Runs one job: the pipeline, or a stand-in.
    pub(crate) run_job: RunJob,
    /// Whether to start the machine checks and the size measure when the window opens.
    pub(crate) background: bool,
}

impl Environment {
    /// The owner's real files and the pipeline, with `wake` from the window's context.
    pub(crate) fn real(wake: Wake) -> anyhow::Result<Environment> {
        let data = inference::model_store::app_data_dir()?;
        Ok(Environment {
            settings_path: settings_file::default_path()?,
            queue_path: data.join("queue.json"),
            gpu_lock: pipeline::work_dir::gpu_lock_path()?,
            runtime_dir: inference::model_store::runtime_dir()?,
            exe_dir: std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(PathBuf::from)),
            wake,
            run_job: job_runner::pipeline_runner(),
            background: true,
        })
    }

    /// Files under `root` only, a settings file naming models and work folders there, a runner
    /// that answers every job with `run_job`, and no background work.
    #[cfg(test)]
    pub(crate) fn scratch(root: &std::path::Path, run_job: RunJob) -> Environment {
        let settings = crate::settings::models::app_settings::AppSettings {
            models_dir: Some(root.join("models")),
            work_root: Some(root.join("work")),
            ..Default::default()
        };
        let settings_path = root.join("config").join("settings.toml");
        if let Err(error) = settings_file::save(&settings_path, &settings) {
            panic!("the scratch settings could not be written: {error}");
        }
        Environment {
            settings_path: root.join("config").join("settings.toml"),
            queue_path: root.join("data").join("queue.json"),
            gpu_lock: root.join("data").join("gpu.lock"),
            runtime_dir: root.join("runtime"),
            exe_dir: None,
            wake: crate::core::background::no_wake(),
            run_job,
            background: false,
        }
    }
}
