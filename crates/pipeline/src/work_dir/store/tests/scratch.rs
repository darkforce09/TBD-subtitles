//! A job database in a folder of its own for the crate's tests, removed when the test ends.

use std::path::PathBuf;
use std::sync::Arc;

use job_model::job::{JobRecord, JobSettings};

use super::JobStore;
use crate::work_dir::WorkDir;

/// A job folder with its database open.
pub(crate) struct Scratch {
    store: Option<Arc<JobStore>>,
    pub(crate) dir: PathBuf,
}

impl Scratch {
    /// A fresh job folder named for `name`, with its database open.
    pub(crate) fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!(
            "tbd-scratch-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let store = JobStore::open(&WorkDir::new(dir.clone())).expect("the store opens");
        Scratch {
            store: Some(store),
            dir,
        }
    }

    pub(crate) fn store(&self) -> &Arc<JobStore> {
        self.store.as_ref().expect("open")
    }

    pub(crate) fn work(&self) -> &WorkDir {
        self.store().work()
    }

    /// Close the database, then open it again, as a later run does.
    pub(crate) fn reopen(&mut self) {
        drop(self.store.take());
        self.store = Some(JobStore::open(&WorkDir::new(self.dir.clone())).expect("reopens"));
    }

    /// Write `bytes` to `relative` in the job folder, creating its folder.
    pub(crate) fn file(&self, relative: &str, bytes: &[u8]) -> PathBuf {
        let path = self.dir.join(relative);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("the folder");
        std::fs::write(&path, bytes).expect("the file");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(self.store.take());
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A job record for a video in `dir`.
pub(crate) fn job_record(dir: &std::path::Path) -> JobRecord {
    JobRecord {
        video: dir.join("episode.mp4").to_string_lossy().into_owned(),
        video_size: 10,
        video_modified_s: 5,
        settings: JobSettings::with_glossary(vec!["Luffy".into()]),
        models_dir: None,
        corrections: None,
    }
}
