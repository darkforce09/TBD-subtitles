//! Checks against the downloaded models; they need the models folder, and the GPU ones need the
//! CUDA runtime on `LD_LIBRARY_PATH`, so they run only when asked for.

use crate::model_store;
use crate::onnx::session::{self, Device};

fn model(id: &str, file: &str) -> std::path::PathBuf {
    model_store::models_dir().unwrap().join(id).join(file)
}

#[test]
#[ignore = "needs the downloaded models and the CUDA runtime; run on the host"]
fn describe_the_separation_models() {
    for (id, file) in [
        ("mdx-net-voc-ft", "UVR-MDX-NET-Voc_FT.onnx"),
        (
            "mel-band-roformer-vocals",
            "syhft_core_folded_fp16_webgpu.onnx",
        ),
        ("ced-base", "model.onnx"),
        ("parakeet-ctc-0.6b", "model_fp16.onnx"),
    ] {
        let session = session::open(&model(id, file), Device::Cuda).unwrap();
        println!("{id}:");
        for line in session::describe(&session) {
            println!("  {line}");
        }
    }
}
