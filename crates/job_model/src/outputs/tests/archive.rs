use std::collections::BTreeMap;

use super::{
    AdjudicationPass, Aligned, AlignedUtterance, AlignedWord, AudioStream, CandidateKind, Chosen,
    ChunkWords, Correction, Corrections, EngineTranscript, Findings, FixBefore, FixBrief,
    FixFamily, FixRecord, FixStep, FixVerdict, Line, LineFix, OutputRecord, ProbeDecoded,
    ProbeResult, Redecode, ShotChanges, ShotCut, SoundCandidate, SoundCue, SoundCues, SoundEvent,
    SpeechPlan, Suspect, TimeSpan, TimedWord, TimingSource, Utterance, VideoStream,
};
use crate::archive_round_trip::round_trip;
use crate::report::QcCheck;

const EVERY_KIND: [CandidateKind; 4] = [
    CandidateKind::Effect,
    CandidateKind::Voice,
    CandidateKind::Tag,
    CandidateKind::Song,
];

/// Fails to compile when `CandidateKind` gains a variant, until [`EVERY_KIND`] lists it too.
fn listed_kind(kind: CandidateKind) {
    match kind {
        CandidateKind::Effect | CandidateKind::Voice | CandidateKind::Tag | CandidateKind::Song => {
        }
    }
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

fn line() -> Line {
    Line {
        id: "U0012".into(),
        t: "Luffy! || Wait for me!".into(),
        f: strings(&["NARR", "UNSURE"]),
    }
}

fn findings() -> Findings {
    Findings {
        missing_ids: strings(&["U0001"]),
        duplicate_ids: strings(&["U0002"]),
        unknown_ids: strings(&["U9999"]),
        novel: vec![("U0003".into(), "Dofy".into())],
        removed_locked: vec![("U0004".into(), "gonna".into())],
        too_fast: strings(&["U0005"]),
    }
}

fn timed_word(text: &str) -> TimedWord {
    TimedWord {
        text: text.into(),
        start_s: 1.5,
        end_s: 1.75,
        confidence: Some(0.875),
    }
}

fn transcript() -> EngineTranscript {
    EngineTranscript {
        engine: "parakeet-tdt-0.6b-v2".into(),
        input: "vocals.roformer".into(),
        chunks: vec![ChunkWords {
            span: TimeSpan::new(0.0, 30.0),
            words: vec![timed_word("Straw"), timed_word("Hat!")],
        }],
    }
}

fn aligned_word(source: TimingSource) -> AlignedWord {
    AlignedWord {
        text: "Rebecca".into(),
        start_s: 2.0,
        end_s: 2.5,
        source,
    }
}

fn aligned_utterance() -> AlignedUtterance {
    AlignedUtterance {
        id: "U0007".into(),
        words: TimingSource::ALL.into_iter().map(aligned_word).collect(),
        speaker_starts: vec![0, 2],
        new_speaker: true,
        narrator: true,
        unsure: true,
    }
}

fn candidate(kind: CandidateKind) -> SoundCandidate {
    SoundCandidate {
        id: "S001".into(),
        kind,
        label: "Explosion".into(),
        start_s: 10.0,
        end_s: 11.5,
        peak: 0.75,
    }
}

fn cue(kind: CandidateKind) -> SoundCue {
    SoundCue {
        candidate: "S001".into(),
        kind,
        start_s: 10.0,
        end_s: 11.5,
        text: "[explosion]".into(),
    }
}

fn video_stream() -> VideoStream {
    VideoStream {
        index: 0,
        codec: "hevc".into(),
        width: 1920,
        height: 1080,
        frame_rate_num: 24000,
        frame_rate_den: 1001,
        start_time_s: 0.042,
        pix_fmt: Some("yuv420p10le".into()),
        color_primaries: Some("bt709".into()),
        color_transfer: Some("bt709".into()),
        color_space: Some("bt709".into()),
        color_range: Some("tv".into()),
        bit_rate: Some(4_000_000),
    }
}

fn audio_stream() -> AudioStream {
    AudioStream {
        index: 1,
        audio_position: 0,
        codec: "aac".into(),
        language: Some("eng".into()),
        channels: 2,
        sample_rate: 48_000,
        start_time_s: 0.0,
    }
}

fn probe() -> ProbeResult {
    ProbeResult {
        duration_s: 1440.5,
        video: Some(video_stream()),
        audio: vec![audio_stream()],
    }
}

fn every_verdict() -> Vec<FixVerdict> {
    let why = || "the judge agreed".to_string();
    vec![
        FixVerdict::Unchanged,
        FixVerdict::Kept { why: why() },
        FixVerdict::Accepted { why: why() },
        FixVerdict::TurnedDown { why: why() },
        FixVerdict::NotJudged { why: why() },
        FixVerdict::NotAnswered { why: why() },
    ]
}

/// Fails to compile when `FixVerdict` gains a variant, until [`every_verdict`] lists it too.
fn listed_verdict(verdict: &FixVerdict) {
    match verdict {
        FixVerdict::Unchanged
        | FixVerdict::Kept { .. }
        | FixVerdict::Accepted { .. }
        | FixVerdict::TurnedDown { .. }
        | FixVerdict::NotJudged { .. }
        | FixVerdict::NotAnswered { .. } => {}
    }
}

fn fix_step(family: FixFamily) -> FixStep {
    FixStep {
        family,
        text: "Wait for me!".into(),
        flags: strings(&["SPK"]),
        why: "the engines split the name".into(),
    }
}

fn line_fix(verdict: FixVerdict) -> LineFix {
    LineFix {
        id: "U0012".into(),
        problems: strings(&["unsure"]),
        checks: vec![QcCheck::Unsure, QcCheck::TooFast],
        before_text: "Wait for me".into(),
        before_flags: strings(&["UNSURE"]),
        after_text: "Wait for me!".into(),
        after_flags: strings(&["SPK"]),
        steps: FixFamily::ALL.into_iter().map(fix_step).collect(),
        refused: strings(&["a word no engine heard"]),
        removed: strings(&["uh"]),
        verdict,
        applied: true,
    }
}

fn brief() -> FixBrief {
    FixBrief {
        show: "One Piece (anime), English dub".into(),
        episode: "Dressrosa 11".into(),
        cast: strings(&["Luffy", "Rebecca"]),
        summary: "Luffy meets Rebecca in the colosseum.".into(),
        speech_habits: strings(&["Luffy shouts"]),
        suspects: vec![Suspect {
            id: "U0020".into(),
            why: "out of place".into(),
        }],
    }
}

fn before() -> FixBefore {
    FixBefore {
        counts: BTreeMap::from([(QcCheck::TooFast, 3), (QcCheck::Unsure, 2)]),
        first_uncovered_s: Some(88.5),
        cps_ok_share: 0.93,
        cues: 400,
    }
}

fn correction(chosen: Chosen) -> Correction {
    Correction {
        id: "U0412".into(),
        text: "Wait for me!".into(),
        flags: strings(&["SPK"]),
        chosen,
    }
}

fn every_chosen() -> Vec<Chosen> {
    let fix = || ("opus".to_string(), "the name was split".to_string());
    vec![
        Chosen::Engine("W".into()),
        Chosen::Typed,
        Chosen::FixIt {
            model: fix().0,
            why: fix().1,
        },
        Chosen::KeptFixIt {
            model: fix().0,
            why: fix().1,
        },
    ]
}

/// Fails to compile when `Chosen` gains a variant, until [`every_chosen`] lists it too.
fn listed_chosen(chosen: &Chosen) {
    match chosen {
        Chosen::Engine(_) | Chosen::Typed | Chosen::FixIt { .. } | Chosen::KeptFixIt { .. } => {}
    }
}

#[test]
fn line_round_trips() {
    round_trip(&line());
}

#[test]
fn findings_round_trip() {
    round_trip(&findings());
}

#[test]
fn adjudication_pass_round_trips() {
    round_trip(&AdjudicationPass {
        lines: vec![line()],
        findings: findings(),
        redecoded: strings(&["U0012"]),
        calls: 12,
        input_tokens: 50_000,
        output_tokens: 4_000,
        cost_usd: 0.35,
        failed_calls: strings(&["timed out"]),
    });
}

#[test]
fn redecode_round_trips() {
    round_trip(&Redecode {
        ids: strings(&["U0012"]),
        transcript: transcript(),
    });
}

#[test]
fn every_timing_source_round_trips() {
    for source in TimingSource::ALL {
        round_trip(&source);
    }
}

#[test]
fn aligned_word_round_trips() {
    for source in TimingSource::ALL {
        round_trip(&aligned_word(source));
    }
}

#[test]
fn aligned_utterance_round_trips() {
    round_trip(&aligned_utterance());
}

#[test]
fn aligned_round_trips() {
    round_trip(&Aligned {
        utterances: vec![aligned_utterance()],
        blocks: 40,
        failed_blocks: 1,
        offset_s: Some(-0.004),
        errors: strings(&["block 7: no path"]),
    });
}

#[test]
fn every_fix_family_round_trips() {
    for family in FixFamily::ALL {
        round_trip(&family);
    }
}

#[test]
fn suspect_round_trips() {
    round_trip(&Suspect {
        id: "U0020".into(),
        why: "out of place".into(),
    });
}

#[test]
fn fix_brief_round_trips() {
    round_trip(&brief());
}

#[test]
fn fix_step_round_trips() {
    for family in FixFamily::ALL {
        round_trip(&fix_step(family));
    }
}

#[test]
fn every_fix_verdict_round_trips() {
    for verdict in every_verdict() {
        listed_verdict(&verdict);
        round_trip(&verdict);
    }
}

#[test]
fn line_fix_round_trips() {
    for verdict in every_verdict() {
        round_trip(&line_fix(verdict));
    }
}

#[test]
fn fix_before_round_trips() {
    round_trip(&before());
}

#[test]
fn fix_record_round_trips() {
    round_trip(&FixRecord {
        model: "opus".into(),
        video: "Dressrosa 11".into(),
        brief: brief(),
        lines: every_verdict().into_iter().map(line_fix).collect(),
        calls: 9,
        cached_calls: 2,
        input_tokens: 90_000,
        output_tokens: 9_000,
        cost_usd: 1.25,
        failed_calls: strings(&["refused"]),
        adjudication: "sha256-readjudicate".into(),
        before: Some(before()),
    });
}

#[test]
fn output_record_round_trips() {
    round_trip(&OutputRecord {
        path: "/media/one pace/Dressrosa 11.ass".into(),
        unchanged: true,
        backup: Some("/media/one pace/Dressrosa 11.ass.bak".into()),
        retired: Some("/media/one pace/Dressrosa 11.srt".into()),
        localized: Some("/media/one pace/Dressrosa 11.localized.ass".into()),
    });
}

#[test]
fn probe_types_round_trip() {
    round_trip(&video_stream());
    round_trip(&audio_stream());
    round_trip(&probe());
    round_trip(&ProbeDecoded {
        probe: probe(),
        track: audio_stream(),
        samples: 23_000_000,
    });
}

#[test]
fn every_chosen_round_trips() {
    for chosen in every_chosen() {
        listed_chosen(&chosen);
        round_trip(&chosen);
    }
}

#[test]
fn correction_round_trips() {
    for chosen in every_chosen() {
        round_trip(&correction(chosen));
    }
}

#[test]
fn corrections_round_trip() {
    round_trip(&Corrections {
        lines: every_chosen().into_iter().map(correction).collect(),
    });
}

#[test]
fn utterance_round_trips() {
    round_trip(&Utterance {
        id: "U0012".into(),
        start_s: 1.5,
        end_s: 3.0,
        words: vec![timed_word("Straw"), timed_word("Hat!")],
        locked: vec![true, false],
        line: "Straw Hat!".into(),
        hypotheses: vec![
            ("P".into(), strings(&["Straw", "Hat!"])),
            ("W".into(), strings(&["Strawhat!"])),
        ],
    });
}

#[test]
fn shot_types_round_trip() {
    let cut = ShotCut {
        time_s: 12.5,
        score: 42.0,
    };
    round_trip(&cut);
    round_trip(&ShotChanges { cuts: vec![cut] });
}

#[test]
fn every_candidate_kind_round_trips() {
    for kind in EVERY_KIND {
        listed_kind(kind);
        round_trip(&kind);
    }
}

#[test]
fn sound_candidate_and_cue_round_trip() {
    for kind in EVERY_KIND {
        round_trip(&candidate(kind));
        round_trip(&cue(kind));
    }
}

#[test]
fn sound_cues_round_trip() {
    round_trip(&SoundCues {
        candidates: EVERY_KIND.into_iter().map(candidate).collect(),
        cues: EVERY_KIND.into_iter().map(cue).collect(),
        refused: strings(&["not a sound"]),
        failed_calls: strings(&["timed out"]),
        cost_usd: 0.05,
    });
}

#[test]
fn sound_event_round_trips() {
    round_trip(&SoundEvent {
        label: "Laughter".into(),
        stem: "vocals".into(),
        start_s: 5.0,
        end_s: 6.25,
        peak: 0.625,
    });
}

#[test]
fn speech_types_round_trip() {
    let span = TimeSpan::new(0.5, 4.25);
    round_trip(&span);
    round_trip(&SpeechPlan {
        frame_s: 0.032,
        threshold: 0.5,
        regions: vec![span],
        chunks: vec![TimeSpan::new(0.0, 30.0)],
    });
}

#[test]
fn word_types_round_trip() {
    round_trip(&timed_word("Luffy"));
    round_trip(&transcript().chunks[0]);
    round_trip(&transcript());
}
