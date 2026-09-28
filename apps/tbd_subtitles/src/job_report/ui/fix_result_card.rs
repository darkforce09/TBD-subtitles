//! The Overview's Fix It result: a green note under the file card's head saying what Claude
//! changed, which problems it cleared and which are left, a few of its changes, what the owner did
//! with them, and See Changes.
//!
//! **Role:** draw the borrowed report's `FixResult` and turn its buttons into `ReportEvent`s.
//!
//! **Position:** drawn by `file_card` right after its head while Fix It is not under way on the
//! video; the note itself is `file_card::note_ui`.
//!
//! **Signals and state:** none; reads egui's clock to animate a run that has just finished, and
//! asks for frames until the animation settles.
//!
//! **Invariants:** from the moment the run finished the seal grows from 0.6 of its size to its
//! whole size in 0.35 s, easing out, and the green fades from strong to its usual tint in 2 s,
//! with the note's layout still meanwhile; each problem left keeps its fix and its button; the
//! examples are plain words in the note's own font; See Changes shows only when Claude changed a
//! line.

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{Align2, Color32, FontId, Label, RichText, Sense, Ui, Vec2};

use crate::core::format;
use crate::core::ui::button::{Button, ButtonSize};
use crate::core::ui::icons;
use crate::core::ui::palette::palette;
use crate::job_report::events::{LinesToCheck, ReportEvent};
use crate::job_report::models::finding_group::LineGroup;
use crate::job_report::models::fix_result::{FixResult, change_words};
use crate::job_report::ui::file_card::{note_ui, remedy_button};

/// The seal's square, the share of it the seal starts at, and how long it grows.
const SEAL: f32 = 20.0;
const SEAL_START: f32 = 0.6;
const SEAL_GROW_S: f64 = 0.35;
/// How long the strong green takes to fade to the usual tint.
const TINT_FADE_S: f64 = 2.0;
/// How much of the icon green the strong tint takes.
const STRONG_TINT: f32 = 0.35;
/// The space between a cleared problem's check and its words.
const CHECK_GAP: f32 = 5.0;

/// Draw `result`; `unchecked` is how many of Claude's changes wait for the owner, and
/// `fixed_at` egui's time when the run finished, while it has just finished.
pub(super) fn fix_result_ui(
    ui: &mut Ui,
    result: &FixResult,
    unchecked: usize,
    fixed_at: Option<f64>,
    events: &mut Vec<ReportEvent>,
) {
    let p = palette(ui);
    let since = fixed_at.map(|at| (ui.input(|input| input.time) - at).max(0.0));
    let grown = since.map_or(1.0, |s| ease_out((s / SEAL_GROW_S).min(1.0) as f32));
    let faded = since.map_or(1.0, |s| (s / TINT_FADE_S).min(1.0) as f32);
    if since.is_some_and(|s| s < TINT_FADE_S) {
        ui.ctx().request_repaint();
    }
    let strong = p.good_icon.gamma_multiply(STRONG_TINT);
    let fill = strong.lerp_to_gamma(p.good_tint, faded);
    let seal = |ui: &mut Ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(SEAL), Sense::hover());
        let size = SEAL * (SEAL_START + (1.0 - SEAL_START) * grown);
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            icons::SEAL_CHECK,
            icons::font(size),
            p.good_icon,
        );
    };
    let mut see_changes = false;
    let see = |ui: &mut Ui| {
        if result.changed == 0 {
            return;
        }
        let button = Button::new("See Changes")
            .icon(icons::MAGIC_WAND)
            .size(ButtonSize::Small);
        see_changes = button.show(ui).clicked();
    };
    let title = format!("Fixed by {}", result.model);
    note_ui(ui, fill, seal, &title, see, |ui| {
        body_ui(ui, result, events)
    });
    if see_changes {
        // Once the owner kept or undid every change, Claude's group is empty: the lines to
        // check show instead.
        let lines = if unchecked > 0 {
            LinesToCheck::Group(LineGroup::ChangedByFixIt)
        } else {
            LinesToCheck::Flagged
        };
        events.push(ReportEvent::CheckLines(lines));
    }
}

/// The counts, the problems cleared and left, the examples and what the owner did.
fn body_ui(ui: &mut Ui, result: &FixResult, events: &mut Vec<ReportEvent>) {
    let p = palette(ui);
    let small = |text: String, colour: Color32| {
        Label::new(RichText::new(text).size(12.0).color(colour)).wrap()
    };
    let right = if result.already_right == 1 {
        "1 was already right".to_string()
    } else {
        format!("{} were already right", result.already_right)
    };
    let counts = format!(
        "{} changed · {right}",
        format::plural(result.changed, "line")
    );
    ui.add(small(counts, p.text2));
    for problem in &result.cleared {
        let mut line = LayoutJob::default();
        let glyph = TextFormat::simple(icons::font(12.0), p.good_icon);
        line.append(icons::CHECK, 0.0, glyph);
        let words = TextFormat::simple(FontId::proportional(12.0), p.good_icon);
        line.append(&problem.cleared_title(), CHECK_GAP, words);
        ui.add(Label::new(line).wrap());
    }
    for problem in &result.left {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let title = format!("Claude could not fix: {}", problem.title());
            ui.add(small(title, p.warn));
            remedy_button(ui, *problem, events);
        });
        ui.add(small(problem.fix().to_string(), p.text2));
    }
    for (before, after) in &result.examples {
        ui.add(small(change_words(before, after), p.text));
    }
    if result.more > 0 {
        ui.add(small(format!("and {} more", result.more), p.text3));
    }
    let mut owner = Vec::new();
    if result.owner_kept > 0 {
        owner.push(format!("You kept {}", result.owner_kept));
    }
    if result.owner_undid > 0 {
        let undid = format!("undid {}", result.owner_undid);
        owner.push(if owner.is_empty() {
            format!("You {undid}")
        } else {
            undid
        });
    }
    if !owner.is_empty() {
        ui.add(small(owner.join(" · "), p.text2));
    }
    let mut aside = Vec::new();
    if result.turned_down > 0 {
        let changes = format::plural(result.turned_down, "change");
        aside.push(format!("{changes} turned down on checking"));
    }
    if result.kept_yours > 0 {
        let lines = format::plural(result.kept_yours, "line");
        aside.push(format!("{lines} kept your own correction"));
    }
    if !aside.is_empty() {
        ui.add(small(aside.join(" · "), p.text3));
    }
}

/// `t` from 0 to 1 eased out: fast at first, settling at the end.
fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}
