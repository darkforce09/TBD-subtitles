//! A job's Check Lines: the list of lines on the left, 330 px wide, and the line the editor shows
//! on the right, over the grouped grey, or what to do when no line is shown.
//!
//! **Role:** lay out the list and the editor, and hold what they share: the borrowed view, the
//! small capitals of a section's label, and the clip's picture texture.
//!
//! **Position:** called by the application under a finished job's Check Lines tab; draws
//! `line_list`, `line_editor` (with `clip_view` and `heard_list`) from here.
//!
//! **Signals and state:** keeps the picture's texture in egui's memory, uploading each frame
//! once.
//!
//! **Invariants:** nothing is saved from here; every click and key is a `ReviewEvent`; the list is
//! 330 px wide; the editor shows `line_filter::open_line`.

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    CentralPanel, Color32, ColorImage, FontFamily, FontId, Frame, Id, Label, Panel, Rect, Response,
    Sense, TextureHandle, TextureOptions, Ui, WidgetInfo, WidgetType, pos2, vec2,
};

use super::{line_editor, line_list};
use crate::core::ui::fonts;
use crate::core::ui::palette::palette;
use crate::line_review::events::ReviewEvent;
use crate::line_review::models::clip::{Frame as PictureFrame, Sound};
use crate::line_review::models::session::ReviewSession;
use crate::line_review::services::line_filter;

/// The width of the list of lines.
const LIST_WIDTH: f32 = 330.0;

/// The clip playing now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Playing {
    pub(crate) sound: Sound,
    /// Where it is, in video seconds.
    pub(crate) position_s: f64,
}

/// What the review view draws for one frame.
pub(crate) struct ReviewView<'a> {
    pub(crate) session: &'a ReviewSession,
    /// The open line's clip while it plays.
    pub(crate) playing: Option<Playing>,
    /// The picture: the playing clip's newest frame, else the open line's still frame.
    pub(crate) frame: Option<PictureFrame>,
    /// Whether the open line's still frame is being decoded.
    pub(crate) decoding: bool,
    /// A full run of the job's video runs now, so nothing can be saved.
    pub(crate) job_busy: bool,
}

/// Draw the review and push what the owner asked for onto `events`.
pub(crate) fn review_view_ui(ui: &mut Ui, view: &ReviewView<'_>, events: &mut Vec<ReviewEvent>) {
    let p = palette(ui);
    Panel::left(Id::new("review-lines"))
        .exact_size(LIST_WIDTH)
        .resizable(false)
        .frame(Frame::new().fill(p.window))
        .show(ui, |ui| line_list::line_list_ui(ui, view.session, events));
    CentralPanel::default()
        .frame(Frame::new().fill(p.grouped))
        .show(ui, |ui| match line_filter::open_line(view.session) {
            Some(line) => line_editor::line_editor_ui(ui, view, line, events),
            None => line_editor::nothing_open_ui(ui, view.session, events),
        });
}

/// A section's label in the mockup's small capitals: 11 px semibold, upper case, spaced out.
pub(super) fn small_caps(ui: &mut Ui, text: &str) {
    let p = palette(ui);
    let mut job = LayoutJob::default();
    job.append(
        &text.to_uppercase(),
        0.0,
        TextFormat {
            font_id: FontId::new(11.0, FontFamily::Name(fonts::SEMIBOLD.into())),
            color: p.text2,
            extra_letter_spacing: 0.44,
            ..TextFormat::default()
        },
    );
    ui.add(Label::new(job));
}

/// Paint `frame` fitted inside `rect`, keeping its shape, uploading it as a texture only when it
/// is new.
pub(super) fn paint_picture(ui: &Ui, rect: Rect, frame: &PictureFrame) {
    let id = Id::new("review-clip-texture");
    let held: Option<(u32, TextureHandle)> = ui.ctx().data(|d| d.get_temp(id));
    let texture = match held {
        Some((serial, texture)) if serial == frame.serial => texture,
        _ => {
            let image = ColorImage::from_rgba_unmultiplied(
                [frame.width as usize, frame.height as usize],
                &frame.rgba,
            );
            let texture = ui
                .ctx()
                .load_texture("review-clip", image, TextureOptions::LINEAR);
            ui.ctx()
                .data_mut(|d| d.insert_temp(id, (frame.serial, texture.clone())));
            texture
        }
    };
    let shape = frame.width as f32 / frame.height.max(1) as f32;
    let (width, height) = if rect.width() / rect.height() > shape {
        (rect.height() * shape, rect.height())
    } else {
        (rect.width(), rect.width() / shape)
    };
    let fitted = Rect::from_center_size(rect.center(), vec2(width, height));
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    ui.painter().image(texture.id(), fitted, uv, Color32::WHITE);
}

/// A click anywhere on `rect`, named `name` for accessibility.
pub(super) fn clickable(ui: &mut Ui, rect: Rect, id: Id, name: &str) -> Response {
    let response = ui.interact(rect, id, Sense::click());
    let name = name.to_string();
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &name));
    response
}
