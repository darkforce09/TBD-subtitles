//! Which composed patches cover each frame, and a bounded cache of their converted samples.
//!
//! **Role:** turn the replacement document and the writing's per-frame shifts into a
//! frame-by-frame schedule of patches, and load each patch file once into frame samples while
//! it is in use.
//! **Position:** inside `localize::render` and the read-back check, between the replacement
//! document, its `Motion` and `blend`.
//! **Signals and state:** the schedule holds the patches not yet started, ordered by first
//! frame, and the active ones in document order; the cache holds converted patches by file with
//! a use stamp and their total size.
//! **Invariants:** only baked occurrences' plates with a patch are scheduled; a plate whose
//! frames take several shifts is scheduled as one entry per run of frames, each with the patch
//! lettered at that run's shift, and a frame no row covers takes the plate's own patch; frames
//! advance strictly; an entry is active exactly on `first_frame..=last_frame`; overlapping
//! patches apply in document order; the cache stays within its byte budget except for a single
//! patch larger than the budget, and a patch file leaves it once the last entry using it ends.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use job_model::onscreen::{PixelRect, Plate, ReplacedText, ReplacementDocument};

use super::LocalizeResult;
use super::blend::{FramePatch, convert};
use super::colour::Conversion;
use super::motion::Motion;

/// One patch and the frames it covers.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledPatch {
    /// The entry's position in the document: occurrences, then plates, then runs, in order.
    pub order: usize,
    /// The patch file's number, shared by every entry that blends the same file.
    pub file: usize,
    pub first_frame: u64,
    pub last_frame: u64,
    pub rect: PixelRect,
    /// The RGBA patch file, relative to the job directory.
    pub path: PathBuf,
}

/// The patches active at each frame, advanced one frame at a time.
#[derive(Debug, Default)]
pub struct Schedule {
    pending: VecDeque<ScheduledPatch>,
    active: Vec<ScheduledPatch>,
    /// The last frame each patch file is blended on.
    last_use: HashMap<usize, u64>,
    next_frame: u64,
}

impl Schedule {
    /// Every patch of the document's baked occurrences, each frame's taken by its shift in
    /// `motion`.
    pub fn new(document: &ReplacementDocument, motion: &Motion) -> LocalizeResult<Schedule> {
        let mut pending = Vec::new();
        let mut files: HashMap<PathBuf, usize> = HashMap::new();
        for text in document.baked() {
            for (index, plate) in text.plates.iter().enumerate() {
                let Some(own) = plate.patch.as_ref() else {
                    continue;
                };
                for (first_frame, last_frame, path) in entries(text, index, plate, own, motion)? {
                    let next = files.len();
                    let file = *files.entry(path.clone()).or_insert(next);
                    pending.push(ScheduledPatch {
                        order: pending.len(),
                        file,
                        first_frame,
                        last_frame,
                        rect: plate.rect,
                        path,
                    });
                }
            }
        }
        let mut last_use: HashMap<usize, u64> = HashMap::new();
        for patch in &pending {
            let last = last_use.entry(patch.file).or_insert(patch.last_frame);
            *last = (*last).max(patch.last_frame);
        }
        pending.sort_by_key(|patch| (patch.first_frame, patch.order));
        Ok(Schedule {
            pending: pending.into(),
            active: Vec::new(),
            last_use,
            next_frame: 0,
        })
    }

    /// Whether no patch is scheduled at all.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty() && self.active.is_empty()
    }

    /// Move to `frame`, later than every frame before; the patch files no later frame blends.
    pub fn advance(&mut self, frame: u64) -> LocalizeResult<Vec<usize>> {
        if frame < self.next_frame {
            return Err(format!("frame {frame} comes after frame {}", self.next_frame - 1).into());
        }
        self.next_frame = frame + 1;
        let mut ended = Vec::new();
        let last_use = &self.last_use;
        self.active.retain(|patch| {
            let keep = patch.last_frame >= frame;
            if !keep && last_use.get(&patch.file) == Some(&patch.last_frame) {
                ended.push(patch.file);
            }
            keep
        });
        let mut started = false;
        while self
            .pending
            .front()
            .is_some_and(|patch| patch.first_frame <= frame)
        {
            if let Some(patch) = self.pending.pop_front()
                && patch.last_frame >= frame
            {
                self.active.push(patch);
                started = true;
            }
        }
        if started {
            self.active.sort_by_key(|patch| patch.order);
        }
        Ok(ended)
    }

    /// The patches covering the current frame, in document order.
    pub fn active(&self) -> &[ScheduledPatch] {
        &self.active
    }
}

/// The entries of `text`'s plate `index`: its whole span with its own patch when its frames
/// keep its own shift, else one per run of frames with the patch of that run's shift.
fn entries(
    text: &ReplacedText,
    index: usize,
    plate: &Plate,
    own: &Path,
    motion: &Motion,
) -> LocalizeResult<Vec<(u64, u64, PathBuf)>> {
    let runs: Vec<_> = motion
        .runs(&text.id, index)
        .iter()
        .filter(|run| run.last_frame >= plate.first_frame && run.first_frame <= plate.last_frame)
        .collect();
    if runs.iter().all(|run| run.shift == plate.shift) {
        return Ok(vec![(
            plate.first_frame,
            plate.last_frame,
            own.to_path_buf(),
        )]);
    }
    let patch_for = |shift: [f64; 2]| -> LocalizeResult<PathBuf> {
        if shift == plate.shift {
            return Ok(own.to_path_buf());
        }
        plate
            .shifted
            .iter()
            .find(|shifted| shifted.shift == shift)
            .map(|shifted| shifted.patch.clone())
            .ok_or_else(|| {
                format!(
                    "{}: plate {index} has no patch for the shift {:?} its frames take",
                    text.id, shift
                )
                .into()
            })
    };
    let mut found = Vec::with_capacity(runs.len() + 2);
    let mut next = plate.first_frame;
    for run in runs {
        let first = run.first_frame.max(plate.first_frame);
        let last = run.last_frame.min(plate.last_frame);
        if first > next {
            found.push((next, first - 1, own.to_path_buf()));
        }
        found.push((first, last, patch_for(run.shift)?));
        next = last + 1;
    }
    if next <= plate.last_frame {
        found.push((next, plate.last_frame, own.to_path_buf()));
    }
    Ok(found)
}

/// Converted patches kept while they are in use, least recently used first out.
#[derive(Debug)]
pub struct PatchCache {
    budget: usize,
    used: usize,
    clock: u64,
    entries: HashMap<usize, Cached>,
}

#[derive(Debug)]
struct Cached {
    patch: FramePatch,
    last_use: u64,
}

impl PatchCache {
    /// An empty cache holding at most `budget` bytes of converted samples.
    pub fn new(budget: usize) -> PatchCache {
        PatchCache {
            budget,
            used: 0,
            clock: 0,
            entries: HashMap::new(),
        }
    }

    /// The converted patch for `entry`, loaded with `load` when it is not cached.
    pub fn get(
        &mut self,
        entry: &ScheduledPatch,
        load: impl FnOnce(&ScheduledPatch) -> LocalizeResult<FramePatch>,
    ) -> LocalizeResult<&FramePatch> {
        self.clock += 1;
        let clock = self.clock;
        if !self.entries.contains_key(&entry.file) {
            let patch = load(entry)?;
            let bytes = patch.bytes();
            while self.used + bytes > self.budget && self.evict_oldest() {}
            self.used += bytes;
            self.entries.insert(
                entry.file,
                Cached {
                    patch,
                    last_use: clock,
                },
            );
        }
        let cached = self
            .entries
            .get_mut(&entry.file)
            .ok_or("a cached patch went missing")?;
        cached.last_use = clock;
        Ok(&cached.patch)
    }

    /// Drop the patch file `file`, which no later frame blends.
    pub fn remove(&mut self, file: usize) {
        if let Some(cached) = self.entries.remove(&file) {
            self.used -= cached.patch.bytes();
        }
    }

    /// The bytes of converted samples held.
    pub fn used(&self) -> usize {
        self.used
    }

    /// How many patches are held.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no patch is held.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Drop the least recently used patch; false when there is none.
    fn evict_oldest(&mut self) -> bool {
        let oldest = self
            .entries
            .iter()
            .min_by_key(|(_, cached)| cached.last_use)
            .map(|(file, _)| *file);
        match oldest {
            Some(file) => {
                self.remove(file);
                true
            }
            None => false,
        }
    }
}

/// Read `entry`'s RGBA patch file under `root` and convert it into frame samples.
pub fn load(
    root: &Path,
    entry: &ScheduledPatch,
    conversion: Conversion,
) -> LocalizeResult<FramePatch> {
    let path = root.join(&entry.path);
    let image = image::open(&path)
        .map_err(|error| format!("read the patch {}: {error}", path.display()))?
        .to_rgba8();
    convert(&image, entry.rect, conversion)
        .map_err(|error| format!("{}: {error}", entry.path.display()).into())
}

#[cfg(test)]
#[path = "tests/patches.rs"]
mod tests;
