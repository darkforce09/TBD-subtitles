//! Fix It on many videos at once, rendered headless: two runs side by side and Stop on one, Fix
//! All in the sidebar's Done heading, Fix It after each job, the note of a run waiting for a free
//! Claude call, and the cap on Claude calls following its setting.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use job_model::outputs::FixRecord;
use job_model::report::QcCheck;
use pipeline::fix_it::{FixOutcome, FixProgress, FixStage};

use super::rendering_fix_it::{assert_shows, until_fixed, waiting_fix};
use super::rendering_report::{check, finding, scratch, write_job};
use super::*;
use crate::job_report::services::fix_it::FixVideo;

/// A check with a line too short and an unsure line, both lines Fix It asks about.
fn lines_to_fix() -> QcReport {
    check(vec![
        finding(QcCheck::TooShort, 1156.1, Some("U0314")),
        finding(QcCheck::Unsure, 246.8, Some("U0053")),
    ])
}

/// A window over `root` whose videos `names`, run through `run` in order, have finished, each
/// with the lines of `lines_to_fix`; its Fix It runs through `fix`. The jobs' ids, in `names`'
/// order.
fn finished_videos(
    root: &Path,
    names: &[&str],
    fix: FixVideo,
    run: RunJob,
) -> (TbdSubtitlesApp, Vec<JobId>) {
    let videos: Vec<PathBuf> = names
        .iter()
        .map(|name| {
            let video = root.join(format!("{name}.mp4"));
            std::fs::write(&video, b"video").expect("video");
            video
        })
        .collect();
    let mut env = Environment::scratch(root, run);
    env.fix_video = fix;
    let mut app = TbdSubtitlesApp::new(env, videos.clone());
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.apply(vec![Action::from(JobQueueEvent::Start)]);
    settle(&mut app);
    for video in &videos {
        write_job(root, video, &lines_to_fix(), None);
    }
    app.refresh_summaries(None);
    let ids = videos
        .iter()
        .map(|video| {
            let item = app
                .queue
                .items
                .iter()
                .find(|item| item.video.file_name() == video.file_name());
            item.expect("the video is listed").id
        })
        .collect();
    (app, ids)
}

/// Poll until every Fix It run under way has said which pass it is in.
fn until_all_reading(app: &mut TbdSubtitlesApp) {
    for _ in 0..500 {
        app.poll();
        if app
            .pending
            .fixes
            .values()
            .all(|fixing| fixing.progress.is_some())
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("a Fix It run never reported");
}

/// Poll until `done` holds, or fail after five seconds.
fn until(app: &mut TbdSubtitlesApp, what: &str, done: impl Fn(&TbdSubtitlesApp) -> bool) {
    for _ in 0..500 {
        app.poll();
        if done(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("{what} never happened");
}

/// A Fix It that reads the video, takes a slot at the gate its calls share, holds it until
/// `release` is set or it is stopped, then changes nothing.
fn holding_a_call(release: Arc<AtomicBool>) -> FixVideo {
    Arc::new(move |video, options, progress| {
        progress(FixProgress {
            stage: FixStage::Reading,
            done: 0,
            total: 1,
        });
        let cancel = options.cancel.flag();
        let Some(_permit) = options.calls.acquire(&cancel) else {
            return Err(PipelineError::cancelled("Fix It"));
        };
        while !release.load(Ordering::SeqCst) {
            if options.cancel.is_cancelled() {
                return Err(PipelineError::cancelled("Fix It"));
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        Ok(FixOutcome {
            work_dir: video.to_path_buf(),
            record: FixRecord {
                model: options.model.clone(),
                ..FixRecord::default()
            },
            changed: Vec::new(),
            kept_yours: Vec::new(),
        })
    })
}

/// A run that leaves the lines of `lines_to_fix` in the job's work directory, as a full run
/// whose check flags them does.
fn leaving_lines_to_fix() -> RunJob {
    Arc::new(|video, options, progress| {
        let root = options.work_root.parent().expect("the scratch root");
        write_job(root, video, &lines_to_fix(), None);
        stand_in()(video, options, progress)
    })
}

/// Turn Fix It after each job on or off, as the Settings window does.
fn fix_after_each_job(app: &mut TbdSubtitlesApp, on: bool) {
    let mut edited = app.settings.saved.clone();
    edited.language_model.fix_after_run = on;
    app.apply(vec![Action::from(SettingsEvent::Edit(edited))]);
    assert_eq!(app.settings.saved.language_model.fix_after_run, on);
}

#[test]
fn two_videos_fix_at_once() {
    let root = scratch("fix-many-at-once");
    let go = Arc::new(AtomicBool::new(false));
    let names = ["Dressrosa 12", "Dressrosa 13"];
    let (mut app, ids) = finished_videos(&root, &names, waiting_fix(go.clone()), stand_in());
    app.apply(vec![Action::FixIt(ids[0]), Action::FixIt(ids[1])]);
    assert_eq!(
        app.pending.fixes.len(),
        2,
        "each video has a run of its own"
    );
    until_all_reading(&mut app);
    app.apply(vec![Action::from(JobQueueEvent::Select(ids[1]))]);
    let (text, _) = render(&app);
    assert_shows(
        &text,
        &["Fixing with Claude Opus · reading the whole video (1 of 4)…"],
    );
    assert_eq!(
        text.matches("Fixing with Claude · 1 of 4").count(),
        2,
        "both rows give their step: {text}"
    );
    app.apply(vec![Action::FixIt(ids[1])]);
    assert_eq!(app.pending.fixes.len(), 2, "never two runs of one video");
    let (text, _) = render(&app);
    assert_shows(&text, &["Fix It is already fixing this video."]);
    go.store(true, Ordering::SeqCst);
    until_fixed(&mut app);
    settle(&mut app);
    assert!(app.fix_followups.is_empty(), "both runs finished");
    let (text, _) = render(&app);
    assert_shows(
        &text,
        &[
            "Dressrosa 12 is fixed: Claude changed 1 line",
            "Dressrosa 13 is fixed: Claude changed 1 line",
        ],
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn stop_ends_only_its_own_video() {
    let root = scratch("fix-many-stop");
    let go = Arc::new(AtomicBool::new(false));
    let names = ["Dressrosa 12", "Dressrosa 13"];
    let (mut app, ids) = finished_videos(&root, &names, waiting_fix(go.clone()), stand_in());
    app.apply(vec![Action::FixIt(ids[0]), Action::FixIt(ids[1])]);
    until_all_reading(&mut app);
    app.apply(vec![Action::StopFix(ids[0])]);
    until(&mut app, "the stopped run's end", |app| {
        !app.video_fixing(ids[0])
    });
    assert!(app.video_fixing(ids[1]), "the other video's run goes on");
    let (text, _) = render(&app);
    assert_shows(
        &text,
        &[
            "Fix It stopped on Dressrosa 12. Nothing was changed; Fix It again picks up where it \
           stopped.",
        ],
    );
    go.store(true, Ordering::SeqCst);
    until_fixed(&mut app);
    settle(&mut app);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn fix_all_starts_every_finished_video_with_lines_to_fix_and_hides_once_all_are_fixing() {
    let root = scratch("fix-many-all");
    let go = Arc::new(AtomicBool::new(false));
    let names = ["Dressrosa 12", "Dressrosa 13", "Dressrosa 14"];
    let (mut app, ids) = finished_videos(&root, &names, waiting_fix(go.clone()), stand_in());
    let passing = app.queue.get(ids[2]).expect("listed").video.clone();
    write_job(&root, &passing, &check(Vec::new()), None);
    app.refresh_summaries(None);
    assert_eq!(
        app.fix_candidates(),
        [ids[0], ids[1]],
        "the videos with lines to fix, oldest finished first"
    );
    let (text, _) = render(&app);
    assert_shows(&text, &["Fix All", "DONE"]);
    app.apply(vec![Action::from(JobQueueEvent::FixAll)]);
    let fixing: Vec<JobId> = app.pending.fixes.keys().copied().collect();
    assert_eq!(fixing, [ids[0], ids[1]]);
    assert!(app.fix_candidates().is_empty());
    let (text, _) = render(&app);
    assert_shows(&text, &["Fix It started on 2 videos."]);
    assert!(!text.contains("Fix All"), "nothing left to start: {text}");
    go.store(true, Ordering::SeqCst);
    until_fixed(&mut app);
    settle(&mut app);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn fix_after_each_job_starts_when_a_full_run_finishes() {
    let root = scratch("fix-many-after-run");
    let go = Arc::new(AtomicBool::new(false));
    let video = root.join("Dressrosa 12.mp4");
    std::fs::write(&video, b"video").expect("video");
    let mut env = Environment::scratch(&root, leaving_lines_to_fix());
    env.fix_video = waiting_fix(go.clone());
    let mut app = TbdSubtitlesApp::new(env, vec![video]);
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    fix_after_each_job(&mut app, true);
    app.apply(vec![Action::from(JobQueueEvent::Start)]);
    settle(&mut app);
    let id = app.queue.items[0].id;
    assert!(app.video_fixing(id), "Fix It starts once the run finished");
    let (text, _) = render(&app);
    assert_shows(&text, &["Fixing with Claude · "]);
    assert!(!text.contains("Fix It could not start"), "{text}");
    go.store(true, Ordering::SeqCst);
    until_fixed(&mut app);
    settle(&mut app);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn fix_after_each_job_off_starts_nothing() {
    let root = scratch("fix-many-after-run-off");
    let video = root.join("Dressrosa 12.mp4");
    std::fs::write(&video, b"video").expect("video");
    let mut env = Environment::scratch(&root, leaving_lines_to_fix());
    env.fix_video = waiting_fix(Arc::new(AtomicBool::new(true)));
    let mut app = TbdSubtitlesApp::new(env, vec![video]);
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    fix_after_each_job(&mut app, false);
    app.apply(vec![Action::from(JobQueueEvent::Start)]);
    settle(&mut app);
    let id = app.queue.items[0].id;
    assert!(app.pending.fixes.is_empty(), "no run starts by itself");
    assert_eq!(app.fix_candidates(), [id], "Fix All would start it");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_video_waiting_for_a_free_call_says_so() {
    let root = scratch("fix-many-waiting");
    let release = Arc::new(AtomicBool::new(false));
    let names = ["Dressrosa 12", "Dressrosa 13"];
    let fix = holding_a_call(release.clone());
    let (mut app, ids) = finished_videos(&root, &names, fix, stand_in());
    app.claude_gate.set_limit(1);
    app.apply(vec![Action::FixIt(ids[0])]);
    until(&mut app, "the first run's call", |app| {
        app.claude_gate.held() == 1
    });
    app.apply(vec![Action::FixIt(ids[1])]);
    until(&mut app, "the second run's wait", |app| {
        app.pending.fixes.get(&ids[1]).is_some_and(|f| f.waiting())
    });
    app.apply(vec![Action::from(JobQueueEvent::Select(ids[1]))]);
    let (text, _) = render(&app);
    assert_shows(
        &text,
        &["Waiting for a free Claude call. 0 of 1 call done."],
    );
    app.apply(vec![Action::from(JobQueueEvent::Select(ids[0]))]);
    let (text, _) = render(&app);
    assert!(!text.contains("Waiting for a free Claude call"), "{text}");
    assert_shows(
        &text,
        &["0 of 1 call done. The subtitles change only once every change is checked."],
    );
    release.store(true, Ordering::SeqCst);
    until_fixed(&mut app);
    assert_eq!(app.claude_gate.held(), 0, "every call gave its slot back");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn claude_calls_at_once_applies_at_once() {
    let root = scratch("fix-many-cap");
    let app_root = root.join("app");
    let mut app = TbdSubtitlesApp::new(Environment::scratch(&app_root, stand_in()), Vec::new());
    assert_eq!(app.claude_gate.limit(), 32, "the saved cap");
    let mut edited = app.settings.saved.clone();
    edited.language_model.fix_calls = 5;
    app.apply(vec![Action::from(SettingsEvent::Edit(edited))]);
    assert_eq!(app.claude_gate.limit(), 5);
    let _ = std::fs::remove_dir_all(&root);
}
