//! The window's middle while the list is empty: a card with a dashed border that says to drop
//! videos here, or add them with its buttons.
//!
//! **Role:** draw the card, centred in the space it is given, and turn its buttons into
//! `JobQueueEvent`s.
//!
//! **Position:** called by the application's detail pane when the queue holds no job.
//!
//! **Signals and state:** the card's height and its buttons' width from the last frame live in
//! egui's memory, so both are centred.
//!
//! **Invariants:** while a model is missing the card says that the models download first.

use std::f32::consts::FRAC_PI_2;

use eframe::egui::{
    Align, CornerRadius, FontFamily, FontId, Frame, Label, Layout, Margin, Pos2, Rect, RichText,
    Shape, Stroke, Ui, UiBuilder, pos2, vec2,
};

use crate::core::ui::button::{Button, ButtonSize};
use crate::core::ui::fonts;
use crate::core::ui::icons;
use crate::core::ui::palette::palette;
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::view::JobQueueView;

/// The card's widest, its corner radius and the space inside its border.
const WIDTH: f32 = 520.0;
const RADIUS: f32 = 16.0;
const PAD_X: f32 = 32.0;
const PAD_Y: f32 = 44.0;

/// Draw the card in the middle of `ui` and push the button pressed onto `events`.
pub(crate) fn empty_state_ui(
    ui: &mut Ui,
    view: &JobQueueView<'_>,
    events: &mut Vec<JobQueueEvent>,
) {
    let p = palette(ui);
    let area = ui.available_rect_before_wrap();
    let height_id = ui.id().with("empty-card-height");
    let height = ui
        .ctx()
        .data(|data| data.get_temp::<f32>(height_id))
        .unwrap_or(320.0);
    let width = WIDTH.min(area.width() - 2.0 * PAD_X).max(240.0);
    let rect = Rect::from_center_size(area.center(), vec2(width, height));
    let builder = UiBuilder::new()
        .max_rect(rect)
        .layout(Layout::top_down(Align::Center));
    let card = ui.scope_builder(builder, |ui| {
        Frame::new()
            .fill(p.card)
            .corner_radius(CornerRadius::same(RADIUS as u8))
            .inner_margin(Margin::symmetric(PAD_X as i8, PAD_Y as i8))
            .show(ui, |ui| {
                ui.set_width(width - 2.0 * PAD_X);
                ui.spacing_mut().item_spacing.y = 10.0;
                ui.label(
                    RichText::new(icons::FILM_STRIP)
                        .font(icons::font(40.0))
                        .color(p.accent),
                );
                let semibold = FontFamily::Name(fonts::SEMIBOLD.into());
                ui.label(
                    RichText::new("Drop videos here")
                        .font(FontId::new(17.0, semibold))
                        .color(p.text),
                );
                ui.scope(|ui| {
                    ui.set_max_width(340.0);
                    ui.add(
                        Label::new(
                            RichText::new(
                                "Or add them with the buttons below. Each video gets English \
                                 subtitles, saved next to it with the same name.",
                            )
                            .color(p.text2),
                        )
                        .halign(Align::Center)
                        .wrap(),
                    );
                });
                if view.models_missing {
                    ui.label(
                        RichText::new("The models download first; you can add videos now.")
                            .color(p.warn),
                    );
                }
                ui.add_space(6.0);
                buttons_ui(ui, events);
            })
            .response
            .rect
    });
    let drawn = card.inner;
    ui.ctx()
        .data_mut(|data| data.insert_temp(height_id, drawn.height()));
    ui.painter().extend(Shape::dashed_line(
        &outline(drawn.shrink(1.0), RADIUS),
        Stroke::new(2.0, p.line_strong),
        6.0,
        5.0,
    ));
}

/// Add Videos… and Add Folder…, centred side by side.
fn buttons_ui(ui: &mut Ui, events: &mut Vec<JobQueueEvent>) {
    let width_id = ui.id().with("empty-card-buttons");
    let width = ui
        .ctx()
        .data(|data| data.get_temp::<f32>(width_id))
        .unwrap_or(260.0);
    let row = ui.allocate_ui_with_layout(
        vec2(width, 32.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let videos = Button::new("Add Videos…")
                .icon(icons::PLUS)
                .primary(true)
                .size(ButtonSize::Large)
                .show(ui);
            if videos.clicked() {
                events.push(JobQueueEvent::AddVideos);
            }
            let folder = Button::new("Add Folder…")
                .icon(icons::FOLDER_PLUS)
                .size(ButtonSize::Large)
                .show(ui);
            if folder.clicked() {
                events.push(JobQueueEvent::AddFolder);
            }
            ui.min_rect().width()
        },
    );
    ui.ctx()
        .data_mut(|data| data.insert_temp(width_id, row.inner));
}

/// The outline of `rect` with its corners rounded by `radius`, closed, clockwise.
fn outline(rect: Rect, radius: f32) -> Vec<Pos2> {
    let corners = [
        (pos2(rect.right() - radius, rect.top() + radius), -FRAC_PI_2),
        (pos2(rect.right() - radius, rect.bottom() - radius), 0.0),
        (
            pos2(rect.left() + radius, rect.bottom() - radius),
            FRAC_PI_2,
        ),
        (
            pos2(rect.left() + radius, rect.top() + radius),
            2.0 * FRAC_PI_2,
        ),
    ];
    let mut points: Vec<Pos2> = corners
        .iter()
        .flat_map(|&(centre, start)| {
            (0..=8).map(move |i| {
                let angle = start + FRAC_PI_2 * i as f32 / 8.0;
                pos2(
                    centre.x + radius * angle.cos(),
                    centre.y + radius * angle.sin(),
                )
            })
        })
        .collect();
    if let Some(&first) = points.first() {
        points.push(first);
    }
    points
}
