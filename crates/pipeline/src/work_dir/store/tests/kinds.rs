use std::collections::BTreeMap;

use job_model::job::{JobSettings, StepMeasure};
use job_model::outputs::{AudioStream, ProbeResult};
use rkyv::api::high::HighSerializer;
use rkyv::ser::allocator::ArenaHandle;
use rkyv::util::AlignedVec;
use subtitle_formats::cue::FrameRate;

use super::*;

/// Bytes no archive of any stored type checks: every relative pointer runs out of the buffer, and
/// as an inline string they are not UTF-8 (all `0xff` would be the empty string, whose inline
/// form pads with `0xff`).
const GARBAGE: [u8; 40] = [0xc3; 40];

fn archive<T>(value: &T) -> Vec<u8>
where
    T: for<'a> rkyv::Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, ArchiveError>>,
{
    rkyv::to_bytes::<ArchiveError>(value)
        .expect("the value archives")
        .to_vec()
}

fn name(text: &str) -> Key {
    Key::Name(text.into())
}

fn probe_decoded() -> ProbeDecoded {
    let track = AudioStream {
        index: 1,
        audio_position: 0,
        codec: "aac".into(),
        language: Some("eng".into()),
        channels: 2,
        sample_rate: 48_000,
        start_time_s: 0.0,
    };
    ProbeDecoded {
        probe: ProbeResult {
            duration_s: 1_440.5,
            video: None,
            audio: vec![track.clone()],
        },
        track,
        samples: 23_048_000,
    }
}

fn step_record() -> StepRecord {
    StepRecord {
        fingerprint: "ab12".into(),
        finished_ns: 7,
        measure: StepMeasure {
            wall_s: 1.0,
            notes: BTreeMap::new(),
            ..StepMeasure::default()
        },
    }
}

fn job_record() -> JobRecord {
    JobRecord {
        video: "/videos/episode.mkv".into(),
        video_size: 9,
        video_modified_s: 3,
        settings: JobSettings::with_glossary(Vec::new()),
        models_dir: None,
        corrections: None,
    }
}

/// Every document a task or the owner writes today, by where it is stored, with a sample archive.
fn every_document() -> Vec<(Table, &'static str, Vec<u8>)> {
    let text = archive(&TextDocument::default());
    let replacement = archive(&ReplacementDocument::default());
    let transcript = archive(&EngineTranscript::default());
    let pass = archive(&AdjudicationPass::default());
    let redecode = archive(&Redecode::default());
    let aligned = archive(&Aligned::default());
    let cues = CueTrack {
        frame_rate: FrameRate::FILM,
        cues: Vec::new(),
    };
    vec![
        (Table::Meta, "job_record", archive(&job_record())),
        (Table::Meta, "layout", archive(&TableLayouts::default())),
        (Table::StepRecords, "text_verify", archive(&step_record())),
        (
            Table::Corrections,
            "lines",
            archive(&Corrections::default()),
        ),
        (
            Table::Corrections,
            "text",
            archive(&TextCorrections::default()),
        ),
        (Table::Corrections, "fix", archive(&FixRecord::default())),
        (Table::Outputs, "probe_decode", archive(&probe_decoded())),
        (
            Table::Outputs,
            "shot_scan",
            archive(&ShotChanges::default()),
        ),
        (Table::Outputs, "vad", archive(&SpeechPlan::default())),
        (Table::Outputs, "asr_parakeet", transcript.clone()),
        (Table::Outputs, "asr_whisper", transcript),
        (
            Table::Outputs,
            "diff_sheet",
            archive(&Vec::<Utterance>::new()),
        ),
        (
            Table::Outputs,
            "sound_events",
            archive(&Vec::<SoundEvent>::new()),
        ),
        (Table::Outputs, "adjudicate", pass.clone()),
        (Table::Outputs, "redecode_parakeet", redecode.clone()),
        (Table::Outputs, "redecode_whisper", redecode),
        (Table::Outputs, "readjudicate", pass),
        (Table::Outputs, "sound_cues", archive(&SoundCues::default())),
        (Table::Outputs, "alignment", aligned.clone()),
        (Table::Outputs, "review", aligned),
        (Table::Outputs, "cues", archive(&cues)),
        (
            Table::Outputs,
            "cues/dropped_sounds",
            archive(&vec!["[THUD]".to_string()]),
        ),
        (Table::Outputs, "text_detect", text.clone()),
        (Table::Outputs, "text_read", text.clone()),
        (Table::Outputs, "text_track", text.clone()),
        (Table::Outputs, "text_translate", text.clone()),
        (Table::Outputs, "text_review", text.clone()),
        (Table::Outputs, "text_mask", replacement.clone()),
        (Table::Outputs, "text_inpaint", replacement.clone()),
        (Table::Outputs, "text_compose", replacement),
        (
            Table::Outputs,
            "text_verify",
            archive(&VerifiedReplacements::default()),
        ),
        (Table::Outputs, "text_typeset", text),
        (
            Table::Outputs,
            "text_typeset/ass",
            archive(&"Dialogue: 0,0:00:01.00".to_string()),
        ),
        (Table::Outputs, "qc", archive(&QcReport::default())),
        (Table::Outputs, "output", archive(&OutputRecord::default())),
        (
            Table::Outputs,
            "localized_video",
            archive(&LocalizedVideoRecord::default()),
        ),
    ]
}

#[test]
fn every_document_a_task_writes_has_a_kind_that_checks_it_and_refuses_garbage() {
    for (table, key, bytes) in every_document() {
        let kind = kind(table, &name(key)).unwrap_or_else(|error| panic!("{table} {key}: {error}"));
        kind.check(&bytes)
            .unwrap_or_else(|error| panic!("{table} {key} ({}): {error}", kind.name));
        kind.json(&bytes)
            .unwrap_or_else(|error| panic!("{table} {key} ({}): {error}", kind.name));
        assert!(
            kind.check(&GARBAGE).is_err(),
            "{table} {key} ({}) accepts garbage",
            kind.name
        );
    }
}

#[test]
fn every_step_but_separation_has_an_output_document() {
    let documents = every_document();
    for step in StepName::ALL {
        let listed = documents
            .iter()
            .any(|(table, key, _)| *table == Table::Outputs && *key == step.as_str());
        let has_kind = kind(Table::Outputs, &name(step.as_str())).is_ok();
        assert_eq!(listed, step != StepName::Separation, "{step}");
        assert_eq!(has_kind, listed, "{step}");
        assert!(
            kind(Table::StepRecords, &name(step.as_str())).is_ok(),
            "{step}"
        );
    }
}

#[test]
fn every_output_key_of_every_step_has_a_kind_and_a_sample() {
    let documents = every_document();
    for step in StepName::ALL {
        for key in keys::output_keys(step) {
            assert!(kind(Table::Outputs, &key).is_ok(), "{key:?}");
            let Key::Name(key) = key else {
                panic!("an output key is a name")
            };
            assert!(
                documents
                    .iter()
                    .any(|(table, listed, _)| *table == Table::Outputs && *listed == key),
                "{key} has no sample"
            );
        }
    }
}

#[test]
fn a_kind_prints_its_record_as_json() {
    let value = STEP_RECORD.json(&archive(&step_record())).expect("json");
    assert_eq!(value["fingerprint"], "ab12");
    assert_eq!(value["measure"]["wall_s"], 1.0);
    let probe = kind(Table::Outputs, &name("probe_decode"))
        .expect("kind")
        .json(&archive(&probe_decoded()))
        .expect("json");
    assert_eq!(probe["samples"], 23_048_000);
}

#[test]
fn unknown_keys_are_errors() {
    for (table, key) in [
        (Table::Meta, "settings"),
        (Table::StepRecords, "not_a_step"),
        (Table::Corrections, "words"),
        (Table::Outputs, "separation"),
        (Table::Outputs, "not_a_step"),
        (Table::Outputs, "cues/other"),
        (Table::Outputs, "probe_decode/dropped_sounds"),
        (Table::Outputs, "text_typeset/ass_localized"),
        (Table::Outputs, "fix"),
        (Table::Outputs, ""),
    ] {
        let error = kind(table, &name(key)).expect_err(key);
        assert!(
            error.message.contains("no record type"),
            "{table} {key}: {error}"
        );
    }
    let frame = Key::Frame {
        occurrence: "o1".into(),
        frame: 4,
    };
    let error = kind(Table::Frames, &frame).expect_err("frames");
    assert!(
        error
            .message
            .contains("no record type is defined for the frames table yet"),
        "{error}"
    );
    assert!(kind(Table::Readings, &frame).is_err());
    let error = kind(Table::Outputs, &frame).expect_err("a per-frame key");
    assert!(error.message.contains("per-frame key"), "{error}");
}

#[test]
fn a_key_is_shown_as_the_owner_writes_it() {
    assert_eq!(shown(&name("vad")), "vad");
    let frame = Key::Frame {
        occurrence: "o1".into(),
        frame: 40,
    };
    assert_eq!(shown(&frame), "o1/40");
}
