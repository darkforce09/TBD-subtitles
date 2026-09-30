use super::{StageName, StepName};
use crate::archive_round_trip::round_trip;

#[test]
fn every_stage_name_round_trips() {
    for stage in StageName::ALL {
        round_trip(&stage);
    }
}

#[test]
fn every_step_name_round_trips() {
    for step in StepName::ALL {
        round_trip(&step);
    }
}

#[test]
fn archived_step_names_keep_the_run_order() {
    let archived: Vec<_> = StepName::ALL
        .into_iter()
        .map(|step| rkyv::to_bytes::<rkyv::rancor::Error>(&step).unwrap())
        .collect();
    let names: Vec<_> = archived
        .iter()
        .map(|bytes| rkyv::access::<rkyv::Archived<StepName>, rkyv::rancor::Error>(bytes).unwrap())
        .collect();
    assert!(names.windows(2).all(|pair| pair[0] < pair[1]));
    for (name, step) in names.iter().zip(StepName::ALL) {
        assert_eq!(*name, &step);
    }
}
