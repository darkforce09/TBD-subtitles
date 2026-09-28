//! Where the window finds its files, how it runs jobs and Fix It, and how its threads reach it:
//! the real paths and the pipeline when it runs, scratch paths and stand-in runners in the tests,
//! which never touch the owner's home or start a worker or `claude`.
//!
//! **Role:** hold the paths, the job runner, Fix It's runner, the wake and the log the application
//! uses.
//!
//! **Position:** built by `application::launch` (`real`) or by the tests (`scratch`); owned by
//! `TbdSubtitlesApp`.
//!
//! **Signals and state:** reads the data and config folders' locations; the scratch one writes
//! its settings file.
//!
//! **Invariants:** a test environment never names the owner's files, keeps a log of its own, and
//! its Fix It refuses to run unless the test gives it a stand-in.

use std::path::PathBuf;
use std::sync::Arc;

use crate::core::background::Wake;
use crate::core::log_buffer::LogBuffer;
use crate::core::logging;
use crate::job_queue::services::job_runner::{self, RunJob};
use crate::job_report::services::fix_it::{self, FixVideo};
use crate::settings::services::settings_file;

/// The paths, the job runner, the wake and the log the application uses.
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
    /// Runs Fix It on one video: the pipeline's, or a stand-in.
    pub(crate) fix_video: FixVideo,
    /// Whether to start the machine checks and the size measure when the window opens.
    pub(crate) background: bool,
    /// The lines logged in this process, which the log window shows.
    pub(crate) log: Arc<LogBuffer>,
    /// The log file the window writes, which the log window opens.
    pub(crate) log_file: Option<PathBuf>,
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
            fix_video: fix_it::pipeline_fix(),
            background: true,
            log: logging::console(),
            log_file: logging::window_log_path(),
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
            fix_video: std::sync::Arc::new(|_, _, _| {
                Err(pipeline::PipelineError::new(
                    "Fix It",
                    "no Fix It in this test",
                ))
            }),
            background: false,
            log: Arc::new(LogBuffer::new()),
            log_file: None,
        }
    }
}
