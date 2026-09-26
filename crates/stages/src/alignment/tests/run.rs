use super::*;
use crate::alignment::blocks::Kept;
use job_model::outputs::TimedWord;

fn kept_at(id: &str, sheet_index: usize, start_s: f64, end_s: f64, words: &[&str]) -> Kept {
    let step = (end_s - start_s) / words.len() as f64;
    Kept {
        id: id.into(),
        sheet_index,
        start_s,
        end_s,
        words: words.iter().map(|w| w.to_string()).collect(),
        speaker_starts: vec![],
        new_speaker: false,
        narrator: false,
        unsure: false,
        backbone: words
            .iter()
            .enumerate()
            .map(|(i, w)| TimedWord {
                text: w.to_string(),
                start_s: start_s + i as f64 * step,
                end_s: start_s + (i + 1) as f64 * step,
                confidence: None,
            })
            .collect(),
    }
}

/// Times each word evenly over its span shifted by `shift`, fails calls of `refuse_words` words,
/// and remembers what it was asked.
struct Scripted {
    shift: f64,
    refuse_words: usize,
    calls: Vec<(TimeSpan, usize)>,
}

impl WordAligner for Scripted {
    fn align(&mut self, span: TimeSpan, words: &[String]) -> Result<Option<WordTimes>, String> {
        self.calls.push((span, words.len()));
        if words.len() == self.refuse_words {
            return Err("model failed".into());
        }
        let step = (span.end_s - span.start_s - 0.6) / words.len() as f64;
        Ok(Some(
            (0..words.len())
                .map(|i| {
                    let start = span.start_s + 0.3 + i as f64 * step + self.shift;
                    Some((start, start + step * 0.9))
                })
                .collect(),
        ))
    }
}

#[test]
fn a_good_block_is_timed_by_the_aligner_and_measures_the_offset() {
    let kept = vec![
        kept_at("U1", 0, 1.0, 2.0, &["a", "b"]),
        kept_at("U2", 1, 2.1, 3.0, &["c"]),
    ];
    let mut aligner = Scripted {
        shift: 0.0,
        refuse_words: 99,
        calls: vec![],
    };
    let aligned = align_all(&kept, 100.0, &mut aligner, |_, _| {});
    assert_eq!(aligned.blocks, 1);
    assert_eq!(aligned.failed_blocks, 0);
    assert_eq!(aligner.calls.len(), 1);
    assert!(
        aligned
            .utterances
            .iter()
            .flat_map(|u| &u.words)
            .all(|w| w.source == TimingSource::Ctc)
    );
    assert!(aligned.offset_s.is_some());
}

#[test]
fn a_failed_block_falls_back_to_utterances_then_to_the_backbone() {
    let kept = vec![
        kept_at("U1", 0, 1.0, 2.0, &["a", "b"]),
        kept_at("U2", 1, 2.1, 3.0, &["c"]),
    ];
    // The block call (3 words) errors; U1 (2 words) aligns alone; U2 (1 word) is shifted too far.
    let mut aligner = Scripted {
        shift: 0.0,
        refuse_words: 3,
        calls: vec![],
    };
    let aligned = align_all(&kept, 100.0, &mut aligner, |_, _| {});
    assert_eq!(aligned.failed_blocks, 1);
    assert_eq!(aligned.errors.len(), 1);
    assert_eq!(aligner.calls.len(), 3);
    assert!(
        aligned.utterances[0]
            .words
            .iter()
            .all(|w| w.source == TimingSource::CtcUtterance)
    );
    assert_eq!(aligned.offset_s, None);

    let mut far = Scripted {
        shift: 1.5,
        refuse_words: 99,
        calls: vec![],
    };
    let aligned = align_all(&kept, 100.0, &mut far, |_, _| {});
    assert!(
        aligned
            .utterances
            .iter()
            .flat_map(|u| &u.words)
            .all(|w| w.source == TimingSource::Backbone)
    );
    assert_eq!(aligned.utterances[1].words[0].start_s, 2.1);
}

#[test]
fn every_word_gets_a_time_in_order() {
    let kept = vec![kept_at("U1", 0, 1.0, 3.0, &["one", "two", "three", "four"])];
    let mut aligner = Scripted {
        shift: 0.0,
        refuse_words: 99,
        calls: vec![],
    };
    let aligned = align_all(&kept, 100.0, &mut aligner, |_, _| {});
    let words = &aligned.utterances[0].words;
    assert_eq!(words.len(), 4);
    assert!(words.windows(2).all(|w| w[0].start_s <= w[1].start_s));
}
