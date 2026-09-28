//! Progress bars as the mockup draws them: a rounded track in the separator grey filled with the
//! accent, 4 px high, or 6 px for the thick bar of a running job; or filled green for a share of
//! things done, such as the lines checked.

use eframe::egui::{
    Color32, CornerRadius, Rect, Response, Sense, Ui, Vec2, WidgetInfo, WidgetType, pos2, vec2,
};

use crate::core::ui::palette::palette;

/// A bar `width` px wide filled to `share` (0 to 1): 4 px high, or 6 px when `thick`.
pub(crate) fn bar(ui: &mut Ui, share: f32, width: f32, thick: bool) -> Response {
    let height = if thick { 6.0 } else { 4.0 };
    let fill = palette(ui).accent_fill;
    shown(ui, share, vec2(width, height), fill)
}

/// A 4 px bar `width` px wide filled green to `share` (0 to 1): a share of things done.
pub(crate) fn good_bar(ui: &mut Ui, share: f32, width: f32) -> Response {
    let fill = palette(ui).good_icon;
    shown(ui, share, vec2(width, 4.0), fill)
}

/// Paint a bar filling `rect` to `share`: the track, then the accent from the left.
pub(crate) fn paint_bar(ui: &Ui, rect: Rect, share: f32) {
    paint(ui, rect, share, palette(ui).accent_fill);
}

/// Allocate a bar of `size`, name it as a progress indicator, and paint it filled with `fill`.
fn shown(ui: &mut Ui, share: f32, size: Vec2, fill: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| {
        let mut info = WidgetInfo::new(WidgetType::ProgressIndicator);
        info.value = Some(f64::from(share.clamp(0.0, 1.0)) * 100.0);
        info
    });
    paint(ui, rect, share, fill);
    response
}

/// Paint the track in `rect`, then `fill` from the left to `share`.
fn paint(ui: &Ui, rect: Rect, share: f32, fill: Color32) {
    let p = palette(ui);
    let radius = CornerRadius::same((rect.height() / 2.0).round() as u8);
    ui.painter().rect_filled(rect, radius, p.line);
    let share = share.clamp(0.0, 1.0);
    if share > 0.0 {
        let right = rect.left() + (rect.width() * share).max(rect.height());
        let done = Rect::from_min_max(rect.min, pos2(right.min(rect.right()), rect.bottom()));
        ui.painter().rect_filled(done, radius, fill);
    }
}
