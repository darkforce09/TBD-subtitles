//! The Models tab: every model and runtime library a job needs, with its kind, size and state,
//! and under the table the download: Download Missing, or Stop while it runs, or that everything
//! is on disk.
//!
//! **Role:** draw the rows `model_list::rows` gives in a bordered table, the row being downloaded
//! with its bar, and turn the buttons into `Download` and `StopDownload`.
//!
//! **Position:** drawn by `settings_window` while Models is open.
//!
//! **Signals and state:** none; reads the borrowed page and returns events.
//!
//! **Invariants:** the rows are the ones the models banner counts; a stopped download says it
//! resumes next time.

use eframe::egui::{
    Align2, Color32, CornerRadius, FontFamily, FontId, Frame, Label, Rect, RichText, Sense, Stroke,
    Ui, pos2, vec2,
};

use crate::core::format;
use crate::core::ui::button::Button;
use crate::core::ui::fonts;
use crate::core::ui::icons::{self, StatusIcon, status_icon};
use crate::core::ui::palette::palette;
use crate::core::ui::progress::paint_bar;
use crate::settings::events::SettingsEvent;
use crate::settings::models::machine::ItemKind;
use crate::settings::models::page::SettingsPage;
use crate::settings::services::model_list::{self, ModelRow, RowState};

/// A cell's side padding, a row's and the header's heights, and the widths of the columns right
/// of the name.
const PAD: f32 = 18.0;
const ROW_HEIGHT: f32 = 31.0;
const HEAD_HEIGHT: f32 = 27.0;
const KIND_WIDTH: f32 = 60.0;
const SIZE_WIDTH: f32 = 70.0;
const STATUS_WIDTH: f32 = 150.0;

/// Draw the Models tab from `page` and push what the owner asked for onto `events`.
pub(super) fn models_ui(ui: &mut Ui, page: &SettingsPage, events: &mut Vec<SettingsEvent>) {
    let p = palette(ui);
    ui.add(
        Label::new(
            RichText::new(
                "The list follows your engine choices. Each file is checked against its known \
                 fingerprint after download.",
            )
            .color(p.text2),
        )
        .wrap(),
    );
    let rows = model_list::rows(&page.items, page.download.as_ref());
    Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(CornerRadius::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            table_ui(ui, &rows);
        });
    let missing = model_list::missing(&page.items);
    if let Some(progress) = &page.download {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            if Button::new("Stop").show(ui).clicked() {
                events.push(SettingsEvent::StopDownload);
            }
            ui.label(
                RichText::new(format!(
                    "Downloading {} of {}. A stopped file resumes next time.",
                    format::size(progress.done()),
                    format::size(progress.size)
                ))
                .size(11.5)
                .color(p.text2),
            );
        });
    } else if missing.any() {
        let label = format!("Download Missing ({})", format::size(missing.bytes));
        if Button::new(&label)
            .icon(icons::DOWNLOAD)
            .primary(true)
            .show(ui)
            .clicked()
        {
            events.push(SettingsEvent::Download);
        }
    } else {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            status_icon(ui, StatusIcon::Done, 18.0, false);
            ui.label(RichText::new("Everything a job needs is on disk.").color(p.good));
        });
    }
}

/// The table: its header, then one row per model and runtime library, a line between each.
fn table_ui(ui: &mut Ui, rows: &[ModelRow]) {
    let p = palette(ui);
    let width = ui.available_width();
    let (head, _) = ui.allocate_exact_size(vec2(width, HEAD_HEIGHT), Sense::hover());
    let columns = Columns::of(head);
    let semibold = FontId::new(11.0, FontFamily::Name(fonts::SEMIBOLD.into()));
    for (x, align, title) in [
        (columns.name, Align2::LEFT_CENTER, "Name"),
        (columns.kind, Align2::LEFT_CENTER, "Kind"),
        (columns.size_right, Align2::RIGHT_CENTER, "Size"),
        (columns.status, Align2::LEFT_CENTER, "Status"),
    ] {
        ui.painter().text(
            pos2(x, head.center().y),
            align,
            title,
            semibold.clone(),
            p.text2,
        );
    }
    line(ui, head.bottom(), head);
    for (index, row) in rows.iter().enumerate() {
        let (rect, _) = ui.allocate_exact_size(vec2(width, ROW_HEIGHT), Sense::hover());
        let y = rect.center().y;
        let body = FontId::proportional(13.0);
        let name = ui.painter().layout(
            row.name.clone(),
            body.clone(),
            p.text,
            columns.kind - PAD - columns.name,
        );
        ui.painter()
            .galley(pos2(columns.name, y - name.size().y / 2.0), name, p.text);
        let kind = match row.kind {
            ItemKind::Model => "model",
            ItemKind::Runtime => "runtime",
        };
        ui.painter().text(
            pos2(columns.kind, y),
            Align2::LEFT_CENTER,
            kind,
            body.clone(),
            p.text2,
        );
        ui.painter().text(
            pos2(columns.size_right, y),
            Align2::RIGHT_CENTER,
            format::size(row.bytes),
            body.clone(),
            p.text,
        );
        status_ui(ui, row.state, pos2(columns.status, y));
        if index + 1 < rows.len() {
            line(ui, rect.bottom(), rect);
        }
    }
}

/// Where each column starts, in a row as wide as `rect`: the size column is right-aligned.
struct Columns {
    name: f32,
    kind: f32,
    size_right: f32,
    status: f32,
}

impl Columns {
    fn of(rect: Rect) -> Columns {
        let status = rect.right() - PAD - STATUS_WIDTH;
        let size_right = status - PAD;
        Columns {
            name: rect.left() + PAD,
            kind: size_right - SIZE_WIDTH - PAD - KIND_WIDTH,
            size_right,
            status,
        }
    }
}

/// A row's state at `at` (its left, centred): "On disk" in green with a check, "Missing" in red,
/// or a 120 px bar with the share on disk.
fn status_ui(ui: &mut Ui, state: RowState, at: eframe::egui::Pos2) {
    let p = palette(ui);
    let body = FontId::proportional(13.0);
    match state {
        RowState::OnDisk => {
            ui.painter().text(
                at,
                Align2::LEFT_CENTER,
                icons::CHECK,
                icons::font(14.0),
                p.good,
            );
            ui.painter().text(
                at + vec2(19.0, 0.0),
                Align2::LEFT_CENTER,
                "On disk",
                body,
                p.good,
            );
        }
        RowState::Missing => {
            let medium = FontId::new(13.0, FontFamily::Name(fonts::SEMIBOLD.into()));
            ui.painter()
                .text(at, Align2::LEFT_CENTER, "Missing", medium, p.bad);
        }
        RowState::Downloading(share) => {
            let bar = Rect::from_min_size(at - vec2(0.0, 2.0), vec2(120.0, 4.0));
            paint_bar(ui, bar, share);
            ui.painter().text(
                at + vec2(126.0, 0.0),
                Align2::LEFT_CENTER,
                format!("{} %", (share * 100.0).round() as u32),
                body,
                p.text,
            );
        }
    }
}

/// A 1 px separator at `y` across `rect`.
fn line(ui: &Ui, y: f32, rect: Rect) {
    let colour: Color32 = palette(ui).line;
    ui.painter()
        .hline(rect.x_range(), y - 0.5, Stroke::new(1.0, colour));
}
