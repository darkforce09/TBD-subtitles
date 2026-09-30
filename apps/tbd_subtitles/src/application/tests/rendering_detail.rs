//! The detail pane rendered headless: the header, the job cards, the progress card and the
//! stages.

use super::*;

/// Whether some piece of text painted is exactly `line`.
fn painted(text: &str, line: &str) -> bool {
    text.lines().any(|painted| painted == line)
}

/// A live clock can cross a second while fonts and two headless frames are rendered.
fn painted_elapsed(
    text: &str,
    prefix: &str,
    suffix: &str,
    started: Instant,
    before: Instant,
    after: Instant,
) {
    let first = before.duration_since(started).as_secs_f64().round() as u64;
    let last = after.duration_since(started).as_secs_f64().round() as u64;
    assert!(
        (first..=last).any(|seconds| {
            let duration = crate::core::format::duration(seconds as f64);
            painted(text, &format!("{prefix}{duration}{suffix}"))
        }),
        "no {prefix}<duration>{suffix} between {first} and {last} seconds in {text}"
    );
}

#[test]
fn a_running_job_shows_its_stage_its_step_and_its_stages() {
    let mut app = app("detail-running", vec![PathBuf::from("/v/Dressrosa 16.mp4")]);
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    let now = Instant::now();
    let id = app.queue.items[0].id;
    app.queue.items[0].state = JobState::Running(Box::new(settling(now)));
    app.queue.running = true;
    // The full lane holds the running job, with no thread behind it.
    app.cancel = Some((id, CancelToken::new()));
    app.queue.selected = Some(id);
    let before = Instant::now();
    let (text, actions) = render(&app);
    let after = Instant::now();
    for expected in [
        "Dressrosa 16",
        "Cancel",
        "Settling the words",
        "Now: Language model settles the words · step 9 of 29",
        "Show all 29 steps",
        "9 stages",
        "Separate the voices",
        "1 min 00 s",
        "Settle the words",
        "Language model settles the words",
        "Second look at unsure lines",
        "to run",
        "Lay out the subtitles",
        "Translate on-screen text",
        "Write the subtitles",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    for (prefix, suffix, elapsed) in [
        ("25:59 video · running for ", "", 600),
        ("", " so far", 600),
        ("", " so far", 40),
    ] {
        painted_elapsed(
            &text,
            prefix,
            suffix,
            now - Duration::from_secs(elapsed),
            before,
            after,
        );
    }
    assert!(
        text.lines()
            .any(|line| line.ends_with(" left") && !line.contains('·')),
        "the progress card shows the time left on its own: {text}"
    );
    assert!(
        !text.contains("Listen with Whisper"),
        "a done stage keeps its steps folded: {text}"
    );
    assert!(actions.is_empty(), "an idle frame asks for nothing");
    if let Some(item) = app.queue.get_mut(id)
        && let JobState::Running(progress) = &mut item.state
    {
        progress.cancelling = true;
    }
    let (text, _) = render(&app);
    assert!(painted(&text, "Stopping…"), "{text}");
    assert!(
        !painted(&text, "Cancel"),
        "Stopping… stands in for Cancel: {text}"
    );
}

#[test]
fn a_running_job_works_out_its_time_left_until_its_length_is_known() {
    let mut app = app("detail-probe", vec![PathBuf::from("/v/a.mp4")]);
    let id = app.queue.items[0].id;
    let started = Instant::now() - Duration::from_secs(5);
    let mut progress = JobProgress::new(started);
    if let Some(row) = progress.row_mut(StepName::ProbeDecode) {
        row.state = StepState::Running {
            started: Instant::now(),
            done: 0,
            total: 0,
            message: None,
        };
    }
    app.queue.items[0].state = JobState::Running(Box::new(progress));
    app.cancel = Some((id, CancelToken::new()));
    app.queue.selected = Some(id);
    let before = Instant::now();
    let (text, _) = render(&app);
    let after = Instant::now();
    for expected in [
        "Reading the video",
        "Now: Read the video's details · step 1 of 29",
        "Working out the time left…",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    painted_elapsed(&text, "Running for ", "", started, before, after);
}

#[test]
fn a_waiting_job_shows_its_place_and_what_starts_it() {
    let mut app = app(
        "detail-waiting",
        ["/v/a.mp4", "/v/b.mp4"].map(PathBuf::from).to_vec(),
    );
    let (text, _) = render(&app);
    for expected in [
        "Select a video",
        "Its progress, subtitles and lines to check show here.",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    app.queue.selected = Some(app.queue.items[1].id);
    let (text, actions) = render(&app);
    for expected in [
        "2nd in line",
        "Length known once it starts · Waiting",
        "It can start once the models are on disk. Drag it in the sidebar to change the order.",
        "/v/b.mp4",
        "Run Next",
        "Remove from List",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    assert!(actions.is_empty(), "an idle frame asks for nothing");
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.queue.selected = Some(app.queue.items[0].id);
    let (text, _) = render(&app);
    for expected in [
        "Next in line",
        "Length known once it starts · next in line",
        "Press Start Queue to begin. Drag it in the sidebar to change the order.",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
}

#[test]
fn a_job_that_failed_before_its_first_step_shows_no_stages() {
    let mut app = app_with("detail-locked", vec![PathBuf::from("a.mp4")], locked());
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.apply(vec![Action::from(JobQueueEvent::Start)]);
    settle(&mut app);
    let id = app.queue.items[0].id;
    app.apply(vec![Action::from(JobQueueEvent::Select(id))]);
    let (text, _) = render(&app);
    for expected in [
        "Failed before the first step",
        "The video could not start.",
        "lock the work directory: another process runs this job",
        "Try Again continues after any steps already done. It starts at once.",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    assert!(!text.contains("Show all 29 steps"), "{text}");
}

#[test]
fn a_failed_job_shows_the_steps_it_ran_as_done_with_their_times() {
    let mut app = app("detail-failed", vec![PathBuf::from("/v/a.mp4")]);
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    let id = app.queue.items[0].id;
    app.queue.items[0].state = JobState::Failed(Failure::new(
        Some(StepName::AsrWhisper),
        "step asr_whisper: out of memory".into(),
        vec![
            (StepName::ProbeDecode, FinishedStep::StillValid),
            (StepName::Separation, FinishedStep::Done(Some(190.0))),
            (StepName::Vad, FinishedStep::Done(Some(1.0))),
            (StepName::AsrParakeet, FinishedStep::Done(Some(16.0))),
        ],
    ));
    app.queue.selected = Some(id);
    let (text, actions) = render(&app);
    for expected in [
        "The 4 finished steps are kept. Try Again continues after them. It starts at once.",
        "already done",
        "3 min 11 s",
        "Listen with Parakeet",
        "16 s",
        "Listen with Whisper",
        "failed",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    assert!(
        painted(&text, "done"),
        "a step run before the failure: {text}"
    );
    assert!(actions.is_empty(), "an idle frame asks for nothing");
}

#[test]
fn a_job_another_process_runs_waits_busy_and_starts_once_that_process_ends() {
    let owner = std::os::unix::process::parent_id();
    let mut app = app_with(
        "detail-busy",
        vec![PathBuf::from("a.mp4")],
        busy_once(owner),
    );
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.apply(vec![Action::from(JobQueueEvent::Start)]);
    settle(&mut app);
    let id = app.queue.items[0].id;
    assert!(
        matches!(app.queue.items[0].state, JobState::Busy { owner: Some(pid), .. } if pid == owner),
        "{:?}",
        app.queue.items[0].state
    );
    assert!(app.queue.running, "a busy job keeps the queue on");
    app.apply(vec![Action::from(JobQueueEvent::Select(id))]);
    let (text, _) = render(&app);
    for expected in [
        format!("Busy · process {owner} runs this video"),
        format!(
            "Process {owner}, outside this window, runs this video. It starts once that process \
             ends."
        ),
    ] {
        assert!(text.contains(&expected), "{expected} not in {text}");
    }
    assert!(!text.contains("Failed"), "{text}");

    // The owner ends: the job waits again and the queue starts it.
    if let JobState::Busy { owner, .. } = &mut app.queue.items[0].state {
        *owner = Some(u32::MAX);
    }
    app.poll_busy(Instant::now());
    settle(&mut app);
    assert!(
        matches!(app.queue.items[0].state, JobState::Finished(_)),
        "{:?}",
        app.queue.items[0].state
    );
}
