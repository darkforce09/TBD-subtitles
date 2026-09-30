//! Original and rendered pictures with transport controls for Check Text.
//!
//! **Role:** display decoded pictures and return playback, seek and frame-step events; for a job
//! that writes a localized video, the picture on the right switches between the subtitles and
//! the localized video, and `replacement` adds the erase mask and the replacement's status.
//! **Position:** borrowed view called by the text editor and its occurrence list.
//! **Signals and state:** egui holds two preview textures and 32 reusable thumbnail slots.
//! **Invariants:** pixels come from the preview service; a texture uploads only for a new serial;
//! frame stepping prefers source presentation times and never changes the source video.

use eframe::egui::{
    self, Align, Color32, ColorImage, Id, Layout, Rect, RichText, Sense, TextureHandle,
    TextureOptions, Ui, Vec2, pos2, vec2,
};

use super::replacement;
use crate::core::format;
use crate::core::ui::button::Button;
use crate::core::ui::icons;
use crate::core::ui::palette::palette;
use crate::text_review::models::{Comparison, Event, Picture, Session};

/// The height of the row over each picture: its title, the mask switch or the mode control.
const HEADER_HEIGHT: f32 = 28.0;

pub(super) fn show(
    ui: &mut Ui,
    session: &Session,
    comparison: Option<&Comparison>,
    playing: bool,
    busy: bool,
    events: &mut Vec<Event>,
) {
    let p = palette(ui);
    let owner = Id::new(&session.work);
    let localized = session.localized.as_ref();
    ui.columns(2, |columns| {
        let [left, right] = columns else {
            return;
        };
        header(left, |ui| {
            ui.label(RichText::new("Original").size(12.0).color(p.text2));
            if let Some(localized) = localized {
                replacement::mask_toggle(ui, session, localized, events);
            }
        });
        let size = picture_size(left);
        let fitted = picture_ui(
            left,
            comparison.map(|pair| &pair.original),
            size,
            Id::new(("text-preview", 0)),
            owner,
        );
        if let (Some(fitted), Some(localized)) = (fitted, localized) {
            replacement::mask_ui(left, session, localized, fitted, owner);
        }
        header(right, |ui| match localized {
            Some(localized) => replacement::mode_ui(ui, localized, events),
            None => {
                ui.label(RichText::new("English subtitles").size(12.0).color(p.text2));
            }
        });
        let size = picture_size(right);
        let rendered = comparison.and_then(|pair| pair.rendered.as_ref());
        match localized {
            Some(localized) if replacement::unwritten(localized) => {
                replacement::plate_ui(right, session, localized, size, owner);
            }
            _ => {
                picture_ui(right, rendered, size, Id::new(("text-preview", 1)), owner);
            }
        }
    });
    let shown_time = comparison.map_or(session.position_s, |pair| pair.time_s);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format::clock_tenths(shown_time))
                .size(12.0)
                .color(p.text2),
        );
        if busy {
            ui.spinner();
            ui.label(RichText::new("Updating…").size(12.0).color(p.text2));
        }
    });
    if let Some(localized) = localized {
        replacement::status_ui(ui, session, localized);
    }
    let Some(occurrence) = session.document.occurrences.get(session.selected) else {
        return;
    };
    let from = (occurrence.start_s - 0.75).max(0.0);
    let to = (occurrence.end_s + 0.75).min(session.duration_s).max(from);
    let mut position = session.position_s.clamp(from, to);
    ui.add_enabled_ui(!busy, |ui| {
        if ui
            .add(
                egui::Slider::new(&mut position, from..=to)
                    .show_value(false)
                    .text("Preview position"),
            )
            .changed()
        {
            seek(events, position, playing);
        }
    });
    ui.horizontal_wrapped(|ui| {
        if playing {
            if Button::new("Stop").icon(icons::STOP).show(ui).clicked() {
                events.push(Event::Stop);
            }
        } else if Button::new("Play")
            .icon(icons::PLAY)
            .enabled(!busy)
            .show(ui)
            .clicked()
        {
            events.push(Event::Play);
        }
        for (forward, label, icon) in [
            (false, "Previous frame", icons::CARET_LEFT),
            (true, "Next frame", icons::CARET_RIGHT),
        ] {
            if Button::new(label)
                .icon(icon)
                .enabled(!busy)
                .show(ui)
                .clicked()
            {
                seek(events, adjacent_frame(session, forward), playing);
            }
        }
        ui.label(
            RichText::new(format!(
                "{} – {}",
                format::clock_tenths(from),
                format::clock_tenths(to)
            ))
            .size(11.0)
            .color(p.text2),
        );
    });
    if playing {
        ui.ctx().request_repaint();
    }
}

pub(super) fn thumbnail(ui: &mut Ui, session: &Session, index: usize) {
    let picture = session.thumbnails.get(index).and_then(Option::as_ref);
    picture_ui(
        ui,
        picture,
        vec2(78.0, 48.0),
        Id::new(("text-thumbnail", index % 32)),
        Id::new(&session.work),
    );
}

/// A preview picture's size in `ui`: its whole width at 16:9, at most 300 px high.
fn picture_size(ui: &Ui) -> Vec2 {
    let width = ui.available_width().max(1.0);
    vec2(width, (width * 9.0 / 16.0).min(300.0))
}

/// A header row 28 px high, so both columns' pictures start level.
fn header(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let width = ui.available_width();
    ui.allocate_ui_with_layout(
        vec2(width, HEADER_HEIGHT),
        Layout::left_to_right(Align::Center),
        add,
    );
}

/// Draw `picture` fitted into a video-black `size` box; where the picture landed, if drawn.
pub(super) fn picture_ui(
    ui: &mut Ui,
    picture: Option<&Picture>,
    size: Vec2,
    key: Id,
    owner: Id,
) -> Option<Rect> {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    if !ui.is_rect_visible(rect) {
        return None;
    }
    ui.painter().rect_filled(rect, 6.0, palette(ui).video);
    let frame = picture?;
    let expected = (frame.width as usize)
        .checked_mul(frame.height as usize)
        .and_then(|pixels| pixels.checked_mul(3));
    if frame.width == 0 || frame.height == 0 || expected != Some(frame.rgb.len()) {
        return None;
    }
    let held: Option<(Id, u64, TextureHandle)> = ui.ctx().data(|data| data.get_temp(key));
    let texture = match held {
        Some((source, serial, texture)) if source == owner && serial == frame.serial => texture,
        _ => {
            let pixels =
                ColorImage::from_rgb([frame.width as usize, frame.height as usize], &frame.rgb);
            let texture = ui
                .ctx()
                .load_texture("on-screen-text", pixels, TextureOptions::LINEAR);
            ui.ctx()
                .data_mut(|data| data.insert_temp(key, (owner, frame.serial, texture.clone())));
            texture
        }
    };
    let ratio = frame.width as f32 / frame.height as f32;
    let width = rect.width().min(rect.height() * ratio);
    let fitted = Rect::from_center_size(rect.center(), vec2(width, width / ratio));
    ui.painter().image(
        texture.id(),
        fitted,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    Some(fitted)
}

fn adjacent_frame(session: &Session, forward: bool) -> f64 {
    let current = session.position_s;
    let times = session
        .document
        .occurrences
        .get(session.selected)
        .into_iter()
        .flat_map(|text| &text.frames)
        .map(|frame| frame.time_s)
        .filter(|time| time.is_finite());
    let next = if forward {
        times
            .filter(|time| *time > current + 0.000_001)
            .min_by(f64::total_cmp)
    } else {
        times
            .filter(|time| *time < current - 0.000_001)
            .max_by(f64::total_cmp)
    };
    let fps = if session.fps.is_finite() && session.fps > 0.0 {
        session.fps
    } else {
        24.0
    };
    next.unwrap_or(current + if forward { 1.0 / fps } else { -1.0 / fps })
        .clamp(0.0, session.duration_s.max(0.0))
}

fn seek(events: &mut Vec<Event>, time_s: f64, playing: bool) {
    if playing {
        events.push(Event::Stop);
    }
    events.push(Event::Seek(time_s));
}
