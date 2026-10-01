//! Snapshot scenes of the real window, rendered offscreen for visual review.
//!
//! **Role:** drive `TbdSubtitlesApp` through scripted scenes (clicks by label, light and dark,
//! fonts installed) and write one PNG per scene and scheme at 1280 by 800.
//!
//! **Position:** a child of the rendering tests; an ignored test, run on the host with
//! `TBD_SNAPSHOTS=<folder> cargo test -p tbd_subtitles -- --ignored window_snapshots`.
//!
//! **Signals and state:** reads the owner's Dressrosa 11 and 15–17 work folders and copies their
//! job databases into a scratch folder; writes PNGs only to `$TBD_SNAPSHOTS`, never into the repo;
//! the Check Lines scenes write Dressrosa 15's corrections into the scratch copy and decode still
//! frames of its video with FFmpeg; the Fix It scene writes Dressrosa 17's Fix It record and
//! corrections into the scratch copy through a stand-in Fix It.
//!
//! **Invariants:** the owner's work folders and videos are only read, and the settings file
//! written is the scratch one; the app runs over the scratch copy with a stand-in runner, and a
//! running, failed or cancelled job, a download and the machine checks are set by hand, so no job
//! or download starts; the correction run of the Check Lines scenes waits until it is stopped;
//! no `claude` runs.

use std::path::Path;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use job_model::report::QcCheck;

use super::*;
use crate::log_console::events::LogConsoleEvent;

/// The work folders the scenes are built from.
const EPISODES: [&str; 4] = ["11", "15", "16", "17"];
/// Episodes queued in the running scene with no work folder: they only wait.
const WAITING: [u32; 9] = [12, 13, 14, 18, 19, 20, 21, 22, 23];
/// The file a report and a review read from a work folder: the job's database.
const JOB_DATABASE: &str = "job.redb";

#[test]
#[ignore = "renders PNGs for visual review; needs a GPU"]
fn window_snapshots() {
    let out = std::env::var_os("TBD_SNAPSHOTS")
        .map(PathBuf::from)
        .expect("TBD_SNAPSHOTS names the folder for the PNGs");
    std::fs::create_dir_all(&out).expect("the snapshot folder");
    let root = std::env::temp_dir().join(format!("tbd-snapshots-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let videos = copy_work_folders(&root.join("work"));
    finished_scenes(&root, &out, &videos);
    settings_scenes(&root, &out, &videos);
    first_run_scene(&root.join("first-run"), &out);
    banner_scenes(&root.join("banners"), &out);
    queue_scenes(&root, &out, &videos);
    detail_scenes(&root, &out, &videos);
    attention_scene(&root, &out, &videos);
    review_scenes(&root, &out, &videos);
    fix_it_done_scene(&root, &out, &videos);
    let _ = std::fs::remove_dir_all(&root);
}

/// The window over `root` with `setup` applied to the app before the first frame.
fn harness(
    root: &Path,
    setup: impl FnOnce(&mut TbdSubtitlesApp),
) -> Harness<'static, TbdSubtitlesApp> {
    harness_with(root, stand_in(), setup)
}

/// As `harness`, its jobs run by `run`.
fn harness_with(
    root: &Path,
    run: RunJob,
    setup: impl FnOnce(&mut TbdSubtitlesApp),
) -> Harness<'static, TbdSubtitlesApp> {
    let root = root.to_path_buf();
    Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .wgpu()
        .build_eframe(move |creation| {
            theme::install(&creation.egui_ctx);
            let mut app = TbdSubtitlesApp::new(Environment::scratch(&root, run), Vec::new());
            setup(&mut app);
            app
        })
}

/// A stand-in whose run lasts until it is stopped: a correction run still updating the subtitles.
fn until_stopped() -> RunJob {
    Arc::new(|_video, options, progress| {
        progress(Progress::StepStarted(StepName::Review));
        while !options.cancel.is_cancelled() {
            std::thread::sleep(Duration::from_millis(20));
        }
        Err(PipelineError::cancelled("step review"))
    })
}

/// Dressrosa 15's Check Lines: the first line to check, after Use (edited), after Looks Right
/// (its correction run updating the subtitles), narrowed to "Heard word replaced", and with every
/// line checked. It writes 15's corrections into the scratch copy, so it runs last.
fn review_scenes(root: &Path, out: &Path, videos: &[PathBuf]) {
    use crate::job_report::events::{LinesToCheck, ReportEvent};
    use crate::job_report::models::finding_group::LineGroup;
    use crate::line_review::events::ReviewEvent;
    use crate::line_review::services::line_filter;
    let _ = std::fs::remove_dir_all(root.join("data"));
    let mut harness = harness_with(root, until_stopped(), all_finished(videos));
    harness.get_by_label("[Muhn Pace] Dressrosa 15").click();
    harness.run_steps(2);
    // The header's tab, above the lines card's button of the same name.
    harness
        .get_all_by_label("Check Lines")
        .min_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .expect("the header offers Check Lines")
        .click();
    harness.run_steps(2);
    wait_for_still(&mut harness);
    shoot(&mut harness, out, "check_lines_first");
    // Use the first reading that differs from the line's text.
    let tag = harness.state().review.as_ref().and_then(|(_, session)| {
        let line = line_filter::open_line(session)?;
        let now = session.current(line).text.to_lowercase();
        ["P", "W", "ALT p", "ALT w"]
            .into_iter()
            .find(|tag| line.reading(tag).is_some_and(|t| t.to_lowercase() != now))
    });
    let tag = tag.expect("a reading that differs from the line");
    let apply = |harness: &mut Harness<'_, TbdSubtitlesApp>, event: ReviewEvent| {
        harness.state_mut().apply(vec![Action::from(event)]);
    };
    apply(&mut harness, ReviewEvent::Pick(tag.to_string()));
    shoot(&mut harness, out, "after_use");
    apply(&mut harness, ReviewEvent::Discard);
    apply(&mut harness, ReviewEvent::LooksRight);
    wait_for_still(&mut harness);
    shoot(&mut harness, out, "after_looks_right");
    harness.state_mut().apply(vec![
        Action::ShowTab(DetailTab::Overview),
        Action::from(ReportEvent::CheckLines(LinesToCheck::Group(
            LineGroup::HeardWordReplaced,
        ))),
    ]);
    wait_for_still(&mut harness);
    shoot(&mut harness, out, "group_filter");
    apply(&mut harness, ReviewEvent::ClearGroup);
    let left: Vec<String> = harness
        .state()
        .review
        .as_ref()
        .map(|(_, session)| {
            line_filter::shown(session)
                .iter()
                .map(|line| line.id.clone())
                .collect()
        })
        .unwrap_or_default();
    for id in left {
        apply(&mut harness, ReviewEvent::Open(id));
        apply(&mut harness, ReviewEvent::LooksRight);
    }
    shoot(&mut harness, out, "all_checked");
    for token in harness.state().review_lanes.tokens() {
        token.cancel();
    }
}

/// Dressrosa 17's Overview just after Fix It finished, as the owner sees a whole finish: the owner
/// kept its first flagged line as it was, and a stand-in Fix It answered every other one, so
/// nothing is left to check; its result card, its toast and its row, with the pointer on no row.
/// Both write into the scratch copy's database (the corrections, the Fix It record), so the scene runs last; the
/// correction run is the stand-in runner's.
fn fix_it_done_scene(root: &Path, out: &Path, videos: &[PathBuf]) {
    use crate::job_report::events::ReportEvent;
    use job_model::outputs::{AdjudicationPass, Chosen, Correction, Corrections};
    let job = scratch_work_folder(root, "17");
    let qc: QcReport = super::job_fixtures::stored_qc(&job);
    let settled: AdjudicationPass = super::job_fixtures::stored_settled(&job);
    let first = flagged_lines(&qc)
        .into_iter()
        .next()
        .expect("a flagged line");
    let line = settled.lines.iter().find(|line| line.id == first);
    let line = line.expect("the flagged line is settled");
    let kept = Corrections {
        lines: vec![Correction {
            id: line.id.clone(),
            text: line.t.clone(),
            flags: line.f.clone(),
            chosen: Chosen::Engine("adjudicated".into()),
        }],
    };
    super::rendering_report::put_corrections(&job, &kept);
    let _ = std::fs::remove_dir_all(root.join("data"));
    let setup = all_finished(videos);
    let mut harness = harness(root, move |app| {
        setup(app);
        app.env.fix_video = scene_fix();
    });
    let id = id_of(harness.state(), "17").expect("Dressrosa 17 is listed");
    harness.state_mut().apply(vec![
        Action::from(JobQueueEvent::Select(id)),
        Action::from(ReportEvent::FixIt),
    ]);
    for _ in 0..500 {
        harness.run_steps(1);
        let app = harness.state();
        if app.pending.fixes.is_empty() && app.fix_followups.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    // Clear of the sidebar and the Overview's column.
    harness.hover_at(egui::pos2(1200.0, 320.0));
    shoot(&mut harness, out, "fix_it_done");
}

/// The scratch copy of the work folder of Dressrosa `episode` under `root`.
fn scratch_work_folder(root: &Path, episode: &str) -> PathBuf {
    let prefix = format!("muhn-pace-dressrosa-{episode}-");
    std::fs::read_dir(root.join("work"))
        .expect("the scratch work folder")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(&prefix))
        })
        .unwrap_or_else(|| panic!("no scratch work folder {prefix}*"))
}

/// The lines `qc` flags, by id, each once, in id order.
fn flagged_lines(qc: &QcReport) -> Vec<String> {
    use crate::job_report::models::finding_group::LineGroup;
    let mut ids: Vec<String> = qc
        .findings
        .iter()
        .filter(|finding| LineGroup::of(finding.check).is_some())
        .filter_map(|finding| finding.utterance.clone())
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// The heard word a `removed_locked` finding of line `id` names, and the word of `text` it stands
/// for: the first word sharing its first three letters, not the same, with that word's own
/// punctuation after it; `None` when there is none.
fn heard_word(qc: &QcReport, id: &str, text: &str) -> Option<(String, String)> {
    let fold = |word: &str| -> String {
        word.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect()
    };
    qc.findings
        .iter()
        .filter(|f| f.check == QcCheck::RemovedLocked && f.utterance.as_deref() == Some(id))
        .filter_map(|f| {
            f.detail
                .split_once(": ")
                .map(|(_, heard)| heard.to_string())
        })
        .find_map(|heard| {
            let heard = fold(&heard);
            let word = text.split_whitespace().find(|word| {
                let word = fold(word);
                word != heard && word.len() > 3 && heard.get(..3) == word.get(..3)
            })?;
            let tail: String = word.chars().skip_while(|c| c.is_alphanumeric()).collect();
            let mut spoken = heard.clone();
            if let Some(first) = spoken.get(..1) {
                spoken.replace_range(..1, &first.to_uppercase());
            }
            Some((format!("{spoken}{tail}"), word.to_string()))
        })
}

/// The scene's Fix It, over the job's own check and settled lines: the first line not the
/// owner's whose heard word it can restore, as a word replaced, and the four lines after it, as a
/// word put in or taken out, are changed; every other line the owner has not checked is answered
/// unchanged.
fn scene_fix() -> crate::job_report::services::fix_it::FixVideo {
    use job_model::outputs::{
        AdjudicationPass, Chosen, Correction, Corrections, FixBefore, FixRecord, FixVerdict,
        LineFix,
    };
    use pipeline::fix_it::FixOutcome;
    Arc::new(|video, options, _progress| {
        let job = options.work_root.join(pipeline::work_dir::job_id(
            &std::fs::canonicalize(video).expect("the video"),
        ));
        let qc: QcReport = super::job_fixtures::stored_qc(&job);
        let settled: AdjudicationPass = super::job_fixtures::stored_settled(&job);
        let mut corrections: Corrections = super::rendering_report::stored_corrections(&job);
        let open: Vec<String> = flagged_lines(&qc)
            .into_iter()
            .filter(|id| !corrections.by_owner(id))
            .collect();
        let replaced = open
            .iter()
            .position(|id| {
                let text = settled.lines.iter().find(|line| &line.id == id);
                text.and_then(|line| heard_word(&qc, id, &line.t)).is_some()
            })
            .unwrap_or(0);
        let mut lines = Vec::new();
        for (i, id) in open.iter().enumerate() {
            let Some(line) = settled.lines.iter().find(|line| &line.id == id) else {
                continue;
            };
            let words: Vec<&str> = line.t.split_whitespace().collect();
            let (before, after) = match i.checked_sub(replaced) {
                Some(0) => match heard_word(&qc, id, &line.t) {
                    Some((heard, word)) => (line.t.replacen(&word, &heard, 1), line.t.clone()),
                    None => (line.t.clone(), format!("Uh, {}", line.t)),
                },
                Some(2 | 4) if words.len() > 1 => (line.t.clone(), words[1..].join(" ")),
                Some(1..5) => (line.t.clone(), format!("Uh, {}", line.t)),
                _ => (line.t.clone(), line.t.clone()),
            };
            let verdict = if before == after {
                FixVerdict::Unchanged
            } else {
                FixVerdict::Accepted {
                    why: "Both engines heard it.".into(),
                }
            };
            lines.push(LineFix {
                id: line.id.clone(),
                problems: Vec::new(),
                checks: Vec::new(),
                before_text: before,
                before_flags: line.f.clone(),
                after_text: after,
                after_flags: line.f.clone(),
                steps: Vec::new(),
                refused: Vec::new(),
                removed: Vec::new(),
                applied: verdict.writes_correction(),
                verdict,
            });
        }
        let mut before = FixBefore::of(&qc);
        *before.counts.entry(QcCheck::TooShort).or_default() += 3;
        for line in lines.iter().filter(|line| line.applied) {
            corrections.set(Correction {
                id: line.id.clone(),
                text: line.after_text.clone(),
                flags: line.after_flags.clone(),
                chosen: Chosen::FixIt {
                    model: options.model.clone(),
                    why: line.why(),
                },
            });
        }
        let changed = lines
            .iter()
            .filter(|line| line.applied)
            .map(|line| line.id.clone())
            .collect();
        let record = FixRecord {
            model: options.model.clone(),
            video: "[Muhn Pace] Dressrosa 17".into(),
            lines,
            before: Some(before),
            ..FixRecord::default()
        };
        super::rendering_report::put_corrections(&job, &corrections);
        let store = super::job_fixtures::job_store(&job);
        pipeline::work_dir::put_fix_record(&store, &record).expect("the Fix It record");
        Ok(FixOutcome {
            work_dir: job,
            record,
            changed,
            kept_yours: Vec::new(),
        })
    })
}

/// Wait up to five seconds for the open line's still frame, so the scene shows it.
fn wait_for_still(harness: &mut Harness<'_, TbdSubtitlesApp>) {
    for _ in 0..50 {
        let decoding = harness
            .state()
            .still
            .as_ref()
            .and_then(|(_, still)| still.as_ref())
            .is_some_and(|still| still.is_decoding());
        if !decoding {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Every video finished: the list, the Dressrosa 15 Overview, then with Details and Step times
/// open, its lines to check and the settings.
fn finished_scenes(root: &Path, out: &Path, videos: &[PathBuf]) {
    let mut harness = harness(root, all_finished(videos));
    shoot(&mut harness, out, "queue");
    harness.get_by_label("[Muhn Pace] Dressrosa 15").click();
    shoot(&mut harness, out, "report");
    // Step times first, while the closed Details leaves it on screen.
    harness.get_by_label("Step times").click();
    harness.run_steps(2);
    harness.get_by_label("Details").click();
    harness.run_steps(2);
    // Down to the open Details, with Step times under it.
    for _ in 0..7 {
        harness.get_by_label("Details").scroll_down();
        harness.run_steps(1);
    }
    shoot(&mut harness, out, "overview_d15");
    // The header's tab, above the lines card's button of the same name.
    harness
        .get_all_by_label("Check Lines")
        .min_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .expect("the header offers Check Lines")
        .click();
    shoot(&mut harness, out, "review");
    harness.get_by_label("Settings").click();
    shoot(&mut harness, out, "settings");
    harness.state_mut().apply(vec![Action::ShowSettings(false)]);
    log_lines(&harness.state().env.log);
    harness.get_by_label("Log").click();
    shoot(&mut harness, out, "log");
    let summary = harness
        .state()
        .console
        .activity
        .shown()
        .find(|line| line.call.is_some())
        .map(|line| line.seq);
    harness
        .state_mut()
        .apply(vec![Action::LogConsole(LogConsoleEvent::SelectLine(
            summary,
        ))]);
    shoot(&mut harness, out, "log_detail");
    harness.get_by_label("Show Model Call").click();
    shoot(&mut harness, out, "log_calls");
}

/// The setup of a window with `videos` all finished in an earlier window, their rows summed up
/// from their work folders.
fn all_finished(videos: &[PathBuf]) -> impl FnOnce(&mut TbdSubtitlesApp) + use<> {
    let videos = videos.to_vec();
    move |app| {
        app.settings.items.iter_mut().for_each(|i| i.present = true);
        app.apply(vec![Action::QueueVideos(videos)]);
        for item in &mut app.queue.items {
            item.state = JobState::FinishedBefore;
        }
        app.refresh_summaries(None);
    }
}

/// Every video finished, Dressrosa 16 with a failed language-model call added to its check: the
/// needs-attention Overview. It changes the quality check in the scratch copy of 16's database, so it runs after the
/// scenes that show 16.
fn attention_scene(root: &Path, out: &Path, videos: &[PathBuf]) {
    let job = std::fs::read_dir(root.join("work"))
        .expect("the scratch work folder")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name().is_some_and(|name| {
                name.to_string_lossy()
                    .starts_with("muhn-pace-dressrosa-16-")
            })
        })
        .expect("Dressrosa 16's work folder");
    let mut qc: QcReport = super::job_fixtures::stored_qc(&job);
    qc.findings.push(job_model::report::QcFinding {
        check: job_model::report::QcCheck::FailedCall,
        time_s: 0.0,
        text: String::new(),
        detail: "batch 7 of 12: the model answered with no JSON".into(),
        utterance: None,
    });
    super::job_fixtures::put_qc(&job, &qc);
    let _ = std::fs::remove_dir_all(root.join("data"));
    let mut harness = harness(root, all_finished(videos));
    harness.get_by_label("[Muhn Pace] Dressrosa 16").click();
    shoot(&mut harness, out, "needs_attention");
}

/// The first window: no videos, the models still missing.
fn first_run_scene(root: &Path, out: &Path) {
    let mut harness = harness(root, |_| {});
    shoot(&mut harness, out, "first_run");
}

/// The Settings window on each of its tabs, over the finished list with Dressrosa 15
/// selected; the machine checks are set by hand, as the mockup shows them, and none runs.
fn settings_scenes(root: &Path, out: &Path, videos: &[PathBuf]) {
    use crate::settings::models::machine::{Check, CheckState};
    use crate::settings::models::page::SettingsTab;
    let _ = std::fs::remove_dir_all(root.join("data"));
    let finished = all_finished(videos);
    let mut harness = harness(root, move |app| {
        finished(app);
        app.queue.selected = id_of(app, "15");
        app.refresh_report(false);
        let page = &mut app.settings;
        page.work_size = Some(crate::settings::services::work_folder::size(
            &page.work_folder,
        ));
        page.models_size = Some(
            page.items
                .iter()
                .filter(|item| item.kind == crate::settings::models::machine::ItemKind::Model)
                .map(|item| item.bytes)
                .sum(),
        );
        let check = |name, detail: String| Check {
            name,
            state: CheckState::Ok,
            detail,
            path: None,
        };
        app.settings.checks = Some(vec![
            check(
                "GPU",
                "NVIDIA GeForce RTX 3070, driver 615.71.09, 6566 of 8192 MiB free".into(),
            ),
            Check {
                path: Some(app.env.runtime_dir.clone()),
                ..check("CUDA runtime", "CUDA, cuDNN and ONNX Runtime found".into())
            },
            check("FFmpeg", "ffmpeg version 8.1.2".into()),
            check(
                "Clip playback",
                "FFmpeg plays sound through its pulse output".into(),
            ),
            check("ffprobe", "ffprobe version 8.1.2".into()),
            check("claude CLI", "2.1.282 (Claude Code)".into()),
            check(
                "Whisper worker",
                "~/Projects/TBD-subtitles/target/release/tbd-subtitles-ggml".into(),
            ),
        ]);
        // A watch folder that is there, one on a drive not mounted, and the Dolphin entry
        // written; set on the page only, the scratch settings file keeps none.
        let page = &mut app.settings;
        page.saved.watch_folders = vec![
            page.work_folder.clone(),
            PathBuf::from("/run/media/system/Backup_drive/Media/one_pace"),
        ];
        page.right_click = crate::settings::models::page::RightClickEntry::Installed(
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join(".local/share/kio/servicemenus/tbd-subtitles.desktop"),
        );
    });
    for (tab, scene) in [
        (SettingsTab::General, "settings_general"),
        (SettingsTab::Automation, "settings_automation"),
        (SettingsTab::Engines, "settings_engines"),
        (SettingsTab::Models, "settings_models"),
        (SettingsTab::ThisComputer, "settings_machine"),
    ] {
        harness
            .state_mut()
            .apply(vec![Action::from(SettingsEvent::Open(tab))]);
        shoot(&mut harness, out, scene);
    }
}

/// The first window with the models banner: what is missing, then a download under way (its
/// first model on disk and the second 40 % in), set by hand so nothing downloads.
fn banner_scenes(root: &Path, out: &Path) {
    use crate::settings::models::page::DownloadProgress;
    let mut harness = harness(root, |_| {});
    shoot(&mut harness, out, "banner_missing");
    let page = &mut harness.state_mut().settings;
    let size = page.items.iter().map(|item| item.bytes).sum();
    let finished = page.items[0].bytes;
    page.items[0].present = true;
    let next = &page.items[1];
    page.download = Some(DownloadProgress {
        id: next.id.clone(),
        held: next.bytes * 2 / 5,
        total: next.bytes,
        finished,
        size,
    });
    shoot(&mut harness, out, "banner_downloading");
}

/// A running queue: Dressrosa 16 settling its words, the rest waiting, 15 and 11 done; then a
/// waiting row's menu, and the toast after it is removed.
fn queue_scenes(root: &Path, out: &Path, videos: &[PathBuf]) {
    let queued = queued_videos(root, videos);
    let mut harness = harness(root, move |app| {
        running_queue(app, queued);
        app.queue.selected = id_of(app, "15");
        app.refresh_report(false);
    });
    shoot(&mut harness, out, "running_queue");
    harness
        .get_by_label("[Muhn Pace] Dressrosa 17")
        .click_secondary();
    shoot(&mut harness, out, "row_menu");
    // The menu's command, left of the selected row's card, which offers it too.
    harness
        .get_all_by_label("Remove from List")
        .min_by(|a, b| a.rect().left().total_cmp(&b.rect().left()))
        .expect("the menu offers Remove from List")
        .click();
    shoot(&mut harness, out, "undo_toast");
}

/// The detail pane of the running queue: Dressrosa 16 running, 17 next in line, 19 failed while
/// hearing the speech and 20 cancelled.
fn detail_scenes(root: &Path, out: &Path, videos: &[PathBuf]) {
    let queued = queued_videos(root, videos);
    let mut harness = harness(root, move |app| {
        running_queue(app, queued);
        if let Some(item) = id_of(app, "19").and_then(|id| app.queue.get_mut(id)) {
            let finished = [
                (StepName::ProbeDecode, 7.5),
                (StepName::ShotScan, 41.0),
                (StepName::Separation, 190.0),
                (StepName::Vad, 1.1),
                (StepName::AsrParakeet, 16.0),
            ]
            .map(|(step, seconds)| (step, FinishedStep::Done(Some(seconds))))
            .to_vec();
            item.state = JobState::Failed(Failure::new(
                Some(StepName::AsrWhisper),
                "the Whisper worker stopped early (exit status 1); its log is \
                 logs/asr_whisper.log"
                    .into(),
                finished,
            ));
        }
        if let Some(item) = id_of(app, "20").and_then(|id| app.queue.get_mut(id)) {
            item.state = JobState::Cancelled { kept_steps: 9 };
        }
    });
    for (episode, scene) in [
        ("16", "running_detail"),
        ("17", "waiting_card"),
        ("19", "failed_card"),
        ("20", "cancelled_card"),
    ] {
        let id = id_of(harness.state(), episode).expect("the episode is queued");
        harness
            .state_mut()
            .apply(vec![Action::from(JobQueueEvent::Select(id))]);
        shoot(&mut harness, out, scene);
    }
}

/// The finished videos and the waiting episodes, in a queue of their own rather than the one the
/// finished scenes kept.
fn queued_videos(root: &Path, videos: &[PathBuf]) -> Vec<PathBuf> {
    let _ = std::fs::remove_dir_all(root.join("data"));
    let mut queued = videos.to_vec();
    let folder = root.join("videos");
    queued.extend(
        WAITING
            .iter()
            .map(|n| folder.join(format!("[Muhn Pace] Dressrosa {n}.mp4"))),
    );
    queued
}

/// Queue `queued` with the models on disk, 11 and 15 finished, 16 settling its words, and the
/// queue running.
fn running_queue(app: &mut TbdSubtitlesApp, queued: Vec<PathBuf>) {
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.apply(vec![Action::QueueVideos(queued)]);
    for episode in ["11", "15"] {
        if let Some(item) = id_of(app, episode).and_then(|id| app.queue.get_mut(id)) {
            item.state = JobState::FinishedBefore;
        }
    }
    let running = id_of(app, "16");
    if let Some(item) = running.and_then(|id| app.queue.get_mut(id)) {
        item.state = JobState::Running(Box::new(settling(Instant::now())));
    }
    app.queue.running = true;
    // The full lane holds the running job, with no thread behind it.
    app.cancel = running.map(|id| (id, CancelToken::new()));
    app.refresh_summaries(None);
}

/// The job of Dressrosa `episode`.
fn id_of(app: &TbdSubtitlesApp, episode: &str) -> Option<JobId> {
    let name = format!("[Muhn Pace] Dressrosa {episode}");
    app.queue
        .items
        .iter()
        .find(|item| item.name() == name)
        .map(|item| item.id)
}

/// Render the scene in light and in dark, as `<scene>_light.png` and `<scene>_dark.png`.
fn shoot(harness: &mut Harness<'_, TbdSubtitlesApp>, out: &Path, scene: &str) {
    for (scheme, name) in [(Scheme::Light, "light"), (Scheme::Dark, "dark")] {
        harness.state_mut().scheme = scheme;
        // A spinner asks for frames without end; the scene is taken where it stands.
        let _ = harness.run_ok();
        let image = harness
            .render()
            .unwrap_or_else(|error| panic!("{scene} in {name} did not render: {error}"));
        let path = out.join(format!("{scene}_{name}.png"));
        image
            .save(&path)
            .unwrap_or_else(|error| panic!("{} was not written: {error}", path.display()));
    }
}

/// Copy the job databases of the owner's work folders into `work`, keeping each folder's name, and
/// return the videos they belong to.
fn copy_work_folders(work: &Path) -> Vec<PathBuf> {
    let source = inference::model_store::app_data_dir()
        .expect("the app data folder")
        .join("work");
    let mut videos = Vec::new();
    for episode in EPISODES {
        let prefix = format!("muhn-pace-dressrosa-{episode}-");
        let folder = std::fs::read_dir(&source)
            .unwrap_or_else(|error| panic!("{} cannot be read: {error}", source.display()))
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with(&prefix))
            })
            .unwrap_or_else(|| panic!("no work folder {prefix}* in {}", source.display()));
        let copy = work.join(folder.file_name().expect("a folder name"));
        std::fs::create_dir_all(&copy).expect("a scratch folder");
        std::fs::copy(folder.join(JOB_DATABASE), copy.join(JOB_DATABASE))
            .expect("a copied job database");
        let record = pipeline::work_dir::read_job(&copy)
            .expect("the copied job database reads")
            .expect("a job record")
            .record;
        videos.push(PathBuf::from(record.video));
    }
    videos
}

/// A job's lines as the log window shows them: the owner's action, the job and its steps, a
/// worker's and a program's lines, a model call with its summary, and a failure.
fn log_lines(log: &crate::core::log_buffer::LogBuffer) {
    use crate::core::log_buffer::Fresh;
    use tracing::Level;
    let video = "[Muhn Pace] Dressrosa 12";
    let at = |level, target, message: &str, step: Option<&str>| {
        let mut fresh = Fresh::new(level, target, message);
        fresh.video = Some(video.to_string());
        fresh.step = step.map(str::to_string);
        fresh
    };
    log.push(Fresh::new(
        Level::DEBUG,
        "tbd_subtitles::application",
        "action Queue(Start)",
    ));
    log.push(at(Level::INFO, "job", "Job started for /media/one_pace/[Muhn Pace] Dressrosa 12.mkv in /work/d12; to run: probe_decode, asr_parakeet, adjudicate", None));
    log.push(at(Level::INFO, "job", "Step started", Some("asr_parakeet")));
    log.push(at(
        Level::DEBUG,
        "pipeline::runner",
        "step asr_parakeet runs in a worker of /app/tbd-subtitles",
        Some("asr_parakeet"),
    ));
    log.push(at(Level::DEBUG, "child_process", "tbd-subtitles[4242] started: /app/tbd-subtitles worker asr_parakeet /home/owner/.local/share/tbd-subtitles/work/muhn-pace-dressrosa-12-802e7ebb", Some("asr_parakeet")));
    log.push(at(
        Level::INFO,
        "pipeline::tasks",
        "loading the Parakeet model",
        Some("asr_parakeet"),
    ));
    log.push(at(
        Level::DEBUG,
        "job",
        "5 of 10 done",
        Some("asr_parakeet"),
    ));
    log.push(at(
        Level::INFO,
        "job",
        "Step finished in 88.5 s, 1830 MiB RAM, 2410 MiB VRAM",
        Some("asr_parakeet"),
    ));
    log.push(at(Level::INFO, "job", "Step started", Some("adjudicate")));
    let mut summary = at(
        Level::INFO,
        "inference::llm::call_log",
        "claude sonnet · words, batch 1 of 40: 212 lines answered in 41.3 s, 18234 tokens in, 2210 out, $0.0874",
        Some("adjudicate"),
    );
    summary.call = Some("4250-1".to_string());
    log.push(summary);
    log.push(at(
        Level::DEBUG,
        "child_process",
        "claude[4301] exited 0 after 41.30 s",
        Some("adjudicate"),
    ));
    log.push(at(
        Level::WARN,
        "child_process",
        "ffprobe[4302] exited 1 after 0.05 s",
        Some("adjudicate"),
    ));
    log.push(Fresh::new(
        Level::ERROR,
        "tbd_subtitles::core::portal",
        "the desktop could not open it",
    ));
    log.push_call(
        job_model::model_call::ModelExchange {
            id: "4250-1".into(),
            model: "sonnet".into(),
            purpose: "words, batch 1 of 40".into(),
            system: "You settle the words of an English dub from what two speech engines heard.\nNever invent a word no engine heard.".into(),
            message: "GLOSSARY: Luffy, Zoro, Doflamingo\nU0001 [p] we have to get to the palace\nU0001 [w] we've got to get to the palace\nU0002 [p] Doflamingo is waiting\nU0002 [w] Doflamingo's waiting".into(),
            schema: "{\n  \"type\": \"object\",\n  \"required\": [\"lines\"]\n}".into(),
            answer: "{\n  \"lines\": [\n    {\"id\": \"U0001\", \"text\": \"We've got to get to the palace.\"},\n    {\"id\": \"U0002\", \"text\": \"Doflamingo's waiting.\"}\n  ]\n}".into(),
            error: None,
            input_tokens: 18234,
            output_tokens: 2210,
            cost_usd: Some(0.0874),
            seconds: 41.3,
        },
        Some(video.to_string()),
        Some("adjudicate".to_string()),
    );
}
