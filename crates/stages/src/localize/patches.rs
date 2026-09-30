//! Which composed patches cover each frame, and a bounded cache of their converted samples.
//!
//! **Role:** turn the replacement document into a frame-by-frame schedule of patches, and load
//! each patch file once into frame samples while its span lasts.
//! **Position:** inside `localize::render`, between the replacement document and `blend`.
//! **Signals and state:** the schedule holds the patches not yet started, ordered by first
//! frame, and the active ones in document order; the cache holds converted patches with a
//! use stamp and their total size.
//! **Invariants:** only baked occurrences' plates with a patch are scheduled; frames advance
//! strictly; a patch is active exactly on `first_frame..=last_frame`; overlapping patches apply
//! in document order; the cache stays within its byte budget except for a single patch larger
//! than the budget, and a patch whose span has ended leaves it.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use job_model::onscreen::{PixelRect, ReplacementDocument};

use super::LocalizeResult;
use super::blend::{FramePatch, convert};
use super::colour::Conversion;

/// One patch and the frames it covers.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledPatch {
    /// The patch's position in the document: occurrences, then plates, in order.
    pub order: usize,
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
    next_frame: u64,
}

impl Schedule {
    /// Every patch of the document's baked occurrences.
    pub fn new(document: &ReplacementDocument) -> Schedule {
        let mut pending: Vec<ScheduledPatch> = document
            .baked()
            .flat_map(|text| text.plates.iter())
            .filter_map(|plate| plate.patch.as_ref().map(|path| (plate, path.clone())))
            .enumerate()
            .map(|(order, (plate, path))| ScheduledPatch {
                order,
                first_frame: plate.first_frame,
                last_frame: plate.last_frame,
                rect: plate.rect,
                path,
            })
            .collect();
        pending.sort_by_key(|patch| (patch.first_frame, patch.order));
        Schedule {
            pending: pending.into(),
            active: Vec::new(),
            next_frame: 0,
        }
    }

    /// Whether no patch is scheduled at all.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty() && self.active.is_empty()
    }

    /// Move to `frame`, later than every frame before; the orders of the patches that ended.
    pub fn advance(&mut self, frame: u64) -> LocalizeResult<Vec<usize>> {
        if frame < self.next_frame {
            return Err(format!("frame {frame} comes after frame {}", self.next_frame - 1).into());
        }
        self.next_frame = frame + 1;
        let mut ended = Vec::new();
        self.active.retain(|patch| {
            let keep = patch.last_frame >= frame;
            if !keep {
                ended.push(patch.order);
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
        if !self.entries.contains_key(&entry.order) {
            let patch = load(entry)?;
            let bytes = patch.bytes();
            while self.used + bytes > self.budget && self.evict_oldest() {}
            self.used += bytes;
            self.entries.insert(
                entry.order,
                Cached {
                    patch,
                    last_use: clock,
                },
            );
        }
        let cached = self
            .entries
            .get_mut(&entry.order)
            .ok_or("a cached patch went missing")?;
        cached.last_use = clock;
        Ok(&cached.patch)
    }

    /// Drop the patch with `order`, whose span has ended.
    pub fn remove(&mut self, order: usize) {
        if let Some(cached) = self.entries.remove(&order) {
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
            .map(|(order, _)| *order);
        match oldest {
            Some(order) => {
                self.remove(order);
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
