//! The open line's clip: its picture (the still frame before Play, the clip's frames while it
//! plays) with the time on it, a painted timeline of the clip with its hatched pads, the line's
//! span and the playhead, the times under it, and Play, Voices Only or Stop.
//!
//! **Role:** draw the clip from the borrowed view and ask to play or stop it.
//!
//! **Position:** drawn by `line_editor` under the line's why boxes; the pad is
//! `clip_player::PAD_S`, the place of a playing clip `clip_player::position_s`.
//!
//! **Signals and state:** asks for frames while a clip plays, so the playhead moves.
//!
//! **Invariants:** the picture is at most 480 px wide and 16:9; the playhead, the still frame and
//! the time on the picture stand at the line's start until the clip plays; the plain dark picture shows while the still frame decodes, and
//! the film mark only when there is no picture at all.

use eframe::egui::{
    Align2, Color32, CornerRadius, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2,
    vec2,
};
use media_io::preview::Clip;

use super::review_view::{ReviewView, paint_picture};
use crate::core::format;
use crate::core::ui::button::Button;
use crate::core::ui::icons;
use crate::core::ui::palette::palette;
use crate::line_review::events::ReviewEvent;
use crate::line_review::models::clip::Sound;
use crate::line_review::models::session::ReviewLine;
use crate::line_review::services::clip_player::PAD_S;

/// The widest the picture and the timeline grow, and the timeline's height.
const WIDTH: f32 = 480.0;
const TIMELINE_HEIGHT: f32 = 34.0;

/// Draw the clip of `line`.
pub(super) fn clip_view_ui(
    ui: &mut Ui,
    view: &ReviewView<'_>,
    line: &ReviewLine,
    events: &mut Vec<ReviewEvent>,
) {
    let clip = Clip::around(line.start_s, line.end_s, PAD_S);
    let width = ui.available_width().min(WIDTH);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 8.0;
        picture_ui(ui, view, line, width);
        timeline_ui(ui, view, line, clip, width);
        ui.add_space(-4.0);
        labels_ui(ui, line, clip, width);
        transport_ui(ui, view, events);
    });
    if view.playing.is_some() {
        ui.ctx().request_repaint();
    }
}

/// The picture in a 16:9 box on the dark video grey, the time at its lower right: the playing
/// clip's, else the line's start, where the still frame is from.
fn picture_ui(ui: &mut Ui, view: &ReviewView<'_>, line: &ReviewLine, width: f32) {
    let p = palette(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(width, width * 9.0 / 16.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(8), p.video);
    match &view.frame {
        Some(frame) => paint_picture(ui, rect, frame),
        None if !view.decoding => {
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                icons::FILM_STRIP,
                icons::font(28.0),
                Color32::from_rgb(0x9A, 0x9A, 0xA2),
            );
        }
        None => {}
    }
    let at = view
        .playing
        .map_or(line.start_s, |playing| playing.position_s);
    let time = ui.painter().layout_no_wrap(
        format::clock_tenths(at),
        FontId::proportional(11.0),
        Color32::from_rgb(0xC9, 0xC9, 0xD0),
    );
    let tag = Rect::from_min_size(
        pos2(
            rect.right() - 8.0 - time.size().x - 12.0,
            rect.bottom() - 7.0 - time.size().y - 2.0,
        ),
        vec2(time.size().x + 12.0, time.size().y + 2.0),
    );
    ui.painter()
        .rect_filled(tag, CornerRadius::same(4), Color32::from_black_alpha(128));
    ui.painter().galley(
        pos2(tag.left() + 6.0, tag.top() + 1.0),
        time,
        Color32::WHITE,
    );
}

/// The clip from end to end: hatched pads either side, the line's span with its length, and the
/// red playhead.
fn timeline_ui(ui: &mut Ui, view: &ReviewView<'_>, line: &ReviewLine, clip: Clip, width: f32) {
    let p = palette(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(width, TIMELINE_HEIGHT), Sense::hover());
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect(
        rect,
        CornerRadius::same(6),
        p.card,
        Stroke::new(1.0, p.line),
        StrokeKind::Inside,
    );
    let share = |at: f64| ((at - clip.start_s) / clip.duration_s).clamp(0.0, 1.0) as f32;
    let x = |at: f64| rect.left() + rect.width() * share(at);
    let (from, to) = (x(line.start_s), x(line.end_s));
    for (left, right) in [(rect.left(), from), (to, rect.right())] {
        let mut stripe = left;
        while stripe < right {
            let bar = Rect::from_min_max(
                pos2(stripe, rect.top() + 1.0),
                pos2((stripe + 3.0).min(right), rect.bottom() - 1.0),
            );
            painter.rect_filled(bar, 0.0, p.well);
            stripe += 6.0;
        }
    }
    let span = Rect::from_min_max(pos2(from, rect.top() + 5.0), pos2(to, rect.bottom() - 5.0));
    painter.rect(
        span,
        CornerRadius::same(4),
        p.accent_tint,
        Stroke::new(1.0, p.accent_fill),
        StrokeKind::Inside,
    );
    let length = format!("{:.1} s", line.length_s());
    painter.with_clip_rect(span.shrink(1.0)).text(
        pos2(span.left() + 6.0, span.center().y),
        Align2::LEFT_CENTER,
        length,
        FontId::proportional(11.0),
        p.accent,
    );
    let head = x(view
        .playing
        .map_or(line.start_s, |playing| playing.position_s));
    painter.rect_filled(
        Rect::from_min_max(
            pos2(head - 1.0, rect.top()),
            pos2(head + 1.0, rect.bottom()),
        ),
        0.0,
        p.bad_icon,
    );
    painter.rect_filled(
        Rect::from_min_max(
            pos2(head - 4.0, rect.top()),
            pos2(head + 4.0, rect.top() + 5.0),
        ),
        CornerRadius {
            nw: 0,
            ne: 0,
            sw: 3,
            se: 3,
        },
        p.bad_icon,
    );
}

/// The clip's start, the line's span, and the clip's end, under the timeline.
fn labels_ui(ui: &mut Ui, line: &ReviewLine, clip: Clip, width: f32) {
    let p = palette(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(width, 14.0), Sense::hover());
    let font = FontId::proportional(11.0);
    let end = clip.start_s + clip.duration_s;
    let centre = rect.center().y;
    for (align, at, text) in [
        (
            Align2::LEFT_CENTER,
            rect.left(),
            format::clock_tenths(clip.start_s),
        ),
        (
            Align2::CENTER_CENTER,
            rect.center().x,
            format!(
                "line {} – {}",
                format::clock_tenths(line.start_s),
                format::clock_tenths(line.end_s)
            ),
        ),
        (
            Align2::RIGHT_CENTER,
            rect.right(),
            format::clock_tenths(end),
        ),
    ] {
        ui.painter()
            .text(pos2(at, centre), align, text, font.clone(), p.text2);
    }
}

/// Play and Voices Only with a hint, or Stop and what is playing.
fn transport_ui(ui: &mut Ui, view: &ReviewView<'_>, events: &mut Vec<ReviewEvent>) {
    let p = palette(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let hint = |ui: &mut Ui, text: &str| {
            ui.label(RichText::new(text).size(11.5).color(p.text2));
        };
        match view.playing {
            Some(playing) => {
                if Button::new("Stop").icon(icons::STOP).show(ui).clicked() {
                    events.push(ReviewEvent::Stop);
                }
                hint(
                    ui,
                    match playing.sound {
                        Sound::Mix => "Playing the video's sound…",
                        Sound::Voices => "Playing the voices only…",
                    },
                );
            }
            None => {
                let play = Button::new("Play")
                    .icon(icons::PLAY)
                    .show(ui)
                    .on_hover_text("The video's sound (Space)");
                if play.clicked() {
                    events.push(ReviewEvent::Play(Sound::Mix));
                }
                let voices = Button::new("Voices Only")
                    .icon(icons::WAVEFORM)
                    .show(ui)
                    .on_hover_text("The separated voices, without music");
                if voices.clicked() {
                    events.push(ReviewEvent::Play(Sound::Voices));
                }
                hint(
                    ui,
                    &format!("Space plays · the clip includes {PAD_S} s before and after"),
                );
            }
        }
    });
}
