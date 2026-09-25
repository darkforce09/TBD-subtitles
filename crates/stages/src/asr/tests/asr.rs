use job_model::outputs::TimeSpan;

use super::*;

struct Fixed;

impl SpeechEngine for Fixed {
    fn name(&self) -> String {
        "fixed".into()
    }
    fn transcribe(&mut self, samples: &[f32]) -> Result<Vec<TimedWord>, String> {
        let seconds = samples.len() as f64 / RATE;
        Ok(vec![TimedWord {
            text: format!("{seconds:.1}"),
            start_s: 0.5,
            end_s: seconds + 5.0,
            confidence: Some(0.9),
        }])
    }
}

#[test]
fn every_chunk_is_heard_and_timed_in_video_seconds() {
    let dir = std::env::temp_dir().join(format!("asr-plan-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let audio = dir.join("mix.f32");
    let samples: Vec<u8> = (0..16_000 * 10)
        .flat_map(|_| 0.1f32.to_le_bytes())
        .collect();
    std::fs::write(&audio, samples).unwrap();
    let plan = SpeechPlan {
        chunks: vec![TimeSpan::new(1.0, 3.0), TimeSpan::new(4.0, 9.0)],
        ..SpeechPlan::default()
    };
    let transcript = transcribe_plan(&mut Fixed, &audio, "mix", &plan, |_, _| {}).unwrap();
    assert_eq!(transcript.text(), "2.0 5.0");
    let second = &transcript.chunks[1].words[0];
    assert_eq!(second.start_s, 4.5);
    assert_eq!(second.end_s, 9.0, "clamped to the chunk's end");
    std::fs::remove_dir_all(&dir).unwrap();
}
