//! The window's icons: the Phosphor glyphs the features draw inside their text, and the status
//! marks of rows and toasts.
//!
//! **Role:** name each glyph the window uses once, and paint the status marks: a clock, a
//! progress ring, an empty ring, a check, a warning, a cross, a stop, a spinner and an info mark.
//!
//! **Position:** used by every feature's `ui` and by `core::ui::{button, toast}`; the glyphs come
//! from the icon font `core::ui::fonts` installs, drawn in its own family through [`font`].
//!
//! **Signals and state:** none; a spinner asks for the next frame while it shows.
//!
//! **Invariants:** a mark fills its square; on a selected row every mark is white.

use std::f32::consts::TAU;

use eframe::egui::{
    Align2, Color32, FontFamily, FontId, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2,
    pos2, vec2,
};

use crate::core::ui::fonts;
use crate::core::ui::palette::palette;

pub(crate) const ARROW_CLOCKWISE: &str = egui_phosphor::regular::ARROW_CLOCKWISE;
pub(crate) const ARROW_COUNTER_CLOCKWISE: &str = egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE;
pub(crate) const ARROW_DOWN: &str = egui_phosphor::regular::ARROW_DOWN;
pub(crate) const ARROW_LINE_UP: &str = egui_phosphor::regular::ARROW_LINE_UP;
pub(crate) const ARROW_UP: &str = egui_phosphor::regular::ARROW_UP;
pub(crate) const CARET_DOWN: &str = egui_phosphor::regular::CARET_DOWN;
pub(crate) const CARET_LEFT: &str = egui_phosphor::regular::CARET_LEFT;
pub(crate) const CARET_RIGHT: &str = egui_phosphor::regular::CARET_RIGHT;
pub(crate) const CHECK: &str = egui_phosphor::regular::CHECK;
pub(crate) const CLOCK: &str = egui_phosphor::regular::CLOCK;
pub(crate) const COPY: &str = egui_phosphor::regular::COPY;
pub(crate) const EAR: &str = egui_phosphor::regular::EAR;
pub(crate) const FILE_TEXT: &str = egui_phosphor::regular::FILE_TEXT;
pub(crate) const FILM_STRIP: &str = egui_phosphor::regular::FILM_STRIP;
pub(crate) const FOLDER: &str = egui_phosphor::regular::FOLDER;
pub(crate) const FOLDER_PLUS: &str = egui_phosphor::regular::FOLDER_PLUS;
pub(crate) const GEAR: &str = egui_phosphor::regular::GEAR;
pub(crate) const INFO: &str = egui_phosphor::regular::INFO;
pub(crate) const LIST: &str = egui_phosphor::regular::LIST;
pub(crate) const MAGNIFYING_GLASS: &str = egui_phosphor::regular::MAGNIFYING_GLASS;
pub(crate) const MONITOR_PLAY: &str = egui_phosphor::regular::MONITOR_PLAY;
pub(crate) const PAUSE: &str = egui_phosphor::regular::PAUSE;
pub(crate) const PENCIL_SIMPLE: &str = egui_phosphor::regular::PENCIL_SIMPLE;
pub(crate) const PLAY: &str = egui_phosphor::regular::PLAY;
pub(crate) const PLUS: &str = egui_phosphor::regular::PLUS;
pub(crate) const STOP: &str = egui_phosphor::regular::STOP;
pub(crate) const STOP_CIRCLE: &str = egui_phosphor::regular::STOP_CIRCLE;
pub(crate) const SUBTITLES: &str = egui_phosphor::regular::SUBTITLES;
pub(crate) const WARNING: &str = egui_phosphor::regular::WARNING;
pub(crate) const WAVEFORM: &str = egui_phosphor::regular::WAVEFORM;
pub(crate) const X: &str = egui_phosphor::regular::X;

/// The font a glyph is drawn in, `size` px high: the icon font ahead of every text font.
pub(crate) fn font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(fonts::ICONS.into()))
}

/// A status mark.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum StatusIcon {
    /// A clock: waiting.
    Waiting,
    /// A ring filled to this share, from the top clockwise: running.
    Running(f32),
    /// An empty ring in the quiet grey: still to come.
    Upcoming,
    /// A white check on green: done.
    Done,
    /// A white exclamation on an orange triangle: needs a look.
    Warning,
    /// A white cross on red: failed.
    Failed,
    /// A stop in a circle: cancelled.
    Cancelled,
    /// A turning arc: working.
    Working,
    /// An "i" in a circle: something to know.
    Info,
}

/// Draw `icon` in a `size` square, white when `on_accent` (a selected row).
pub(crate) fn status_icon(ui: &mut Ui, icon: StatusIcon, size: f32, on_accent: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint_status(ui, rect, icon, on_accent);
    response
}

/// Paint `icon` into `rect`, white when `on_accent`.
pub(crate) fn paint_status(ui: &Ui, rect: Rect, icon: StatusIcon, on_accent: bool) {
    let p = palette(ui);
    let painter = ui.painter();
    // The marks are drawn on a 24-unit grid, as the mockup's icons.
    let unit = rect.width() / 24.0;
    let at = |x: f32, y: f32| rect.min + vec2(x, y) * unit;
    let tint = |colour: Color32| if on_accent { Color32::WHITE } else { colour };
    let mark = Stroke::new(
        2.2 * unit,
        if on_accent {
            p.accent_fill
        } else {
            Color32::WHITE
        },
    );
    match icon {
        StatusIcon::Waiting => glyph(ui, rect, CLOCK, tint(p.text3)),
        StatusIcon::Cancelled => glyph(ui, rect, STOP_CIRCLE, tint(p.text3)),
        StatusIcon::Info => glyph(ui, rect, INFO, tint(p.accent)),
        StatusIcon::Done => {
            painter.circle_filled(at(12.0, 12.0), 10.0 * unit, tint(p.good_icon));
            painter.add(Shape::line(
                vec![at(7.8, 12.4), at(10.6, 15.2), at(16.2, 9.4)],
                mark,
            ));
        }
        StatusIcon::Failed => {
            painter.circle_filled(at(12.0, 12.0), 10.0 * unit, tint(p.bad_icon));
            painter.line_segment([at(8.8, 8.8), at(15.2, 15.2)], mark);
            painter.line_segment([at(15.2, 8.8), at(8.8, 15.2)], mark);
        }
        StatusIcon::Warning => {
            let fill = tint(p.warn_icon);
            painter.add(Shape::convex_polygon(
                vec![at(12.0, 3.4), at(21.4, 19.8), at(2.6, 19.8)],
                fill,
                Stroke::new(2.0 * unit, fill),
            ));
            painter.line_segment([at(12.0, 9.3), at(12.0, 13.9)], mark);
            painter.circle_filled(at(12.0, 17.1), 1.2 * unit, mark.color);
        }
        StatusIcon::Running(_) | StatusIcon::Upcoming => {
            let (share, colour) = match icon {
                StatusIcon::Running(share) => (share, tint(p.accent)),
                _ => (0.0, tint(p.text3)),
            };
            let centre = rect.center();
            let radius = rect.width() * 0.4;
            let width = rect.width() * 0.12;
            painter.circle_stroke(
                centre,
                radius,
                Stroke::new(width, colour.gamma_multiply(0.22)),
            );
            let share = share.clamp(0.0, 1.0);
            if share > 0.0 {
                let arc = arc(centre, radius, -TAU / 4.0, share * TAU);
                painter.add(Shape::line(arc, Stroke::new(width, colour)));
            }
        }
        StatusIcon::Working => {
            let turn = (ui.input(|input| input.time) % 1.0) as f32 * TAU;
            let arc = arc(at(12.0, 12.0), 9.0 * unit, -TAU / 4.0 + turn, 0.75 * TAU);
            painter.add(Shape::line(arc, Stroke::new(1.75 * unit, tint(p.accent))));
            ui.ctx().request_repaint();
        }
    }
}

/// A Phosphor glyph filling `rect`.
fn glyph(ui: &Ui, rect: Rect, glyph: &str, colour: Color32) {
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        font(rect.height()),
        colour,
    );
}

/// Points along an arc of `sweep` radians from angle `start`, clockwise on the screen.
fn arc(centre: Pos2, radius: f32, start: f32, sweep: f32) -> Vec<Pos2> {
    let steps = ((sweep / TAU) * 48.0).ceil().max(2.0) as usize;
    (0..=steps)
        .map(|i| {
            let angle = start + sweep * i as f32 / steps as f32;
            pos2(
                centre.x + radius * angle.cos(),
                centre.y + radius * angle.sin(),
            )
        })
        .collect()
}
