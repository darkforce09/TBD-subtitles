use super::*;

#[test]
fn the_sampler_measures_this_process_s_memory_and_stops_at_once() {
    let sampler = JobSampler::start(std::process::id());
    sampler.open(StepName::Vad);
    // Busy work across at least two samples, so the CPU use has an interval to measure.
    let started = Instant::now();
    let mut spin = 0u64;
    while started.elapsed() < Duration::from_millis(600) {
        spin = spin.wrapping_add(1);
    }
    std::hint::black_box(spin);
    let used = sampler.close(StepName::Vad);
    assert!(used.job_ram_mib.is_some_and(|mib| mib > 0.0), "{used:?}");
    assert!(
        used.cpu_cores_mean.is_some_and(|cores| cores > 0.0),
        "{used:?}"
    );
    let stopping = Instant::now();
    let job = sampler.stop();
    assert!(stopping.elapsed() < Duration::from_millis(200));
    assert!(job.samples >= 2, "{job:?}");
    assert!(job.peak_pss_mib.is_some());
}
