//! Every model file and runtime archive the app downloads, pinned by URL, size and SHA-256.
//!
//! **Role:** the one list of downloads. A file whose size or hash differs from its entry is never
//! used. Model hashes are the Hugging Face LFS object ids (SHA-256 of the file). The GPU runtime
//! archives (CUDA, cuDNN, ONNX Runtime, TensorRT) are pinned in `gpu_runtime.rs` and re-exported
//! here, so `manifest::CUDA_FOLDER` and the other names stay where callers look for them.
//!
//! **Position:** read by `download.rs`, `archive.rs`, `archive_libraries.rs` and the callers that
//! name a model or a runtime folder.
//!
//! **Signals and state:** constants only.
//!
//! **Invariants:** every URL is https; every hash is 64 lowercase hex digits; the files of one
//! model share its folder, because ONNX external data is found beside its graph by name.

mod gpu_runtime;

pub use gpu_runtime::{
    CUDA_ARCHIVES, CUDA_BUILD_ARCHIVES, CUDA_BUILD_FOLDER, CUDA_FOLDER, CUDNN_FOLDER,
    ONNX_RUNTIME_ARCHIVE, ONNX_RUNTIME_FOLDER, TENSORRT_ARCHIVE, TENSORRT_FOLDER,
    TENSORRT_LIBRARIES, TENSORRT_VERSION, runtime_archives,
};

/// One model file: `models/<model>/<file>`.
#[derive(Debug, Clone, Copy)]
pub struct PinnedFile {
    /// The model's folder name and id.
    pub model: &'static str,
    /// The file name inside the model's folder.
    pub file: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

/// One `.tar.xz`, `.tgz` or `.tar.gz` archive unpacked into `runtime/<unpack_to>/` with its top
/// folder stripped.
#[derive(Debug, Clone, Copy)]
pub struct PinnedArchive {
    pub id: &'static str,
    pub unpack_to: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

/// Only some shared libraries of a pinned archive, unpacked flat into
/// `runtime/<archive.unpack_to>/lib/` while the archive streams past, so the archive itself is
/// never kept on disk.
#[derive(Debug, Clone, Copy)]
pub struct PinnedLibraries {
    pub archive: PinnedArchive,
    /// A member is kept when its file name starts with one of these.
    pub prefixes: &'static [&'static str],
    /// A member whose file name contains one of these is left out even when a prefix matches.
    pub skipped: &'static [&'static str],
}

impl PinnedLibraries {
    /// Whether an archive member named `file_name`, sitting directly in a folder named `lib`, is
    /// one of the libraries: a prefix matches, no skipped text appears, and it is no static
    /// archive.
    pub fn selects(&self, file_name: &str) -> bool {
        self.prefixes.iter().any(|p| file_name.starts_with(p))
            && !self.skipped.iter().any(|s| file_name.contains(s))
            && !file_name.ends_with(".a")
    }

    /// The text the install marker holds: the archive's hash and the selection, so a changed
    /// selection unpacks again.
    pub fn marker(&self) -> String {
        format!(
            "{} {} -{}",
            self.archive.sha256,
            self.prefixes.join(","),
            self.skipped.join(",")
        )
    }
}

const HF: &str = "https://huggingface.co";

macro_rules! hf {
    ($model:literal, $repo:literal, $path:literal, $file:literal, $size:literal, $sha:literal) => {
        PinnedFile {
            model: $model,
            file: $file,
            url: concat!("https://huggingface.co/", $repo, "/resolve/main/", $path),
            size: $size,
            sha256: $sha,
        }
    };
}

/// Every model file, grouped by model.
pub const MODEL_FILES: &[PinnedFile] = &[
    PinnedFile {
        model: "visual-font",
        file: "NotoSansJP.ttf",
        url: "https://raw.githubusercontent.com/google/fonts/main/ofl/notosansjp/NotoSansJP%5Bwght%5D.ttf",
        size: 9_589_900,
        sha256: "c2f3b4d463500a2ddcd3849cded1fceeb9fd6d1c32e6cbecd568453ba50fc68f",
    },
    PinnedFile {
        model: "visual-font",
        file: "OFL.txt",
        url: "https://raw.githubusercontent.com/google/fonts/main/ofl/notosansjp/OFL.txt",
        size: 4_388,
        sha256: "1c05c68c34f9708415aada51f17e1b0092d2cea709bf4a94cd38114f9e73d7d9",
    },
    PinnedFile {
        model: "latin-fonts",
        file: "NotoSans.ttf",
        url: "https://raw.githubusercontent.com/google/fonts/main/ofl/notosans/NotoSans%5Bwdth%2Cwght%5D.ttf",
        size: 2_049_096,
        sha256: "bfb7bb691513f12e734dc346c03a03f784912432d7e3fa8e56efcf906fe86b3d",
    },
    PinnedFile {
        model: "latin-fonts",
        file: "OFL.txt",
        url: "https://raw.githubusercontent.com/google/fonts/main/ofl/notosans/OFL.txt",
        size: 4_396,
        sha256: "cee9892f9f0cc8fe882c9e9537ee6a89621d86ee7ceaf70b02e2b2b1c25c061a",
    },
    hf!(
        "lama-inpaint",
        "Carve/LaMa-ONNX",
        "lama_fp32.onnx",
        "lama_fp32.onnx",
        208_044_816,
        "1faef5301d78db7dda502fe59966957ec4b79dd64e16f03ed96913c7a4eb68d6"
    ),
    PinnedFile {
        model: "pp-ocrv5",
        file: "det.onnx",
        url: "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/pp-ocrv5_server_det.onnx",
        size: 88_116_836,
        sha256: "9a910baffbefb807ff2f7bfaa72910e3e470bd17014d798386d87bb46f442839",
    },
    PinnedFile {
        model: "pp-ocrv5",
        file: "det_mobile.onnx",
        url: "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/pp-ocrv5_mobile_det.onnx",
        size: 4_826_518,
        sha256: "1eb7b4f7ab657ebd1c66d5f79bca7497f29768a2e3c15e52daecbba1a8e4a039",
    },
    PinnedFile {
        model: "pp-ocrv5",
        file: "rec.onnx",
        url: "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/pp-ocrv5_server_rec.onnx",
        size: 84_502_992,
        sha256: "4bfffad2c62eb1340250455856978fb9fb19cb4776b264ae3c2f91c35fbb40b4",
    },
    PinnedFile {
        model: "pp-ocrv5",
        file: "dict.txt",
        url: "https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/ppocrv5_dict.txt",
        size: 74_012,
        sha256: "d1979e9f794c464c0d2e0b70a7fe14dd978e9dc644c0e71f14158cdf8342af1b",
    },
    PinnedFile {
        model: "manga-ocr",
        file: "encoder.onnx",
        url: "https://huggingface.co/onnx-community/manga-ocr-base-ONNX/resolve/f9023406bb2f6b17df67bc4a327c56ecd20611f0/onnx/encoder_model.onnx",
        size: 343_377_067,
        sha256: "df35f64c2400ea860c70a2d06f2a1f99892374a78c89fcf35308076557a3863f",
    },
    PinnedFile {
        model: "manga-ocr",
        file: "decoder.onnx",
        url: "https://huggingface.co/onnx-community/manga-ocr-base-ONNX/resolve/f9023406bb2f6b17df67bc4a327c56ecd20611f0/onnx/decoder_model.onnx",
        size: 117_445_718,
        sha256: "31ca14d6dee6b3966144e128d0481d5a91f5083cbced81fe7a9571713fa50cd4",
    },
    PinnedFile {
        model: "manga-ocr",
        file: "vocab.txt",
        url: "https://huggingface.co/kha-white/manga-ocr-base/resolve/aa6573bd10b0d446cbf622e29c3e084914df9741/vocab.txt",
        size: 24_072,
        sha256: "344fbb6b8bf18c57839e924e2c9365434697e0227fac00b88bb4899b78aa594d",
    },
    hf!(
        "parakeet-tdt-0.6b-v2",
        "istupakov/parakeet-tdt-0.6b-v2-onnx",
        "encoder-model.onnx",
        "encoder-model.onnx",
        41_770_866,
        "3987bcd28175d829d12888a996a84e8f62a0e374d9ffd640662c1515adc679d3"
    ),
    hf!(
        "parakeet-tdt-0.6b-v2",
        "istupakov/parakeet-tdt-0.6b-v2-onnx",
        "encoder-model.onnx.data",
        "encoder-model.onnx.data",
        2_435_420_160,
        "4dab7362d4874d85965045b1e41b2d61dd2cc0fb25671a7f6b3dc47bf120cc41"
    ),
    hf!(
        "parakeet-tdt-0.6b-v2",
        "istupakov/parakeet-tdt-0.6b-v2-onnx",
        "decoder_joint-model.onnx",
        "decoder_joint-model.onnx",
        35_792_059,
        "cbb52a07bd70ab5b67f8439d4b3cd8704b18467b4430bcacb5adabe154b8d191"
    ),
    hf!(
        "parakeet-tdt-0.6b-v2",
        "istupakov/parakeet-tdt-0.6b-v2-onnx",
        "vocab.txt",
        "vocab.txt",
        9_384,
        "ec182b70dd42113aff6c5372c75cac58c952443eb22322f57bbd7f53977d497d"
    ),
    hf!(
        "parakeet-ctc-0.6b",
        "onnx-community/parakeet-ctc-0.6b-ONNX",
        "onnx/model.onnx",
        "model.onnx",
        887_486,
        "5c459a949508ff0da5b36e8d94feb8ed1746fea9e732879117dd7c1f78a8a86c"
    ),
    hf!(
        "parakeet-ctc-0.6b",
        "onnx-community/parakeet-ctc-0.6b-ONNX",
        "onnx/model.onnx_data",
        "model.onnx_data",
        2_435_004_420,
        "8ebe1f7360dc705dfe8163fe72bc7a4d9b823d9ef6d426f1f1f8da18fffcc1ec"
    ),
    hf!(
        "parakeet-ctc-0.6b",
        "onnx-community/parakeet-ctc-0.6b-ONNX",
        "tokenizer.json",
        "tokenizer.json",
        412_363,
        "f3f1dd45c3889ed2b5bf67180caf05f51d7d7e4948c20e5f24d8c24df9cc47aa"
    ),
    hf!(
        "parakeet-ctc-0.6b",
        "onnx-community/parakeet-ctc-0.6b-ONNX",
        "config.json",
        "config.json",
        1_033,
        "ca081bf2d5eb6c769dccf3c9594ac7cb35f0457a7d836cf63bb49cd36851e399"
    ),
    hf!(
        "mel-band-roformer-vocals",
        "silverdaw/mel-band-roformer-vocals-onnx",
        "syhft_core_folded_fp16_webgpu.onnx",
        "syhft_core_folded_fp16_webgpu.onnx",
        5_308_300,
        "dde2bfe8f85d2c12efa24ce4d45cc13e8709b8a72e277a93f130d496d948e918"
    ),
    hf!(
        "mel-band-roformer-vocals",
        "silverdaw/mel-band-roformer-vocals-onnx",
        "syhft_core_folded_fp16_webgpu.onnx.data",
        "syhft_core_folded_fp16_webgpu.onnx.data",
        741_190_540,
        "b08cfc80905e3560a4dd5d30f641299a47dd96d309ebbe9524d9d6c9d2a0356f"
    ),
    hf!(
        "mdx-net-voc-ft",
        "Politrees/UVR_resources",
        "models/MDXNet/UVR-MDX-NET-Voc_FT.onnx",
        "UVR-MDX-NET-Voc_FT.onnx",
        66_762_490,
        "534b2070fcc7df514b13ef660dc8cbb328679c2374d04354a5c42bb14ecce111"
    ),
    hf!(
        "whisper-large-v3",
        "ggerganov/whisper.cpp",
        "ggml-large-v3.bin",
        "ggml-large-v3.bin",
        3_095_033_483,
        "64d182b440b98d5203c4f9bd541544d84c605196c4f7b845dfa11fb23594d1e2"
    ),
    hf!(
        "whisper-large-v3-turbo",
        "ggerganov/whisper.cpp",
        "ggml-large-v3-turbo-q8_0.bin",
        "ggml-large-v3-turbo-q8_0.bin",
        874_188_075,
        "317eb69c11673c9de1e1f0d459b253999804ec71ac4c23c17ecf5fbe24e259a1"
    ),
    hf!(
        "qwen3-forced-aligner-0.6b",
        "cstr/qwen3-forced-aligner-0.6b-GGUF",
        "qwen3-forced-aligner-0.6b-q8_0.gguf",
        "qwen3-forced-aligner-0.6b-q8_0.gguf",
        985_594_624,
        "539df5dd0fe1721e378ac13bfac9a26b1260dafb62d892c518c1f21244762636"
    ),
    hf!(
        "ced-base",
        "mispeech/ced-base",
        "model.onnx",
        "model.onnx",
        86_870_208,
        "1cb33c4300b6c52ae099a5af72058982e673ec79862855961b8b8c10eeaba74c"
    ),
    hf!(
        "qwen3.5-4b",
        "unsloth/Qwen3.5-4B-GGUF",
        "Qwen3.5-4B-Q4_K_M.gguf",
        "Qwen3.5-4B-Q4_K_M.gguf",
        2_740_937_888,
        "00fe7986ff5f6b463e62455821146049db6f9313603938a70800d1fb69ef11a4"
    ),
];

/// The files of one model, in manifest order.
pub fn files_of(model: &str) -> impl Iterator<Item = &'static PinnedFile> + '_ {
    MODEL_FILES.iter().filter(move |f| f.model == model)
}

/// Every model id, once each, in manifest order.
pub fn model_ids() -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = Vec::new();
    for file in MODEL_FILES {
        if !ids.contains(&file.model) {
            ids.push(file.model);
        }
    }
    ids
}

/// The Hugging Face host, for tests that check every model URL points there.
pub const HUGGING_FACE: &str = HF;
