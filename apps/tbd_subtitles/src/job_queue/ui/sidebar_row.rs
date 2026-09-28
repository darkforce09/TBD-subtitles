//! One row of the sidebar: the video's status mark, its name, its status line and, while it
//! runs, a thin progress bar; a red round ✕ on hover, dragging for waiting rows, and a
//! right-click menu.
//!
//! **Role:** draw a row from the borrowed queue and turn each click, drop and menu choice into a
//! `JobQueueEvent`.
//!
//! **Position:** called by `sidebar` for each row shown; opens `row_menu` on a right click; the
//! status line comes from `status_text`.
//!
//! **Signals and state:** the dragged row's id travels as egui's drag-and-drop payload.
//!
//! **Invariants:** a click anywhere on the row selects it, except on the ✕; only waiting full
//! runs drag, and a drop lands before or after the row under the pointer, as the line shows;
//! the ✕ shows only on a row that can leave the list.

use std::sync::Arc;

use eframe::egui::{
    Align2, Color32, CornerRadius, FontId, Frame, Galley, Margin, Popup, Rect, Response, Sense,
    Stroke, Ui, WidgetInfo, WidgetType, pos2, text::LayoutJob, text::TextWrapping, vec2,
};

use crate::core::ui::icons::{self, StatusIcon, paint_status};
use crate::core::ui::palette::palette;
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::queue::{JobId, JobState, QueueItem};
use crate::job_queue::models::sidebar::SidebarRow;
use crate::job_queue::models::view::JobQueueView;
use crate::job_queue::services::{status_text, time_left};
use crate::job_queue::ui::row_menu::row_menu_ui;

/// A row's height; a running row is taller by its bar.
const ROW_HEIGHT: f32 = 48.0;
const BAR_ROOM: f32 = 7.0;
/// The status mark's size and the space around the row's content.
const ICON: f32 = 18.0;
const LEFT: f32 = 10.0;
const RIGHT: f32 = 8.0;
const GAP: f32 = 10.0;
/// The ✕'s diameter.
const REMOVE: f32 = 20.0;
/// The width of the right-click menu, which fits its longest command.
const MENU_WIDTH: f32 = 244.0;

/// Draw `row` and push what the owner asked for onto `events`; `next` is the row after it in its
/// section, where a drop below it lands before.
pub(crate) fn sidebar_row_ui(
    ui: &mut Ui,
    view: &JobQueueView<'_>,
    row: &SidebarRow,
    next: Option<JobId>,
    events: &mut Vec<JobQueueEvent>,
) {
    let Some(item) = view.queue.get(row.id) else {
        return;
    };
    let p = palette(ui);
    let running = match &item.state {
        JobState::Running(progress) => {
            Some(time_left::estimate(progress, view.rates, view.now).map_or(0.0, |(_, s)| s as f32))
        }
        _ => None,
    };
    let height = ROW_HEIGHT + if running.is_some() { BAR_ROOM } else { 0.0 };
    let sense = if row.draggable {
        Sense::click_and_drag()
    } else {
        Sense::click()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), sense);
    let selected = view.queue.selected == Some(row.id);
    response.widget_info(|| {
        WidgetInfo::selected(WidgetType::SelectableLabel, true, selected, &row.name)
    });
    let pointer_in = response.contains_pointer();
    let dragged = response.dragged();
    if response.clicked() || response.secondary_clicked() {
        events.push(JobQueueEvent::Select(row.id));
    }
    let fade = |colour: Color32| {
        if dragged {
            colour.gamma_multiply(0.4)
        } else {
            colour
        }
    };
    if selected {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(8), fade(p.accent_fill));
    } else if pointer_in && ui.ctx().dragged_id().is_none() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(8), p.hover);
    }
    let icon_rect = Rect::from_center_size(
        pos2(rect.left() + LEFT + ICON / 2.0, rect.center().y),
        vec2(ICON, ICON),
    );
    paint_status(ui, icon_rect, status_icon(row, item, running), selected);
    let show_remove = row.removable && pointer_in && ui.ctx().dragged_id().is_none();
    let text_left = icon_rect.right() + GAP;
    let text_right = rect.right() - RIGHT - if show_remove { REMOVE + GAP } else { 0.0 };
    let width = (text_right - text_left).max(10.0);
    let (name_colour, status_colour) = if selected {
        (Color32::WHITE, Color32::WHITE)
    } else if matches!(item.state, JobState::Failed(_)) {
        (p.text, p.bad)
    } else {
        (p.text, p.text2)
    };
    let name = line(ui, &row.name, 13.0, fade(name_colour), width);
    let status = status_text::status(row, item, view.queue, view.rates, view.now);
    let status = line(ui, &status, 11.5, fade(status_colour), width);
    let content = name.size().y + 1.0 + status.size().y + running.map_or(0.0, |_| BAR_ROOM);
    let top = rect.center().y - content / 2.0;
    let status_top = top + name.size().y + 1.0;
    let bar_top = status_top + status.size().y + 4.0;
    ui.painter().galley(pos2(text_left, top), name, name_colour);
    ui.painter()
        .galley(pos2(text_left, status_top), status, status_colour);
    if let Some(share) = running {
        let track = Rect::from_min_size(pos2(text_left, bar_top), vec2(width, 3.0));
        let (back, fill) = if selected {
            (Color32::from_white_alpha(77), Color32::WHITE)
        } else {
            (p.line, p.accent_fill)
        };
        ui.painter().rect_filled(track, CornerRadius::same(2), back);
        let done = Rect::from_min_size(track.min, vec2(width * share, 3.0));
        ui.painter().rect_filled(done, CornerRadius::same(2), fill);
    }
    if show_remove {
        remove_ui(ui, rect, row, events);
    }
    if row.draggable {
        drop_ui(ui, &response, rect, row, next, events);
        response.dnd_set_drag_payload(row.id);
    }
    let response = response.on_hover_text(row.video.display().to_string());
    Popup::context_menu(&response)
        .frame(
            Frame::new()
                .fill(p.card)
                .stroke(Stroke::new(1.0, p.line_strong))
                .corner_radius(CornerRadius::same(9))
                .inner_margin(Margin::same(5))
                .shadow(ui.visuals().popup_shadow),
        )
        .width(MENU_WIDTH)
        .show(|ui| row_menu_ui(ui, view, row, item, events));
}

/// The red round ✕ at the row's right end.
fn remove_ui(ui: &mut Ui, rect: Rect, row: &SidebarRow, events: &mut Vec<JobQueueEvent>) {
    let p = palette(ui);
    let centre = pos2(rect.right() - RIGHT - REMOVE / 2.0, rect.center().y);
    let area = Rect::from_center_size(centre, vec2(REMOVE, REMOVE));
    let response = ui.interact(area, ui.id().with(("remove", row.id)), Sense::click());
    let name = format!("Remove {}", row.name);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &name));
    ui.painter().circle_filled(centre, REMOVE / 2.0, p.bad_icon);
    ui.painter().text(
        centre,
        Align2::CENTER_CENTER,
        icons::X,
        icons::font(12.0),
        Color32::WHITE,
    );
    if response
        .on_hover_text("Remove from list (Delete). Files stay on disk.")
        .clicked()
    {
        events.push(JobQueueEvent::Remove(row.id));
    }
}

/// The insertion line while a row is dragged over this one, and the move when it is dropped.
fn drop_ui(
    ui: &Ui,
    response: &Response,
    rect: Rect,
    row: &SidebarRow,
    next: Option<JobId>,
    events: &mut Vec<JobQueueEvent>,
) {
    let above = ui
        .ctx()
        .pointer_hover_pos()
        .is_some_and(|pointer| pointer.y < rect.center().y);
    if response
        .dnd_hover_payload::<JobId>()
        .is_some_and(|dragged| *dragged != row.id)
    {
        let y = if above { rect.top() } else { rect.bottom() };
        ui.painter().hline(
            (rect.left() + 8.0)..=(rect.right() - 8.0),
            y,
            Stroke::new(2.0, palette(ui).accent_fill),
        );
    }
    if let Some(dragged) = response.dnd_release_payload::<JobId>() {
        let before = if above { Some(row.id) } else { next };
        if *dragged != row.id && before != Some(*dragged) {
            events.push(JobQueueEvent::MoveBefore(*dragged, before));
        }
    }
}

/// The status mark of `row`, whose job is `item`; `running` is a running job's share done.
fn status_icon(row: &SidebarRow, item: &QueueItem, running: Option<f32>) -> StatusIcon {
    match (&item.state, running) {
        (_, Some(share)) => StatusIcon::Running(share),
        (JobState::Waiting, _) => StatusIcon::Waiting,
        (JobState::Failed(_), _) => StatusIcon::Failed,
        (JobState::Cancelled { .. }, _) => StatusIcon::Cancelled,
        _ if row.fold.is_some() => StatusIcon::Working,
        (JobState::Finished(result), _) if !result.failures.is_empty() => StatusIcon::Warning,
        _ => StatusIcon::Done,
    }
}

/// `text` on one line of at most `width`, cut with an ellipsis.
fn line(ui: &Ui, text: &str, size: f32, colour: Color32, width: f32) -> Arc<Galley> {
    let mut job =
        LayoutJob::simple_singleline(text.to_string(), FontId::proportional(size), colour);
    job.wrap = TextWrapping::truncate_at_width(width);
    ui.painter().layout_job(job)
}
