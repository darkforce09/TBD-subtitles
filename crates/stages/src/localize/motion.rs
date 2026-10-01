//! Where the writing sits frame by frame, folded from the `frames` rows: the shift each plate's
//! frames take, as runs of frames that share one.
//!
//! **Role:** collect, one row at a time, each occurrence's per-frame shifts into runs of equal
//! shift per plate, and answer which shift a frame takes and which shifts a plate needs patches
//! for.
//! **Position:** built by the pipeline's composition, read-back and localized-video tasks from
//! the rows they read; read by `compose` (one patch per shift), `patches::Schedule` (which patch
//! covers a frame) and `verify::area` (where a sampled frame's lettering sits).
//! **Signals and state:** per occurrence and plate, its runs in frame order; it grows with the
//! number of shift changes, never with the number of frames.
//! **Invariants:** rows arrive in key order, so an occurrence's frames ascend; a run holds
//! consecutive frames of one plate with one shift; a frame no row covers takes its plate's own
//! shift.

use std::collections::HashMap;

/// Consecutive frames of one plate that share one shift.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShiftRun {
    pub first_frame: u64,
    pub last_frame: u64,
    pub shift: [f64; 2],
}

/// Every occurrence's runs of shifts, by plate.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Motion {
    texts: HashMap<String, Vec<Vec<ShiftRun>>>,
}

impl Motion {
    /// Add `occurrence`'s frame `frame`, which belongs to its plate `plate` and shows the writing
    /// at `shift`. Rows come in key order.
    pub fn add(&mut self, occurrence: &str, frame: u64, plate: u32, shift: [f64; 2]) {
        let plates = match self.texts.get_mut(occurrence) {
            Some(plates) => plates,
            None => self.texts.entry(occurrence.to_string()).or_default(),
        };
        let plate = plate as usize;
        if plates.len() <= plate {
            plates.resize_with(plate + 1, Vec::new);
        }
        let runs = &mut plates[plate];
        match runs.last_mut() {
            Some(run) if run.shift == shift && run.last_frame.checked_add(1) == Some(frame) => {
                run.last_frame = frame;
            }
            _ => runs.push(ShiftRun {
                first_frame: frame,
                last_frame: frame,
                shift,
            }),
        }
    }

    /// The runs of `occurrence`'s plate `plate`, in frame order; empty when no row named it.
    pub fn runs(&self, occurrence: &str, plate: usize) -> &[ShiftRun] {
        self.texts
            .get(occurrence)
            .and_then(|plates| plates.get(plate))
            .map_or(&[], Vec::as_slice)
    }

    /// The shift `occurrence`'s plate `plate` shows at `frame`, when a row covers that frame.
    pub fn shift_at(&self, occurrence: &str, plate: usize, frame: u64) -> Option<[f64; 2]> {
        let runs = self.runs(occurrence, plate);
        let after = runs.partition_point(|run| run.first_frame <= frame);
        let run = runs.get(after.checked_sub(1)?)?;
        (frame <= run.last_frame).then_some(run.shift)
    }

    /// Every shift of `occurrence`'s plate `plate` other than `own`, in the order its frames
    /// first take them.
    pub fn other_shifts(&self, occurrence: &str, plate: usize, own: [f64; 2]) -> Vec<[f64; 2]> {
        let mut shifts: Vec<[f64; 2]> = Vec::new();
        for run in self.runs(occurrence, plate) {
            if run.shift != own && !shifts.contains(&run.shift) {
                shifts.push(run.shift);
            }
        }
        shifts
    }

    /// Whether no row was added.
    pub fn is_empty(&self) -> bool {
        self.texts.is_empty()
    }
}

#[cfg(test)]
#[path = "tests/motion.rs"]
mod tests;
