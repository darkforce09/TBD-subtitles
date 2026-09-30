//! The localized video in Check Text's preview: the Subtitles | Localized video control, the
//! replaced plate while the video is not written, the erase mask over the original picture, and
//! the selected occurrence's replacement status.
//!
//! **Role:** draw what a job that writes a localized video adds to the preview and return the
//! mode and mask events.
//! **Position:** called by `preview` for a session with a localized video.
//! **Signals and state:** egui holds the replaced plate's and the mask's textures, reused while
//! their serials stay.
//! **Invariants:** the mask sits on the plate's rectangle scaled to the drawn picture; the plate
//! shows only while the localized video is not written; the status names the reason whenever an
//! occurrence was not replaced.

use eframe::egui::{
    ColorImage, FontId, Id, Rect, RichText, Sense, TextureHandle, TextureOptions, Ui, Vec2, pos2,
    vec2,
};
use job_model::onscreen::ReplaceStatus;

use super::preview::picture_ui;
use crate::core::ui::palette::{DARK, palette};
use crate::core::ui::segmented::segmented;
use crate::text_review::models::{Event, LocalizedReview, Mask, PreviewMode, Replacement, Session};
use crate::text_review::services::localized::{mask_area, status_line};

/// The picture on the right shows the localized video before it is written: its replaced plate.
pub(super) fn unwritten(localized: &LocalizedReview) -> bool {
    localized.mode == PreviewMode::Localized && localized.video.is_none()
}

/// The selected occurrence's replacement, if it has one.
fn selected<'a>(session: &Session, localized: &'a LocalizedReview) -> Option<&'a Replacement> {
    let text = session.document.occurrences.get(session.selected)?;
    localized.replacements.get(&text.id)
}

/// Subtitles | Localized video, over the picture on the right.
pub(super) fn mode_ui(ui: &mut Ui, localized: &LocalizedReview, events: &mut Vec<Event>) {
    let modes = [
        (PreviewMode::Subtitles, "Subtitles", None),
        (PreviewMode::Localized, "Localized video", None),
    ];
    if let Some(mode) = segmented(ui, localized.mode, &modes) {
        events.push(Event::PreviewMode(mode));
    }
}

/// Show erase mask, beside the original picture's title; off while the occurrence has no mask.
pub(super) fn mask_toggle(
    ui: &mut Ui,
    session: &Session,
    localized: &LocalizedReview,
    events: &mut Vec<Event>,
) {
    let has_mask = selected(session, localized).is_some_and(|r| r.mask.is_some());
    let mut show = localized.show_mask;
    ui.add_space(8.0);
    if ui
        .add_enabled_ui(has_mask, |ui| ui.checkbox(&mut show, "Show erase mask"))
        .inner
        .changed()
    {
        events.push(Event::ShowMask(show));
    }
}

/// The selected occurrence's erase mask in a translucent accent over the original picture drawn
/// at `fitted`, while the mask shows and belongs to it.
pub(super) fn mask_ui(
    ui: &mut Ui,
    session: &Session,
    localized: &LocalizedReview,
    fitted: Rect,
    owner: Id,
) {
    let Some(text) = session.document.occurrences.get(session.selected) else {
        return;
    };
    let mask = localized
        .pictures
        .as_ref()
        .filter(|pictures| localized.show_mask && pictures.id == text.id)
        .and_then(|pictures| pictures.mask.as_ref());
    let Some(mask) = mask else {
        return;
    };
    let (width, height) = (session.document.width, session.document.height);
    let Some([left, top, right, bottom]) = mask_area(mask.rect, width, height) else {
        return;
    };
    let area = Rect::from_min_max(
        fitted.lerp_inside(vec2(left, top)),
        fitted.lerp_inside(vec2(right, bottom)),
    );
    let Some(texture) = mask_texture(ui, mask, owner) else {
        return;
    };
    let tint = palette(ui).accent.gamma_multiply(0.6);
    ui.painter().image(
        texture.id(),
        area,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        tint,
    );
}

/// The mask as a white texture whose alpha is its coverage, uploaded once per mask.
fn mask_texture(ui: &Ui, mask: &Mask, owner: Id) -> Option<TextureHandle> {
    let pixels = (mask.width as usize).checked_mul(mask.height as usize)?;
    if mask.width == 0 || mask.height == 0 || pixels != mask.coverage.len() {
        return None;
    }
    let key = Id::new("text-erase-mask");
    let held: Option<(Id, u64, TextureHandle)> = ui.ctx().data(|data| data.get_temp(key));
    if let Some((source, serial, texture)) = held
        && source == owner
        && serial == mask.serial
    {
        return Some(texture);
    }
    let rgba: Vec<u8> = mask
        .coverage
        .iter()
        .flat_map(|&coverage| [255, 255, 255, coverage])
        .collect();
    let image =
        ColorImage::from_rgba_unmultiplied([mask.width as usize, mask.height as usize], &rgba);
    let texture = ui
        .ctx()
        .load_texture("erase-mask", image, TextureOptions::LINEAR);
    ui.ctx()
        .data_mut(|data| data.insert_temp(key, (owner, mask.serial, texture.clone())));
    Some(texture)
}

/// The right picture while the localized video is not written: the selected occurrence's
/// replaced plate, or why it was not replaced; then the caption saying so.
pub(super) fn plate_ui(
    ui: &mut Ui,
    session: &Session,
    localized: &LocalizedReview,
    size: Vec2,
    owner: Id,
) {
    let p = palette(ui);
    let text = session.document.occurrences.get(session.selected);
    let plate = localized
        .pictures
        .as_ref()
        .filter(|pictures| text.is_some_and(|text| text.id == pictures.id))
        .and_then(|pictures| pictures.preview.as_ref());
    if plate.is_some() {
        picture_ui(ui, plate, size, Id::new(("text-preview", 2)), owner);
    } else {
        // The dark palette's grey reads on the near-black picture in either theme.
        let colour = DARK.text2;
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        ui.painter().rect_filled(rect, 6.0, p.video);
        let galley = ui.painter().layout(
            status_line(selected(session, localized)),
            FontId::proportional(12.0),
            colour,
            (size.x - 24.0).max(1.0),
        );
        ui.painter()
            .galley(rect.center() - galley.size() / 2.0, galley, colour);
    }
    ui.label(
        RichText::new("Localized video not written yet")
            .size(12.0)
            .color(p.text2),
    );
}

/// "Replaced in the video" in green, or "Not replaced in the video: …" in orange.
pub(super) fn status_ui(ui: &mut Ui, session: &Session, localized: &LocalizedReview) {
    let p = palette(ui);
    let replacement = selected(session, localized);
    let colour = match replacement.map(|r| &r.status) {
        Some(ReplaceStatus::Baked) => p.good,
        _ => p.warn,
    };
    ui.label(
        RichText::new(status_line(replacement))
            .size(12.0)
            .color(colour),
    );
}
