//! Progress bars as the mockup draws them: a rounded track in the separator grey filled with the
//! accent, 4 px high, or 6 px for the thick bar of a running job.

use eframe::egui::{CornerRadius, Rect, Response, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2};

use crate::core::ui::palette::palette;

/// A bar `width` px wide filled to `share` (0 to 1): 4 px high, or 6 px when `thick`.
pub(crate) fn bar(ui: &mut Ui, share: f32, width: f32, thick: bool) -> Response {
    let height = if thick { 6.0 } else { 4.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    response.widget_info(|| {
        let mut info = WidgetInfo::new(WidgetType::ProgressIndicator);
        info.value = Some(f64::from(share.clamp(0.0, 1.0)) * 100.0);
        info
    });
    paint_bar(ui, rect, share);
    response
}

/// Paint a bar filling `rect` to `share`: the track, then the accent from the left.
pub(crate) fn paint_bar(ui: &Ui, rect: Rect, share: f32) {
    let p = palette(ui);
    let radius = CornerRadius::same((rect.height() / 2.0).round() as u8);
    ui.painter().rect_filled(rect, radius, p.line);
    let share = share.clamp(0.0, 1.0);
    if share > 0.0 {
        let right = rect.left() + (rect.width() * share).max(rect.height());
        let done = Rect::from_min_max(rect.min, pos2(right.min(rect.right()), rect.bottom()));
        ui.painter().rect_filled(done, radius, p.accent_fill);
    }
}
