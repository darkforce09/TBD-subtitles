//! The detector-pool sections of the benchmark's printout.
//!
//! **Role:** run and print, in order: the CUDA pool-by-batch sweep; one session against two at
//! the chosen shape; the fast search against the deterministic one, each run twice; TensorRT
//! FP16 and FP32 over the same sweep; and confirmation on stills by CUDA and both TensorRT
//! precisions. Boxes are compared with a CUDA reference run, two sessions at the chosen shape.
//! **Position:** called by `detect_bench::run` with the padded frames; measures through
//! `measure.rs` and lays rows out through `rows.rs`.
//! **Signals and state:** the measured sweep rows, which choose the shape, and the reference
//! regions, which it returns for the box images.
//! **Invariants:** a section that is off runs nothing; a failed row never stops the next; the
//! reference run is made once, by the first section that needs it.

use inference::ocr::SearchMode;
use inference::ocr::pool::{PaddedFrame, SCREEN_SESSIONS, ScreenShape};
use job_model::onscreen::DetectorEngine;

use super::super::table::Table;
use super::Setup;
use super::compare::{Regions, compare, describe};
use super::measure;
use super::plan::{self, PoolRun};
use super::rows::{COLUMNS, Measured, Phase, row};

/// Which pool sections run, and with what.
#[derive(Debug, Clone, PartialEq)]
pub struct Sections {
    /// The batch-by-pool grid the CUDA and TensorRT sweeps run.
    pub grid: Vec<ScreenShape>,
    /// The shape the later sections run at, when the caller gives one.
    pub shape: Option<ScreenShape>,
    pub cuda_sweep: bool,
    pub sessions: bool,
    pub search: bool,
    pub tensorrt: bool,
    pub confirm: bool,
    /// The GPU memory pools confirmation runs at, in MiB.
    pub confirm_pools_mib: Vec<usize>,
    /// Whether the CUDA reference run is wanted even when no section compares against it.
    pub reference: bool,
}

/// Run every section that is on; the CUDA reference run's regions, when one was made.
pub fn run(
    setup: &Setup,
    frames: &[PaddedFrame],
    stills: &[PaddedFrame],
    sections: &Sections,
) -> Option<Regions> {
    let measured = if sections.cuda_sweep {
        sweep(setup, frames, &sections.grid)
    } else {
        Vec::new()
    };
    let shape = plan::chosen(sections.shape, &measured);
    let source = match (sections.shape, measured.is_empty()) {
        (Some(_), _) => "given",
        (None, false) => "the sweep's fastest",
        (None, true) => "the production default",
    };
    println!(
        "Chosen shape: batch {}, pool {} MiB ({source}).\n",
        shape.batch, shape.pool_mib
    );
    let mut reference = None;
    if sections.sessions {
        reference = one_against_two(setup, frames, shape);
    }
    if sections.search {
        let fast = search(setup, frames, shape);
        reference = reference.or(fast);
    }
    if (sections.tensorrt || sections.reference) && reference.is_none() {
        reference = reference_run(setup, frames, shape);
    }
    if sections.tensorrt {
        tensorrt(setup, frames, &sections.grid, reference.as_ref());
    }
    if sections.confirm {
        confirmation(
            setup,
            stills,
            shape,
            &sections.confirm_pools_mib,
            sections.tensorrt,
        );
    }
    reference
}

/// Measure `run` screening `frames`, print progress, and add its row.
fn screen_row(
    table: &mut Table,
    setup: &Setup,
    frames: &[PaddedFrame],
    run: &PoolRun,
    reference: Option<(&str, &Regions)>,
) -> Result<Measured, String> {
    let outcome = measure::screen(setup, frames, run);
    let against = against(&outcome, reference);
    table.row(row(run, Phase::Screen, &outcome, against.as_deref()));
    eprintln!(
        "pool {} {} ×{} batch {} pool {} MiB done",
        run.engine_label(),
        run.search_label(),
        run.sessions,
        run.shape.batch,
        run.shape.pool_mib
    );
    outcome
}

/// `outcome`'s boxes described against the named reference run's.
fn against(
    outcome: &Result<Measured, String>,
    reference: Option<(&str, &Regions)>,
) -> Option<String> {
    let (name, regions) = reference?;
    let measured = outcome.as_ref().ok()?;
    Some(format!(
        "vs {name}: {}",
        describe(&compare(&measured.regions, regions))
    ))
}

/// Section 4: CUDA, two sessions, fast search, every shape of the grid; each successful row's
/// shape and frames per second.
fn sweep(setup: &Setup, frames: &[PaddedFrame], grid: &[ScreenShape]) -> Vec<(ScreenShape, f64)> {
    println!(
        "## 4. Detector pool, CUDA: pool × batch sweep ({} frames, {SCREEN_SESSIONS} sessions, \
         fast search)\n",
        frames.len()
    );
    let mut table = Table::new(&COLUMNS);
    let mut measured = Vec::new();
    for &shape in grid {
        let run = PoolRun::cuda(shape, SCREEN_SESSIONS);
        if let Ok(m) = screen_row(&mut table, setup, frames, &run, None) {
            measured.push((shape, m.fps()));
        }
    }
    println!("{}", table.render());
    measured
}

/// Section 5: one session against two at `shape`; GPU busy % shows whether two streams
/// overlap. The two-session run's regions.
fn one_against_two(setup: &Setup, frames: &[PaddedFrame], shape: ScreenShape) -> Option<Regions> {
    println!("## 5. Detector pool, CUDA: one session against two\n");
    let mut table = Table::new(&COLUMNS);
    let one = screen_row(&mut table, setup, frames, &PoolRun::cuda(shape, 1), None).ok();
    let reference = one.as_ref().map(|m| ("one session", &m.regions));
    let two = screen_row(
        &mut table,
        setup,
        frames,
        &PoolRun::cuda(shape, SCREEN_SESSIONS),
        reference,
    );
    println!("{}", table.render());
    two.ok().map(|m| m.regions)
}

/// Section 6: the fast and the deterministic search, each run twice on the same frames; the
/// first fast run's regions.
fn search(setup: &Setup, frames: &[PaddedFrame], shape: ScreenShape) -> Option<Regions> {
    println!("## 6. Detector pool, CUDA: fast against deterministic search\n");
    let mut table = Table::new(&COLUMNS);
    let fast = PoolRun::cuda(shape, SCREEN_SESSIONS);
    let deterministic = PoolRun {
        search: SearchMode::Deterministic,
        ..fast
    };
    let first = screen_row(&mut table, setup, frames, &fast, None).ok();
    let first_regions = first.as_ref().map(|m| ("fast run 1", &m.regions));
    let _ = screen_row(&mut table, setup, frames, &fast, first_regions);
    let steady = screen_row(&mut table, setup, frames, &deterministic, first_regions).ok();
    let steady_regions = steady.as_ref().map(|m| ("deterministic run 1", &m.regions));
    let _ = screen_row(&mut table, setup, frames, &deterministic, steady_regions);
    println!("{}", table.render());
    first.map(|m| m.regions)
}

/// The CUDA reference run, two sessions at `shape`, when no earlier section made one.
fn reference_run(setup: &Setup, frames: &[PaddedFrame], shape: ScreenShape) -> Option<Regions> {
    println!("### CUDA reference run (the boxes TensorRT and the box images are compared with)\n");
    let mut table = Table::new(&COLUMNS);
    let run = PoolRun::cuda(shape, SCREEN_SESSIONS);
    let outcome = screen_row(&mut table, setup, frames, &run, None);
    println!("{}", table.render());
    outcome.ok().map(|m| m.regions)
}

/// Section 7: TensorRT FP16, then FP32, over the sweep's grid, against the CUDA reference.
fn tensorrt(
    setup: &Setup,
    frames: &[PaddedFrame],
    grid: &[ScreenShape],
    reference: Option<&Regions>,
) {
    println!(
        "## 7. Detector pool, TensorRT: FP16 and FP32 over the sweep ({} frames, {SCREEN_SESSIONS} \
         sessions; engines in {})\n",
        frames.len(),
        setup.cache_dir.display()
    );
    let mut table = Table::new(&COLUMNS);
    let reference = reference.map(|regions| ("CUDA reference", regions));
    for fp16 in [true, false] {
        for &shape in grid {
            let run = PoolRun::tensorrt(shape, SCREEN_SESSIONS, fp16);
            let _ = screen_row(&mut table, setup, frames, &run, reference);
        }
    }
    println!("{}", table.render());
}

/// Section 8: the server detector confirming stills through the pool at each of `pools_mib`,
/// CUDA and, with `tensorrt`, both TensorRT precisions against the first CUDA run.
fn confirmation(
    setup: &Setup,
    stills: &[PaddedFrame],
    shape: ScreenShape,
    pools_mib: &[usize],
    tensorrt: bool,
) {
    println!(
        "## 8. Detector pool: confirmation, batch 1 ({} stills, one per session untimed first)\n",
        stills.len()
    );
    let mut table = Table::new(&COLUMNS);
    let mut engines = vec![PoolRun::cuda(shape, SCREEN_SESSIONS)];
    if tensorrt {
        engines.push(PoolRun::tensorrt(shape, SCREEN_SESSIONS, true));
        engines.push(PoolRun::tensorrt(shape, SCREEN_SESSIONS, false));
    }
    let runs = engines.into_iter().flat_map(|engine| {
        pools_mib.iter().map(move |&confirm_pool_mib| PoolRun {
            confirm_pool_mib,
            ..engine
        })
    });
    let mut cuda: Option<Regions> = None;
    for run in runs {
        let outcome = measure::confirm(setup, stills, &run);
        let reference = cuda.as_ref().map(|regions| ("CUDA", regions));
        let against = against(&outcome, reference);
        table.row(row(&run, Phase::Confirm, &outcome, against.as_deref()));
        eprintln!(
            "pool confirm {} pool {} MiB done",
            run.engine_label(),
            run.confirm_pool_mib
        );
        if run.engine == DetectorEngine::Cuda
            && cuda.is_none()
            && let Ok(measured) = outcome
        {
            cuda = Some(measured.regions);
        }
    }
    println!("{}", table.render());
}
