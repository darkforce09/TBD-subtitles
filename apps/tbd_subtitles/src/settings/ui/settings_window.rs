//! The Settings window's content: its six tabs, the open tab, and the footer that says how
//! changes apply.
//!
//! **Role:** draw the tab bar across the top on the toolbar grey, the open tab in a scrolling body,
//! and the footer across the bottom, and turn a tab's click into `SettingsEvent::Open`.
//!
//! **Position:** called by the application inside the Settings window's viewport; draws
//! `general_tab`, `automation_tab`, `engines_tab`, `onscreen_text`, `models_tab` and `machine_tab`.
//!
//! **Signals and state:** none; reads the borrowed page and returns events.
//!
//! **Invariants:** nothing is written from here: each change is an event the application applies
//! at once; a tab button is at least 88 px wide, an icon over its name.

use eframe::egui::{
    Align2, CentralPanel, CornerRadius, FontId, Frame, Id, Label, Margin, Panel, RichText,
    ScrollArea, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2,
};

use super::form::ROW_GAP;
use super::{automation_tab, engines_tab, general_tab, machine_tab, models_tab};
use crate::core::ui::icons;
use crate::core::ui::palette::palette;
use crate::settings::events::SettingsEvent;
use crate::settings::models::page::{SettingsPage, SettingsTab};

#[path = "onscreen_text.rs"]
mod onscreen_text;

/// The footer's words.
const FOOTER: &str = "Changes save as you make them. They apply to videos that haven't started.";
/// A tab button's least width and height, and the space between the buttons.
const TAB_WIDTH: f32 = 88.0;
const TAB_HEIGHT: f32 = 49.0;
const TAB_GAP: f32 = 4.0;

/// Draw the Settings window's content on `tab` and push what the owner asked for onto `events`;
/// while it is `closing`, what is still being typed in a field is sent.
pub(crate) fn settings_window_ui(
    ui: &mut Ui,
    page: &SettingsPage,
    tab: SettingsTab,
    closing: bool,
    events: &mut Vec<SettingsEvent>,
) {
    let p = palette(ui);
    Panel::top(Id::new("settings-tabs"))
        .frame(Frame::new().fill(p.toolbar).inner_margin(Margin::same(8)))
        .show(ui, |ui| tab_bar_ui(ui, tab, events));
    Panel::bottom(Id::new("settings-footer"))
        .frame(
            Frame::new()
                .fill(p.toolbar)
                .inner_margin(Margin::symmetric(24, 10)),
        )
        .show(ui, footer_ui);
    CentralPanel::default()
        .frame(Frame::new().fill(p.window))
        .show(ui, |ui| {
            ScrollArea::vertical()
                .id_salt(("settings-body", tab.title()))
                .auto_shrink(false)
                .show(ui, |ui| {
                    Frame::new()
                        .inner_margin(Margin {
                            left: 24,
                            right: 24,
                            top: 18,
                            bottom: 16,
                        })
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.spacing_mut().item_spacing.y = ROW_GAP;
                            match tab {
                                SettingsTab::General => general_tab::general_ui(ui, page, events),
                                SettingsTab::Automation => {
                                    automation_tab::automation_ui(ui, page, events);
                                }
                                SettingsTab::Engines => {
                                    engines_tab::engines_ui(ui, page, closing, events);
                                }
                                SettingsTab::OnscreenText => {
                                    onscreen_text::onscreen_text_ui(ui, page, events);
                                }
                                SettingsTab::Models => models_tab::models_ui(ui, page, events),
                                SettingsTab::ThisComputer => {
                                    machine_tab::machine_ui(ui, page, events);
                                }
                            }
                        });
                });
        });
}

/// The tabs, centred, the open one in the accent tint.
fn tab_bar_ui(ui: &mut Ui, open: SettingsTab, events: &mut Vec<SettingsEvent>) {
    let p = palette(ui);
    let label_font = FontId::proportional(11.5);
    let widths: Vec<f32> = SettingsTab::ALL
        .iter()
        .map(|tab| {
            let galley =
                ui.painter()
                    .layout_no_wrap(tab.title().to_string(), label_font.clone(), p.text2);
            (galley.size().x + 20.0).max(TAB_WIDTH)
        })
        .collect();
    let total = widths.iter().sum::<f32>() + TAB_GAP * (widths.len() - 1) as f32;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = TAB_GAP;
        ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
        for (tab, width) in SettingsTab::ALL.into_iter().zip(widths) {
            let (rect, response) = ui.allocate_exact_size(vec2(width, TAB_HEIGHT), Sense::click());
            let chosen = tab == open;
            response.widget_info(|| {
                WidgetInfo::selected(WidgetType::Button, true, chosen, tab.title())
            });
            if chosen {
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(7), p.accent_tint);
            } else if response.hovered() {
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(7), p.hover);
            }
            let colour = if chosen { p.accent } else { p.text2 };
            ui.painter().text(
                pos2(rect.center().x, rect.top() + 16.0),
                Align2::CENTER_CENTER,
                glyph(tab),
                icons::font(20.0),
                colour,
            );
            ui.painter().text(
                pos2(rect.center().x, rect.top() + 29.0),
                Align2::CENTER_TOP,
                tab.title(),
                label_font.clone(),
                colour,
            );
            if response.clicked() && !chosen {
                events.push(SettingsEvent::Open(tab));
            }
        }
    });
}

fn glyph(tab: SettingsTab) -> &'static str {
    match tab {
        SettingsTab::General => icons::GEAR,
        SettingsTab::Automation => icons::LIGHTNING,
        SettingsTab::Engines => icons::SLIDERS,
        SettingsTab::OnscreenText => icons::FILE_TEXT,
        SettingsTab::Models => icons::PACKAGE,
        SettingsTab::ThisComputer => icons::MONITOR,
    }
}

/// The info mark and the footer's words, 12 px grey.
fn footer_ui(ui: &mut Ui) {
    let p = palette(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        ui.label(
            RichText::new(icons::INFO)
                .font(icons::font(14.0))
                .color(p.text2),
        );
        ui.add(Label::new(RichText::new(FOOTER).size(12.0).color(p.text2)).truncate());
    });
}
