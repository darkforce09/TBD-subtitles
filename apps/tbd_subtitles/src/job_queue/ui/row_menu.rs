//! A row's right-click menu: what can be done with the video from where it stands.
//!
//! **Role:** list the commands of a row's state (move a waiting video, cancel a running one or
//! the correction run updating a finished one, check, open, show, copy or run again a finished
//! one, try a failed or cancelled one again, remove any that is not running) and turn the one
//! chosen into a `JobQueueEvent`.
//!
//! **Position:** opened by `sidebar_row` on a right click, inside egui's context menu popup.
//!
//! **Signals and state:** Copy Subtitle Path puts the path on the clipboard itself, then reports
//! it; a finished video's subtitle file is looked up beside it only while the menu is open.
//!
//! **Invariants:** a running row offers no Remove; a command that cannot apply is shown disabled,
//! not hidden.

use std::path::PathBuf;

use eframe::egui::{
    Align2, Color32, CornerRadius, FontId, Sense, Stroke, Ui, WidgetInfo, WidgetType, pos2, vec2,
};

use crate::core::ui::icons;
use crate::core::ui::palette::palette;
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::queue::{JobKind, JobState, Move, QueueItem};
use crate::job_queue::models::sidebar::SidebarRow;
use crate::job_queue::models::view::JobQueueView;
use crate::job_queue::services::video_files;

/// One command of the menu.
struct Command {
    glyph: &'static str,
    label: &'static str,
    event: Option<JobQueueEvent>,
    /// Text to put on the clipboard before the event.
    copy: Option<PathBuf>,
    /// A command that removes something: drawn red.
    danger: bool,
    /// The key that does the same.
    key: Option<&'static str>,
}

impl Command {
    fn new(glyph: &'static str, label: &'static str, event: Option<JobQueueEvent>) -> Command {
        Command {
            glyph,
            label,
            event,
            copy: None,
            danger: false,
            key: None,
        }
    }
}

/// Draw the menu of `row`, whose job is `item`, and push the command chosen onto `events`.
pub(crate) fn row_menu_ui(
    ui: &mut Ui,
    view: &JobQueueView<'_>,
    row: &SidebarRow,
    item: &QueueItem,
    events: &mut Vec<JobQueueEvent>,
) {
    ui.spacing_mut().item_spacing.y = 0.0;
    for (at, group) in groups(view, row, item).into_iter().enumerate() {
        if at > 0 {
            separator(ui);
        }
        for command in group {
            if command_ui(ui, &command) {
                if let Some(path) = &command.copy {
                    ui.ctx().copy_text(path.display().to_string());
                }
                if let Some(event) = command.event {
                    events.push(event);
                }
                ui.close();
            }
        }
    }
}

/// The menu's commands, in groups set apart by lines.
fn groups(view: &JobQueueView<'_>, row: &SidebarRow, item: &QueueItem) -> Vec<Vec<Command>> {
    let id = row.id;
    let video = || Some(row.video.clone());
    let mut groups = Vec::new();
    match &item.state {
        JobState::Waiting if item.kind == JobKind::Full => {
            let waiting = view
                .queue
                .items
                .iter()
                .filter(|other| other.kind == JobKind::Full && other.state.is_waiting())
                .count();
            let first = row.place == Some(1);
            let last = row.place == Some(waiting);
            let step = |glyph, label, to, can: bool| {
                Command::new(glyph, label, can.then_some(JobQueueEvent::Move(id, to)))
            };
            groups.push(vec![
                step(icons::ARROW_LINE_UP, "Run Next", Move::Top, !first),
                step(icons::ARROW_UP, "Move Up", Move::Up, !first),
                step(icons::ARROW_DOWN, "Move Down", Move::Down, !last),
            ]);
        }
        JobState::Running(_) => groups.push(vec![Command::new(
            icons::STOP_CIRCLE,
            "Cancel",
            Some(JobQueueEvent::Cancel(id)),
        )]),
        _ => {}
    }
    if let Some(update) = row.fold.and_then(|fold| fold.running) {
        groups.push(vec![Command::new(
            icons::STOP_CIRCLE,
            "Stop Updating Subtitles",
            Some(JobQueueEvent::Cancel(update)),
        )]);
    }
    let finished = matches!(item.state, JobState::Finished(_) | JobState::FinishedBefore);
    let mut open = Vec::new();
    if finished {
        open.push(Command::new(
            icons::EAR,
            "Check Lines",
            Some(JobQueueEvent::CheckLines(id)),
        ));
        open.push(Command::new(
            icons::MONITOR_PLAY,
            "Open in Player",
            video().map(JobQueueEvent::OpenVideo),
        ));
    }
    open.push(Command::new(
        icons::FOLDER,
        "Show in Folder",
        video().map(JobQueueEvent::Reveal),
    ));
    if finished {
        let subtitles = match &item.state {
            JobState::Finished(result) => Some(result.subtitles.clone()),
            _ => video_files::subtitle_file(&item.video),
        };
        let mut copy = Command::new(
            icons::COPY,
            "Copy Subtitle Path",
            subtitles.is_some().then_some(JobQueueEvent::Copied),
        );
        copy.copy = subtitles;
        open.push(copy);
    }
    groups.push(open);
    match &item.state {
        JobState::Finished(_) | JobState::FinishedBefore => groups.push(vec![Command::new(
            icons::ARROW_CLOCKWISE,
            "Run Again with Current Settings",
            Some(JobQueueEvent::RunAgain(id)),
        )]),
        JobState::Failed(_) | JobState::Cancelled { .. } => groups.push(vec![Command::new(
            icons::ARROW_CLOCKWISE,
            "Try Again",
            Some(JobQueueEvent::TryAgain(id, None)),
        )]),
        _ => {}
    }
    if row.removable {
        let mut remove = Command::new(
            icons::X,
            "Remove from List",
            Some(JobQueueEvent::Remove(id)),
        );
        remove.danger = true;
        remove.key = Some("Delete");
        groups.push(vec![remove]);
    }
    groups
}

/// One command's line; whether it was chosen.
fn command_ui(ui: &mut Ui, command: &Command) -> bool {
    let p = palette(ui);
    let enabled = command.event.is_some();
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), sense);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, command.label));
    let lit = enabled && response.hovered();
    if lit {
        let fill = if command.danger {
            p.bad_icon
        } else {
            p.accent_fill
        };
        ui.painter().rect_filled(rect, CornerRadius::same(5), fill);
    }
    let colour = match (lit, command.danger) {
        (true, _) => Color32::WHITE,
        (false, true) => p.bad,
        (false, false) => p.text,
    };
    let colour = if enabled {
        colour
    } else {
        colour.gamma_multiply(0.4)
    };
    let painter = ui.painter();
    painter.text(
        pos2(rect.left() + 16.0, rect.center().y),
        Align2::CENTER_CENTER,
        command.glyph,
        icons::font(14.0),
        colour,
    );
    painter.text(
        pos2(rect.left() + 31.0, rect.center().y),
        Align2::LEFT_CENTER,
        command.label,
        FontId::proportional(13.0),
        colour,
    );
    if let Some(key) = command.key {
        painter.text(
            pos2(rect.right() - 9.0, rect.center().y),
            Align2::RIGHT_CENTER,
            key,
            FontId::proportional(11.5),
            colour.gamma_multiply(0.7),
        );
    }
    response.clicked()
}

/// The line between two groups of commands.
fn separator(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::hover());
    ui.painter().hline(
        (rect.left() + 6.0)..=(rect.right() - 6.0),
        rect.center().y,
        Stroke::new(1.0, palette(ui).line),
    );
}
