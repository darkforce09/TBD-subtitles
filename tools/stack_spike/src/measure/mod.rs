//! Measuring one item: a worker process of its own, its wall time, peak VRAM and peak RAM.
//!
//! **Role:** the parent side of a measurement. It checks the GPU budget is free for GPU items,
//! starts `stack-spike worker <item>` with the CUDA runtime on its library path, samples the
//! worker's VRAM through NVML while forwarding its progress lines, and joins the worker's own
//! report into one result file per item.
//!
//! **Position:** called by the `run` command in `main.rs`; spawns this same binary through
//! `child_process`; `worker.rs` is the child side.
//!
//! **Signals and state:** writes `results/<item>.json` and `results/<item>.log` in the work
//! folder; reads `results/<item>.worker.json` the worker wrote.
//!
//! **Invariants:** an item that could not run is recorded as not run with the reason, never with
//! numbers; a GPU item never runs with less than the VRAM budget free.

pub(crate) mod worker;

use std::io::{BufRead, BufReader};
use std::time::{Duration, Instant};

use anyhow::Context as _;
use child_process::Run;
use inference::cuda_runtime::CudaRuntime;
use inference::model_store;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use pipeline::measure::gpu_monitor;
pub(crate) use pipeline::measure::gpu_monitor::VramPeaks;

use crate::context::Context;
use crate::items::Item;

/// The VRAM a GPU stage may use with the desktop running.
pub(crate) const VRAM_BUDGET_MIB: u64 = 5_632;
/// The longest any one item may run.
const WORKER_DEADLINE: Duration = Duration::from_secs(3 * 3600);

/// How a measurement ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Status {
    Ok,
    Failed,
    NotRun,
}

/// One item's measurement, as kept in `results/<item>.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ItemResult {
    pub(crate) item: String,
    pub(crate) status: Status,
    pub(crate) reason: Option<String>,
    pub(crate) wall_s: f64,
    pub(crate) load_s: f64,
    pub(crate) process_s: f64,
    pub(crate) audio_s: f64,
    /// Audio seconds per processing second.
    pub(crate) x_realtime: f64,
    pub(crate) vram: Option<VramPeaks>,
    pub(crate) peak_ram_mib: f64,
    #[serde(default)]
    pub(crate) peak_child_ram_mib: f64,
    pub(crate) notes: Map<String, Value>,
}

impl ItemResult {
    fn not_run(item: Item, reason: String) -> ItemResult {
        ItemResult {
            item: item.name(),
            status: Status::NotRun,
            reason: Some(reason),
            wall_s: 0.0,
            load_s: 0.0,
            process_s: 0.0,
            audio_s: 0.0,
            x_realtime: 0.0,
            vram: None,
            peak_ram_mib: 0.0,
            peak_child_ram_mib: 0.0,
            notes: Map::new(),
        }
    }
}

/// Measure `item` in a worker process and record the result.
pub(crate) fn measure(ctx: &Context, item: Item) -> anyhow::Result<ItemResult> {
    let result = measure_inner(ctx, item)?;
    let path = ctx.path(&format!("results/{}.json", item.name()));
    std::fs::write(&path, serde_json::to_vec_pretty(&result)?)?;
    Ok(result)
}

fn measure_inner(ctx: &Context, item: Item) -> anyhow::Result<ItemResult> {
    let device = gpu_monitor::device_memory();
    if item.needs_gpu() {
        match &device {
            None => {
                return Ok(ItemResult::not_run(
                    item,
                    "NVML is not available: run on the host".into(),
                ));
            }
            Some(memory) if memory.free_mib < VRAM_BUDGET_MIB => {
                return Ok(ItemResult::not_run(
                    item,
                    format!(
                        "only {} MiB of VRAM free, {} MiB needed: close other GPU programs",
                        memory.free_mib, VRAM_BUDGET_MIB
                    ),
                ));
            }
            Some(_) => {}
        }
    }
    let exe = std::env::current_exe()?;
    let program = match item.worker_binary() {
        Some(binary) => exe.with_file_name(binary),
        None => exe.clone(),
    };
    let mut run = Run::new(program)
        .arg("worker")
        .arg(item.name())
        .arg("--video")
        .arg(&ctx.video)
        .arg("--work")
        .arg(&ctx.work)
        .timeout(WORKER_DEADLINE);
    if item.needs_gpu() {
        let exe_dir = exe.parent().map(|d| d.to_path_buf());
        let runtime = CudaRuntime::locate(exe_dir.as_deref(), &model_store::runtime_dir()?)?;
        for (key, value) in runtime.worker_env() {
            run = run.env(key, value);
        }
    }
    let report_path = ctx.path(&format!("results/{}.worker.json", item.name()));
    let _ = std::fs::remove_file(&report_path);
    let started = Instant::now();
    let mut worker = run.spawn()?;
    let monitor = device
        .as_ref()
        .map(|memory| gpu_monitor::Monitor::start(worker.pid(), memory.used_mib));
    if let Some(stdout) = worker.take_stdout() {
        let label = item.name();
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            println!("  [{label}] {line}");
        }
    }
    let finished = worker.wait();
    let wall_s = started.elapsed().as_secs_f64();
    let vram = monitor.and_then(|m| m.finish());
    let (code, stderr) = match finished {
        Ok(f) => (Some(f.code), f.stderr),
        Err(e) => (None, e.to_string()),
    };
    std::fs::write(ctx.path(&format!("results/{}.log", item.name())), &stderr)?;
    let report: Option<worker::WorkerReport> = std::fs::read_to_string(&report_path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok());
    let Some(report) = report.filter(|_| code == Some(0)) else {
        let tail: String = stderr
            .lines()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        let mut result = ItemResult::not_run(item, format!("worker exit {code:?}: {tail}"));
        result.status = Status::Failed;
        result.wall_s = wall_s;
        return Ok(result);
    };
    Ok(ItemResult {
        item: item.name(),
        status: Status::Ok,
        reason: None,
        wall_s,
        load_s: report.load_s,
        process_s: report.process_s,
        audio_s: report.audio_s,
        x_realtime: if report.process_s > 0.0 {
            report.audio_s / report.process_s
        } else {
            0.0
        },
        vram,
        peak_ram_mib: report.peak_ram_mib,
        peak_child_ram_mib: report.peak_child_ram_mib,
        notes: report.notes,
    })
}

/// Read every recorded result, in item order.
pub(crate) fn read_results(ctx: &Context) -> anyhow::Result<Vec<ItemResult>> {
    let mut results = Vec::new();
    for item in Item::ALL {
        let path = ctx.path(&format!("results/{}.json", item.name()));
        if let Ok(text) = std::fs::read_to_string(&path) {
            results.push(
                serde_json::from_str(&text)
                    .with_context(|| format!("reading {}", path.display()))?,
            );
        }
    }
    Ok(results)
}
