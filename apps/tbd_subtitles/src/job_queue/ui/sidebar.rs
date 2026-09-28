//! The sidebar: the videos in the sections Now, Up Next and Done, each under a heading with its
//! count that folds it away, or a hint when the list is empty.
//!
//! **Role:** draw the rows `sidebar_rows` builds, section by section, and keep which sections
//! are folded away.
//!
//! **Position:** called by the application's frame for the left panel; draws `sidebar_row` for
//! each row; the application's shortcuts read `row_order`.
//!
//! **Signals and state:** whether each section is folded away lives in egui's memory.
//!
//! **Invariants:** the arrow keys move through exactly the rows drawn, in the order drawn.

use eframe::egui::{
    Align2, Context, FontFamily, FontId, Id, RichText, ScrollArea, Sense, TextFormat, Ui,
    WidgetInfo, WidgetType, pos2, text::LayoutJob, vec2,
};

use crate::core::ui::fonts;
use crate::core::ui::icons;
use crate::core::ui::palette::palette;
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::queue::{JobId, Queue};
use crate::job_queue::models::sidebar::{Section, SidebarRow};
use crate::job_queue::models::view::JobQueueView;
use crate::job_queue::services::sidebar_rows;
use crate::job_queue::ui::sidebar_row::sidebar_row_ui;

/// Draw the sidebar from `view` and push what the owner asked for onto `events`.
pub(crate) fn sidebar_ui(ui: &mut Ui, view: &JobQueueView<'_>, events: &mut Vec<JobQueueEvent>) {
    let rows = sidebar_rows::rows(view.queue);
    if rows.is_empty() {
        empty_ui(ui);
        return;
    }
    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for section in Section::ALL {
            let shown: Vec<&SidebarRow> = rows.iter().filter(|r| r.section == section).collect();
            if shown.is_empty() {
                continue;
            }
            if !heading_ui(ui, section, shown.len()) {
                continue;
            }
            for (at, row) in shown.iter().enumerate() {
                let next = shown.get(at + 1).map(|next| next.id);
                sidebar_row_ui(ui, view, row, next, events);
            }
        }
        ui.add_space(16.0);
    });
}

/// The rows the arrow keys move through: those of the sections not folded away, in order.
pub(crate) fn row_order(ctx: &Context, queue: &Queue) -> Vec<JobId> {
    sidebar_rows::rows(queue)
        .into_iter()
        .filter(|row| !folded(ctx, row.section))
        .map(|row| row.id)
        .collect()
}

fn folded_id(section: Section) -> Id {
    Id::new(("sidebar-folded", section))
}

fn folded(ctx: &Context, section: Section) -> bool {
    ctx.data(|data| data.get_temp::<bool>(folded_id(section)))
        .unwrap_or(false)
}

/// A section's heading: a chevron, its title in capitals and its count; a click folds the
/// section away or opens it. Whether the section is open.
fn heading_ui(ui: &mut Ui, section: Section, count: usize) -> bool {
    let p = palette(ui);
    ui.add_space(6.0);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
    let title = section.title().to_uppercase();
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &title));
    let mut open = !folded(ui.ctx(), section);
    if response.clicked() {
        open = !open;
        ui.ctx()
            .data_mut(|data| data.insert_temp(folded_id(section), !open));
    }
    let chevron = if open {
        icons::CARET_DOWN
    } else {
        icons::CARET_RIGHT
    };
    ui.painter().text(
        pos2(rect.left() + 12.0, rect.center().y),
        Align2::CENTER_CENTER,
        chevron,
        icons::font(12.0),
        p.text2,
    );
    let semibold = FontFamily::Name(fonts::SEMIBOLD.into());
    let mut job = LayoutJob::default();
    job.append(
        &title,
        0.0,
        TextFormat {
            font_id: FontId::new(11.0, semibold.clone()),
            color: p.text2,
            extra_letter_spacing: 0.44,
            ..Default::default()
        },
    );
    let galley = ui.painter().layout_job(job);
    let at = pos2(rect.left() + 22.0, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(at, galley, p.text2);
    ui.painter().text(
        pos2(rect.right() - 6.0, rect.center().y),
        Align2::RIGHT_CENTER,
        count.to_string(),
        FontId::new(11.0, semibold),
        p.text2,
    );
    open
}

/// The empty list: a film strip and how to add videos.
fn empty_ui(ui: &mut Ui) {
    let p = palette(ui);
    ui.add_space(28.0);
    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.label(
            RichText::new(icons::FILM_STRIP)
                .font(icons::font(28.0))
                .color(p.text3),
        );
        let semibold = FontFamily::Name(fonts::SEMIBOLD.into());
        ui.label(
            RichText::new("No videos yet")
                .font(FontId::new(12.0, semibold))
                .color(p.text2),
        );
        ui.label(
            RichText::new("Add videos with the toolbar, or drop them anywhere on the window.")
                .size(12.0)
                .color(p.text2),
        );
    });
}
