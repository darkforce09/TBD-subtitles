use super::*;

fn ceil_log2(k: u64) -> usize {
    if k <= 1 {
        0
    } else {
        ((k - 1).ilog2() + 1) as usize
    }
}

#[test]
fn bisection_finds_every_entry_and_exit_within_ceil_log2_k_probes() {
    for k in [1u64, 5, 12, 25] {
        for seek in [Seek::Entry, Seek::Exit] {
            for change in 1..=k {
                let base = 10 * k;
                let mut searches = [Search::new(base, base + k, seek)];
                let probes = std::cell::Cell::new(0usize);
                bisect(
                    &mut searches,
                    &mut (),
                    |_, indices| {
                        probes.set(probes.get() + indices.len());
                        Ok(())
                    },
                    |_, _, index| match seek {
                        Seek::Entry => index >= base + change,
                        Seek::Exit => index < base + change,
                    },
                )
                .unwrap();
                assert_eq!(searches[0].hi, base + change, "k {k}, {seek:?} at {change}");
                assert!(
                    probes.get() <= ceil_log2(k),
                    "k {k}: {} probes",
                    probes.get()
                );
            }
        }
    }
}

#[test]
fn transitions_in_different_gaps_advance_in_lockstep_with_one_screen_per_step() {
    let mut searches = [
        Search::new(0, 12, Seek::Entry),
        Search::new(24, 36, Seek::Exit),
        Search::new(48, 60, Seek::Entry),
        Search::new(0, 12, Seek::Entry),
    ];
    let changes = [5, 31, 60, 5];
    let mut calls: Vec<Vec<u64>> = Vec::new();
    bisect(
        &mut searches,
        &mut calls,
        |calls, indices| {
            calls.push(indices.to_vec());
            Ok(())
        },
        |calls, search, index| {
            assert!(
                calls.last().unwrap().contains(&index),
                "probed in this step"
            );
            match search {
                1 => index < changes[search],
                _ => index >= changes[search],
            }
        },
    )
    .unwrap();
    let found: Vec<u64> = searches.iter().map(|search| search.hi).collect();
    assert_eq!(found, changes);
    assert_eq!(calls.len(), ceil_log2(12), "one screening call per step");
    assert_eq!(calls[0], [6, 30, 54], "one deduplicated probe per gap");
    for call in &calls {
        let gaps: Vec<u64> = call.iter().map(|index| index / 12).collect();
        let mut distinct = gaps.clone();
        distinct.dedup();
        assert_eq!(gaps, distinct, "at most one probe per gap here: {call:?}");
    }
    assert_eq!(calls[3], [5, 59]);
}
