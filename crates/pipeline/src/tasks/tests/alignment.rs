//! The alignment task in a worker reads exactly what `graph::reads` sends it. No model loads.

use job_model::outputs::{AudioStream, ProbeDecoded, ProbeResult};
use worker_channel::address::{Address, Table};

use super::*;
use crate::graph;
use crate::work_dir::store::scratch::{Scratch, job_record};

fn probe() -> ProbeDecoded {
    let track = AudioStream {
        index: 1,
        audio_position: 0,
        codec: "aac".into(),
        language: None,
        channels: 2,
        sample_rate: 48_000,
        start_time_s: 0.0,
    };
    ProbeDecoded {
        probe: ProbeResult {
            duration_s: 2.0,
            video: None,
            audio: vec![track.clone()],
        },
        track,
        samples: 32_000,
    }
}

/// A job whose every step before the alignment is stored.
fn before_alignment() -> Scratch {
    let scratch = Scratch::new("alignment-reads");
    let store = scratch.store();
    store.put_job_record(&job_record(&scratch.dir)).unwrap();
    store
        .put_output(StepName::ProbeDecode, None, &probe())
        .unwrap();
    for engine in [StepName::AsrParakeet, StepName::AsrWhisper] {
        store
            .put_output(engine, None, &EngineTranscript::default())
            .unwrap();
    }
    store
        .put_output(StepName::DiffSheet, None, &Vec::<Utterance>::new())
        .unwrap();
    store
        .put_output(StepName::Readjudicate, None, &AdjudicationPass::default())
        .unwrap();
    scratch
}

/// The archives a worker of the alignment receives, as the runner reads them from `scratch`.
fn worker_inputs(scratch: &Scratch) -> Vec<(Address, Vec<u8>)> {
    let read = scratch.store().read().unwrap();
    graph::reads(StepName::Alignment)
        .into_iter()
        .map(|address| {
            let bytes = read.raw(address.table, &address.key).unwrap();
            (address.clone(), bytes.expect("every read is stored"))
        })
        .collect()
}

#[test]
fn the_worker_inputs_of_the_alignment_hold_everything_the_task_reads() {
    let scratch = before_alignment();
    let inputs = worker_inputs(&scratch);
    let io = StepIo::in_worker(inputs.clone());
    let (kept, duration) = read_inputs(&io).expect("every input is there");
    assert!(kept.is_empty());
    assert_eq!(duration, 2.0);
    for (dropped, _) in inputs.iter().filter(|(a, _)| a.table == Table::Outputs) {
        let fewer: Vec<_> = inputs
            .iter()
            .filter(|(address, _)| address != dropped)
            .cloned()
            .collect();
        let io = StepIo::in_worker(fewer);
        let error = read_inputs(&io).expect_err("an input is missing");
        assert!(
            error.to_string().contains("is not among the inputs"),
            "the task reads {dropped:?}: {error}"
        );
    }
}
