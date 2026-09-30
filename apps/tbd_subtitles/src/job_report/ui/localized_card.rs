//! The Overview's localized video card: the copy of the video with its writing replaced in
//! English, saved beside the original, and its subtitle file.
//!
//! **Role:** draw the head, the video's path and its subtitle file's path on the well, and turn
//! Open in Player, Show in Folder and Copy Path into `ReportEvent`s.
//!
//! **Position:** drawn by `overview` right after the file card; shares the file card's head and
//! path shortening.
//!
//! **Signals and state:** none, but for copying the video's path to the clipboard.
//!
//! **Invariants:** drawn only while the localized video is on disk; each path shows its folder
//! and file, the whole path on hover; the buttons act on the localized video, never the source.

use std::path::Path;

use eframe::egui::Ui;

use crate::core::ui::button::Button;
use crate::core::ui::card::{card, well_text};
use crate::core::ui::icons;
use crate::core::ui::palette::palette;
use crate::job_report::events::ReportEvent;
use crate::job_report::ui::file_card::short_path;
use crate::job_report::ui::overview::head_ui;

/// Draw the card of the localized `video` and its `subtitles`, when that file is there.
pub(super) fn localized_card_ui(
    ui: &mut Ui,
    video: &Path,
    subtitles: Option<&Path>,
    events: &mut Vec<ReportEvent>,
) {
    let p = palette(ui);
    card(ui, true, |ui| {
        head_ui(
            ui,
            (icons::FILM_STRIP, p.good_icon),
            "Localized video saved next to the original",
            "The Japanese writing is replaced in English where it could be; its subtitle file \
             holds the dialogue and sound cues. The original video is unchanged.",
            |_| {},
        );
        for path in std::iter::once(video).chain(subtitles) {
            ui.scope(|ui| well_text(ui, &short_path(path), false))
                .response
                .on_hover_text(path.display().to_string());
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let open = Button::new("Open in Player").icon(icons::MONITOR_PLAY);
            if open.show(ui).clicked() {
                events.push(ReportEvent::OpenVideo(video.to_path_buf()));
            }
            let folder = Button::new("Show in Folder").icon(icons::FOLDER);
            if folder.show(ui).clicked() {
                events.push(ReportEvent::ShowInFolder(video.to_path_buf()));
            }
            if Button::new("Copy Path")
                .icon(icons::COPY)
                .show(ui)
                .clicked()
            {
                ui.ctx().copy_text(video.display().to_string());
                events.push(ReportEvent::Copied);
            }
        });
    });
}
