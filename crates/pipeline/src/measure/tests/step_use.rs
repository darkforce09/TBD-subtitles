use std::collections::BTreeMap;

use super::*;

fn ticks(pairs: &[(u32, u64)]) -> BTreeMap<u32, u64> {
    pairs.iter().copied().collect()
}

#[test]
fn cpu_use_counts_cores_and_the_hottest_thread() {
    let before_p = ticks(&[(1, 100), (2, 50)]);
    let after_p = ticks(&[(1, 150), (2, 75)]);
    let before_t = ticks(&[(1, 60), (11, 40), (2, 50)]);
    let after_t = ticks(&[(1, 70), (11, 80), (2, 75)]);
    let (cores, hot) =
        cpu_use((&before_p, &before_t), (&after_p, &after_t), 0.5, 100.0).expect("time passed");
    // 75 ticks in 0.5 s at 100 ticks a second: 1.5 cores; thread 11 used 40 ticks, 80 %.
    assert!((cores - 1.5).abs() < 1e-9, "{cores}");
    assert!((hot - 80.0).abs() < 1e-9, "{hot}");
}

#[test]
fn a_new_pid_counts_in_full_and_a_gone_one_counts_nothing() {
    let before = ticks(&[(1, 100), (2, 500)]);
    let after = ticks(&[(1, 110), (3, 15)]);
    let none = BTreeMap::new();
    let (cores, hot) = cpu_use((&before, &none), (&after, &none), 1.0, 100.0).unwrap();
    assert!((cores - 0.25).abs() < 1e-9, "{cores}");
    assert_eq!(hot, 0.0);
}

#[test]
fn a_reused_pid_whose_ticks_fell_counts_nothing() {
    let before = ticks(&[(1, 900)]);
    let after = ticks(&[(1, 10)]);
    let (cores, _) = cpu_use((&before, &before), (&after, &after), 1.0, 100.0).unwrap();
    assert_eq!(cores, 0.0);
}

#[test]
fn no_elapsed_time_measures_nothing() {
    let none = BTreeMap::new();
    assert_eq!(cpu_use((&none, &none), (&none, &none), 0.0, 100.0), None);
}

fn sample(gpu: Option<f64>, pss: f64, cpu: f64) -> Sample {
    Sample {
        gpu_pct: gpu,
        encoder_pct: gpu.map(|g| g / 2.0),
        decoder_pct: None,
        pss_mib: Some(pss),
        cpu_cores: Some(cpu),
        hot_thread_pct: Some(cpu * 10.0),
    }
}

#[test]
fn a_window_takes_means_and_peaks_of_its_samples() {
    let mut windows = Windows::default();
    windows.open(StepName::Separation);
    windows.add(&sample(Some(80.0), 1000.0, 2.0));
    windows.add(&sample(Some(100.0), 3000.0, 4.0));
    let used = windows.close(StepName::Separation);
    assert_eq!(used.gpu_busy_pct, Some(90.0));
    assert_eq!(used.gpu_encoder_pct, Some(45.0));
    assert_eq!(used.gpu_decoder_pct, None);
    assert_eq!(used.job_ram_mib, Some(3000.0));
    assert_eq!(used.cpu_cores_mean, Some(3.0));
    assert_eq!(used.cpu_cores_peak, Some(4.0));
    assert_eq!(used.busiest_thread_pct, Some(30.0));
}

#[test]
fn overlapping_windows_each_take_the_samples_while_open() {
    let mut windows = Windows::default();
    windows.open(StepName::ShotScan);
    windows.add(&sample(None, 500.0, 1.0));
    windows.open(StepName::Separation);
    windows.add(&sample(None, 2500.0, 3.0));
    let scan = windows.close(StepName::ShotScan);
    windows.add(&sample(None, 4000.0, 5.0));
    let separation = windows.close(StepName::Separation);
    assert_eq!(scan.cpu_cores_mean, Some(2.0));
    assert_eq!(scan.job_ram_mib, Some(2500.0));
    assert_eq!(separation.cpu_cores_mean, Some(4.0));
    assert_eq!(separation.job_ram_mib, Some(4000.0));
    assert_eq!(windows.job().samples(), 3);
    assert_eq!(windows.job().usage().job_ram_mib, Some(4000.0));
}

#[test]
fn without_nvml_the_gpu_stays_unmeasured_never_zero() {
    let mut windows = Windows::default();
    windows.open(StepName::Vad);
    windows.add(&sample(None, 100.0, 1.0));
    let used = windows.close(StepName::Vad);
    assert_eq!(used.gpu_busy_pct, None);
    assert_eq!(used.gpu_encoder_pct, None);
    assert_eq!(used.job_ram_mib, Some(100.0));
}

#[test]
fn a_window_with_no_sample_and_one_never_opened_measure_nothing() {
    let mut windows = Windows::default();
    windows.open(StepName::Cues);
    assert_eq!(windows.close(StepName::Cues), StepUse::default());
    assert_eq!(windows.close(StepName::Qc), StepUse::default());
}

#[test]
fn a_step_use_fills_its_measure() {
    let mut measure = StepMeasure {
        wall_s: 3.0,
        ..StepMeasure::default()
    };
    StepUse {
        gpu_busy_pct: Some(50.0),
        job_ram_mib: Some(700.0),
        cpu_cores_peak: Some(6.0),
        ..StepUse::default()
    }
    .apply(&mut measure);
    assert_eq!(measure.wall_s, 3.0);
    assert_eq!(measure.gpu_busy_pct, Some(50.0));
    assert_eq!(measure.job_ram_mib, Some(700.0));
    assert_eq!(measure.cpu_cores_peak, Some(6.0));
    assert_eq!(measure.cpu_cores_mean, None);
}
