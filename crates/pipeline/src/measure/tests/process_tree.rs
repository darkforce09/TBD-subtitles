use std::collections::BTreeMap;

use super::*;

#[test]
fn a_stat_line_parses_its_parent_and_ticks() {
    let line = "4242 (tbd-subtitles) S 1 4242 4242 0 -1 4194560 12 0 0 0 731 88 0 0 20 0 9 0 \
                1000 1234567 890 18446744073709551615";
    assert_eq!(
        parse_stat(line),
        Some(Stat {
            ppid: 1,
            utime: 731,
            stime: 88
        })
    );
}

#[test]
fn a_name_with_spaces_and_parentheses_parses_from_its_last_parenthesis() {
    let line = "77 (my (odd) name) (x) R 4242 77 77 0 -1 0 0 0 0 0 15 4 0 0 20 0 1 0 5 6 7";
    let stat = parse_stat(line).expect("parses");
    assert_eq!(stat.ppid, 4242);
    assert_eq!(stat.ticks(), 19);
}

#[test]
fn a_cut_stat_line_does_not_parse() {
    assert_eq!(parse_stat("12 (ffmpeg) S 1 12"), None);
    assert_eq!(parse_stat("no parenthesis here"), None);
}

#[test]
fn smaps_rollup_gives_its_pss_line() {
    let rollup = "55d0c0000000-7ffc00000000 ---p 00000000 00:00 0    [rollup]\n\
                  Rss:             2048000 kB\n\
                  Pss:             1536512 kB\n\
                  Pss_Anon:        1000000 kB\n";
    assert_eq!(parse_pss_kib(rollup), Some(1_536_512));
    assert_eq!(parse_pss_kib("Rss: 5 kB\n"), None);
}

#[test]
fn descendants_follow_every_generation_and_nothing_else() {
    let parents = BTreeMap::from([(10, 1), (20, 10), (30, 20), (31, 20), (40, 1), (50, 40)]);
    let tree: Vec<u32> = descendants(10, &parents).into_iter().collect();
    assert_eq!(tree, vec![10, 20, 30, 31]);
}

#[test]
fn this_process_is_in_its_own_tree_with_memory_and_threads() {
    let me = std::process::id();
    let stats = tree_stats(me);
    assert!(stats.contains_key(&me));
    assert!(pss_mib([me]).is_some_and(|mib| mib > 0.0));
    assert!(!thread_ticks([me]).is_empty());
    assert!(clock_ticks() > 0.0);
}

#[test]
fn a_vanished_pid_is_skipped() {
    assert_eq!(pss_mib([u32::MAX]), None);
    assert!(thread_ticks([u32::MAX]).is_empty());
}
