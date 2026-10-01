//! Where a scan spends its time and what it handles.
//!
//! **Role:** name the scan's phases, which the coordinator, the probes and confirmation add to as
//! they run, with the counts of frames decoded, screened and probed, the keyframes read from
//! memory or decoded again, and what the detector sessions report about their start-up.
//!
//! **Position:** filled by the scan; returned by `scan_measured` to `pipeline::tasks::onscreen`,
//! whose `text_detect` step notes it.
//!
//! **Signals and state:** one `ScanStats` per scan.
//!
//! **Invariants:** measuring never changes the document; screening counts the sample pictures
//! and the probe pictures alike; the waits are wall time on the scan's own thread, so phases
//! that overlap on other threads (decoding ahead, the GPU, the PNG writer) show only as the time
//! the scan waited for them.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// Where one scan spent its time and what it handled.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScanStats {
    /// Waiting on the decoder for the next frame.
    pub decode_wait: Duration,
    /// Converting the screened samples to padded pictures.
    pub convert: Duration,
    /// Waiting for screening results the scan needs next.
    pub screen: Duration,
    /// Bisection probes: converting held frames and waiting for their screens.
    pub probe: Duration,
    /// Region signatures: comparing anchors on samples and on probed frames.
    pub signature: Duration,
    /// In the server detector, confirming keyframes.
    pub confirm: Duration,
    /// Converting keyframes held in memory for confirmation.
    pub stills_from_ram: Duration,
    /// Decoding keyframe stills from the video, for keyframes not held in memory.
    pub stills_from_ffmpeg: Duration,
    /// The frames decoded.
    pub frames_decoded: u64,
    /// The pictures screened: samples and bisection probes.
    pub frames_screened: u64,
    /// The bisection probes among them.
    pub frames_probed: u64,
    /// Keyframes confirmed from frames held in memory.
    pub keyframes_from_ram: u64,
    /// Keyframes confirmed on stills decoded from the video.
    pub keyframes_from_ffmpeg: u64,
    /// The most bytes the keyframe candidates held in memory at once.
    pub keyframes_held_peak_bytes: u64,
    /// Seconds the detector sessions spent warming up when they opened.
    pub warmup_s: f64,
    /// Seconds spent building TensorRT engines; zero when cached engines were used.
    pub engine_build_s: f64,
    /// What the detector sessions recorded about how they run.
    pub notes: BTreeMap<String, String>,
}

impl ScanStats {
    /// The stats as step notes, `(key, value)`: seconds with one decimal under `<phase>_s`, the
    /// counts as numbers, and the sessions' own notes under `detector_<key>`.
    pub fn notes(&self) -> Vec<(String, String)> {
        let seconds = [
            ("decode_wait_s", self.decode_wait),
            ("convert_s", self.convert),
            ("screen_s", self.screen),
            ("probe_s", self.probe),
            ("signature_s", self.signature),
            ("confirm_s", self.confirm),
            ("stills_ram_s", self.stills_from_ram),
            ("stills_ffmpeg_s", self.stills_from_ffmpeg),
            ("warmup_s", Duration::from_secs_f64(self.warmup_s.max(0.0))),
            (
                "engine_build_s",
                Duration::from_secs_f64(self.engine_build_s.max(0.0)),
            ),
        ];
        let counts = [
            ("frames_decoded", self.frames_decoded),
            ("frames_screened", self.frames_screened),
            ("frames_probed", self.frames_probed),
            ("keyframes_ram", self.keyframes_from_ram),
            ("keyframes_ffmpeg", self.keyframes_from_ffmpeg),
            (
                "keyframes_held_peak_mib",
                self.keyframes_held_peak_bytes >> 20,
            ),
        ];
        seconds
            .iter()
            .map(|(key, spent)| (key.to_string(), format!("{:.1}", spent.as_secs_f64())))
            .chain(
                counts
                    .iter()
                    .map(|(key, count)| (key.to_string(), count.to_string())),
            )
            .chain(
                self.notes
                    .iter()
                    .map(|(key, value)| (format!("detector_{key}"), value.clone())),
            )
            .collect()
    }
}

/// `f`'s result, with the time it took added to `phase`.
pub(crate) fn timed<T>(phase: &mut Duration, f: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let result = f();
    *phase += started.elapsed();
    result
}

#[cfg(test)]
#[path = "tests/timing.rs"]
mod tests;
