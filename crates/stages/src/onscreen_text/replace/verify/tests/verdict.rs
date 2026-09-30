use super::*;

fn line(text: &str, confidence: f64) -> ReadLine {
    ReadLine {
        text: text.into(),
        confidence,
        over_lettering: true,
        over_writing: true,
    }
}

#[test]
fn japanese_counts_only_where_the_writing_was_and_english_only_over_the_lettering() {
    let below = ReadLine {
        over_writing: false,
        over_lettering: false,
        ..line("じゅう", 0.9)
    };
    let aside = ReadLine {
        over_lettering: false,
        ..line("GDANV", 0.9)
    };
    let reading = judge(4, &[line("Samurai of", 0.99), below, aside], "Samurai of");
    assert!(reading.passed, "{reading:?}");
    assert!(reading.japanese_found.is_empty());
    assert_eq!(reading.english_read, "Samurai of");
    let ruby = ReadLine {
        over_lettering: false,
        ..line("きゅう", 0.9)
    };
    let left = judge(5, &[ruby, line("Former", 0.95)], "Former");
    assert_eq!(left.japanese_found, "きゅう");
    assert!(!left.passed);
}

#[test]
fn kana_and_kanji_count_but_marks_and_latin_do_not() {
    assert_eq!(japanese_chars("幹部塔"), 3);
    assert_eq!(japanese_chars("ワノ国の侍"), 5);
    assert_eq!(japanese_chars("ひらがな"), 4);
    assert_eq!(japanese_chars("ｽｸﾗｯﾌﾟ"), 5);
    assert_eq!(japanese_chars("Kin'emon ・ー Side"), 0);
    assert_eq!(japanese_chars("A口B"), 1);
}

#[test]
fn letters_keep_lowercase_ascii_letters_and_digits_only() {
    assert_eq!(letters("Kin'emon's Side"), "kinemonsside");
    assert_eq!(letters("Level 1, Colosseum!"), "level1colosseum");
    assert_eq!(letters("Ｒｅｂｅｃｃａ　２"), "rebecca2");
    assert_eq!(letters("Ryūma Café"), "ryumacafe");
    assert_eq!(letters("王宮"), "");
}

#[test]
fn similarity_is_one_minus_edit_distance_over_the_longer_side() {
    assert_eq!(similarity("KIN'EMON'S SIDE", "Kin'emon's Side"), 1.0);
    assert!((similarity("kinemon side", "kinemons side") - (1.0 - 1.0 / 12.0)).abs() < 1e-9);
    assert_eq!(similarity("", ""), 1.0);
    assert_eq!(similarity("", "Palace"), 0.0);
    assert_eq!(similarity("王宮", "Palace"), 0.0);
}

#[test]
fn the_line_above_read_with_the_lettering_does_not_count_against_it() {
    let reading = judge(
        4502,
        &[line("Wano Country Yudachi Kanjuro", 0.93)],
        "Yudachi Kanjuro",
    );
    assert_eq!(reading.similarity, 1.0);
    assert!(reading.passed);
    assert!((similarity("Executive A Tower", "Executive Tower") - 13.0 / 14.0).abs() < 1e-9);
    assert!((similarity("Executive er", "Executive Tower") - 11.0 / 14.0).abs() < 1e-9);
}

#[test]
fn doubled_lettering_does_not_read_back() {
    let reading = judge(
        7,
        &[line("kin'emon side", 0.95), line("kin'emon side", 0.93)],
        "Kin'emon Side",
    );
    assert!(reading.similarity < MIN_SIMILARITY);
    assert!(!reading.passed);
    assert!(reading.japanese_found.is_empty());
    assert_eq!(reading.english_read, "kin'emon side kin'emon side");
    assert_eq!(verdict(&[reading]), Some(UNREADABLE));
}

#[test]
fn a_clean_frame_passes_with_small_misreads() {
    let reading = judge(
        3,
        &[line("Samurai of", 0.97), line("Wano Countrv", 0.9)],
        "Samurai of Wano Country",
    );
    assert!(reading.passed, "{reading:?}");
    assert_eq!(verdict(&[reading]), None);
}

#[test]
fn confident_japanese_fails_the_frame_but_a_weak_or_single_character_does_not() {
    let english = "Executive Tower";
    let left = judge(
        1,
        &[line("Executive Tower", 0.95), line("幹部塔", 0.8)],
        english,
    );
    assert_eq!(left.japanese_found, "幹部塔");
    assert!(!left.passed);
    let weak = judge(
        2,
        &[line("Executive Tower", 0.95), line("幹部", 0.3)],
        english,
    );
    assert!(weak.japanese_found.is_empty());
    let single = judge(3, &[line("Executive Tower 口", 0.95)], english);
    assert!(single.japanese_found.is_empty());
    assert!(single.passed);
}

#[test]
fn japanese_decides_the_reason_over_a_bad_read_back() {
    let bad = judge(1, &[line("garbled", 0.9)], "Scrapyard");
    let japanese = judge(
        2,
        &[line("Scrapyard", 0.9), line("スクラップ", 0.9)],
        "Scrapyard",
    );
    assert_eq!(verdict(&[bad.clone(), japanese]), Some(JAPANESE_LEFT));
    assert_eq!(verdict(&[bad]), Some(UNREADABLE));
    assert_eq!(verdict(&[]), None);
}
