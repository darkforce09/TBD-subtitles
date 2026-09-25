//! `stack-spike fetch`: download what the manifest pins and is not yet on disk.

use std::io::Write;

use anyhow::Context;
use clap::Args;
use inference::model_store::{self, CUDA_ARCHIVES, manifest};

#[derive(Args)]
pub(crate) struct FetchArgs {
    /// Fetch only these model ids or runtime archive ids (default: everything).
    #[arg(long = "only")]
    only: Vec<String>,
    /// List what would be fetched, with sizes, and download nothing.
    #[arg(long)]
    dry_run: bool,
}

pub(crate) fn run(args: &FetchArgs) -> anyhow::Result<()> {
    let wanted = |id: &str| args.only.is_empty() || args.only.iter().any(|o| o == id);
    let models = model_store::models_dir()?;
    let runtime = model_store::runtime_dir()?;
    let mut total = 0u64;
    for id in manifest::model_ids().into_iter().filter(|id| wanted(id)) {
        let size: u64 = manifest::files_of(id).map(|f| f.size).sum();
        let state = if model_store::is_complete(&models, id) {
            "present"
        } else {
            "missing"
        };
        println!("model   {id:<28} {:>8.1} MiB  {state}", mib(size));
        if args.dry_run || state == "present" {
            continue;
        }
        total += size;
        let mut last = 0u64;
        model_store::fetch_model(&models, id, &mut |file, held, size| {
            report(&mut last, file.file, held, size);
        })
        .with_context(|| format!("fetching model {id}"))?;
        println!();
    }
    for archive in CUDA_ARCHIVES.iter().filter(|a| wanted(a.id)) {
        println!("runtime {:<28} {:>8.1} MiB", archive.id, mib(archive.size));
        if args.dry_run {
            continue;
        }
        total += archive.size;
        let mut last = 0u64;
        model_store::install_archive(archive, &runtime, &mut |held, size| {
            report(&mut last, archive.id, held, size);
        })
        .with_context(|| format!("installing runtime archive {}", archive.id))?;
        println!();
    }
    println!(
        "models in {}; runtime in {}",
        models.display(),
        runtime.display()
    );
    if !args.dry_run {
        println!("checked {:.1} MiB against their pinned SHA-256", mib(total));
    }
    Ok(())
}

/// Print a progress line every 5 % of a file.
fn report(last: &mut u64, name: &str, held: u64, size: u64) {
    let step = (size / 20).max(1);
    if held == size || held / step != *last / step {
        print!("\r  {name}: {:>6.1} / {:.1} MiB", mib(held), mib(size));
        let _ = std::io::stdout().flush();
    }
    *last = held;
}

fn mib(bytes: u64) -> f64 {
    bytes as f64 / 1_048_576.0
}
