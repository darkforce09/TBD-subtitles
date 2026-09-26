//! The shared look of the window: sizes and colours every feature's UI uses.

use eframe::egui::Color32;

/// Secondary text: hints, empty states, file paths.
pub(crate) const MUTED_TEXT: Color32 = Color32::from_rgb(150, 150, 150);
/// Something done or in order.
pub(crate) const GOOD: Color32 = Color32::from_rgb(90, 180, 110);
/// Something to look at, which does not stop a job.
pub(crate) const CAUTION: Color32 = Color32::from_rgb(220, 170, 60);
/// Something broken, which stops a job.
pub(crate) const BAD: Color32 = Color32::from_rgb(220, 90, 80);
