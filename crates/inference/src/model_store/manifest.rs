//! Every model file and runtime archive the app downloads, pinned by URL, size and SHA-256.
//!
//! **Role:** the one list of downloads. A file whose size or hash differs from its entry is never
//! used. Model hashes are the Hugging Face LFS object ids (SHA-256 of the file); archive hashes
//! come from NVIDIA's `redistrib_*.json` manifests (CUDA 13.4.2, cuDNN 9.26.0) and the GitHub release
//! digest of ONNX Runtime 1.28.2.
//!
//! **Position:** read by `download.rs`, `archive.rs` and the callers that name a model by id.
//!
//! **Signals and state:** constants only.
//!
//! **Invariants:** every URL is https; every hash is 64 lowercase hex digits; the files of one
//! model share its folder, because ONNX external data is found beside its graph by name.

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

/// One `.tar.xz` or `.tgz` archive unpacked into `runtime/<unpack_to>/` with its top folder stripped.
#[derive(Debug, Clone, Copy)]
pub struct PinnedArchive {
    pub id: &'static str,
    pub unpack_to: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
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

/// The CUDA toolkit folder name under `runtime/`.
pub const CUDA_FOLDER: &str = "cuda-13.4";
/// The cuDNN folder name under `runtime/`.
pub const CUDNN_FOLDER: &str = "cudnn-9.26";
/// A CUDA 13.3 compiler, used only to build mistral.rs, whose build accepts toolkits up to 13.3;
/// the libraries it links against at run time are the 13.4 ones.
pub const CUDA_BUILD_FOLDER: &str = "cuda-13.3-build";

macro_rules! nv {
    ($id:literal, $to:expr, $path:literal, $size:literal, $sha:literal) => {
        PinnedArchive {
            id: $id,
            unpack_to: $to,
            url: concat!("https://developer.download.nvidia.com/compute/", $path),
            size: $size,
            sha256: $sha,
        }
    };
}

/// The CUDA 13 runtime libraries ONNX Runtime, ggml and candle load, and the compiler pieces the
/// ggml and candle kernels are built with.
pub const CUDA_ARCHIVES: &[PinnedArchive] = &[
    nv!(
        "cuda_cudart",
        CUDA_FOLDER,
        "cuda/redist/cuda_cudart/linux-x86_64/cuda_cudart-linux-x86_64-13.4.92-archive.tar.xz",
        1_704_640,
        "0ac5dbc538d04e9983bc493b410cce4b459e1ee9f5f6654b6464ef7b3e14a8b5"
    ),
    nv!(
        "libcublas",
        CUDA_FOLDER,
        "cuda/redist/libcublas/linux-x86_64/libcublas-linux-x86_64-13.8.0.4-archive.tar.xz",
        877_123_136,
        "bd6ffcce561672da6abb928214a6694f392741e7d1e2d7dfde0da4bd651706b6"
    ),
    nv!(
        "libcufft",
        CUDA_FOLDER,
        "cuda/redist/libcufft/linux-x86_64/libcufft-linux-x86_64-12.4.0.43-archive.tar.xz",
        249_327_988,
        "f7f8ac2cc67714989a3c8140cda39b1b7ed7d462a491e1ad3f53adb777f5070f"
    ),
    nv!(
        "libcurand",
        CUDA_FOLDER,
        "cuda/redist/libcurand/linux-x86_64/libcurand-linux-x86_64-10.4.4.72-archive.tar.xz",
        86_992_868,
        "6f725f9187dc5f675308b830f786c163f5b1b499d86986f94fb60aa701c13c0b"
    ),
    nv!(
        "cuda_nvrtc",
        CUDA_FOLDER,
        "cuda/redist/cuda_nvrtc/linux-x86_64/cuda_nvrtc-linux-x86_64-13.4.92-archive.tar.xz",
        72_311_456,
        "defc747cfa5953b86f4c67414fc349d21149e9d513c096cb89c64b8ec4c0c932"
    ),
    nv!(
        "libnvjitlink",
        CUDA_FOLDER,
        "cuda/redist/libnvjitlink/linux-x86_64/libnvjitlink-linux-x86_64-13.4.92-archive.tar.xz",
        58_209_588,
        "e24557473e5e2e1dc591209558523deaca48b32123fe2b822f994b8e691c8ae3"
    ),
    nv!(
        "cuda_nvcc",
        CUDA_FOLDER,
        "cuda/redist/cuda_nvcc/linux-x86_64/cuda_nvcc-linux-x86_64-13.4.92-archive.tar.xz",
        33_304_524,
        "60998f40cc9df5826b2a846e2dcc328133fbc5ef539398ce0a21cf5dcac7193d"
    ),
    nv!(
        "cuda_crt",
        CUDA_FOLDER,
        "cuda/redist/cuda_crt/linux-x86_64/cuda_crt-linux-x86_64-13.4.92-archive.tar.xz",
        101_980,
        "c969e61ded12dbf0cf20ace72867f1622ec0e8ea1e1d305e27761012025f7aaa"
    ),
    nv!(
        "libnvvm",
        CUDA_FOLDER,
        "cuda/redist/libnvvm/linux-x86_64/libnvvm-linux-x86_64-13.4.92-archive.tar.xz",
        51_321_404,
        "0e619cf3d5b27f7c812665599ca9761fc296e56129b24a85d822a5d8a1a3bd75"
    ),
    nv!(
        "cccl",
        CUDA_FOLDER,
        "cuda/redist/cccl/linux-x86_64/cccl-linux-x86_64-13.3.4.3.1-archive.tar.xz",
        1_404_324,
        "6b7516074f42f80dd3feb9c11318c8e1c5ab56cdd9f18fff4c5ace2e7e0a8209"
    ),
    nv!(
        "cuda_culibos",
        CUDA_FOLDER,
        "cuda/redist/cuda_culibos/linux-x86_64/cuda_culibos-linux-x86_64-13.4.92-archive.tar.xz",
        21_440,
        "241c618719e0613ce73b781c4e84b5eccf04e5baa3bae57de07980edb68a0e55"
    ),
    nv!(
        "cuda_profiler_api",
        CUDA_FOLDER,
        "cuda/redist/cuda_profiler_api/linux-x86_64/cuda_profiler_api-linux-x86_64-13.4.92-archive.tar.xz",
        17_112,
        "9d405e9b0fd9a5c29e680d6f6750b9ca79b7df91e253d4dffea3495057a75f08"
    ),
    nv!(
        "cudnn",
        CUDNN_FOLDER,
        "cudnn/redist/cudnn/linux-x86_64/cudnn-linux-x86_64-9.26.0.51_cuda13-archive.tar.xz",
        909_961_620,
        "e62c9b4af62ea130765ea5b13244c05c9452edf1690072373d193d19766d9850"
    ),
];

/// The CUDA 13.3.1 compiler pieces unpacked into `CUDA_BUILD_FOLDER`.
pub const CUDA_BUILD_ARCHIVES: &[PinnedArchive] = &[
    nv!(
        "cuda_nvcc-13.3",
        CUDA_BUILD_FOLDER,
        "cuda/redist/cuda_nvcc/linux-x86_64/cuda_nvcc-linux-x86_64-13.3.73-archive.tar.xz",
        31_628_824,
        "2ff9f9954060794a1c5134a933ccb45bec723d866b2629dadfe4a1a313f21068"
    ),
    nv!(
        "cuda_crt-13.3",
        CUDA_BUILD_FOLDER,
        "cuda/redist/cuda_crt/linux-x86_64/cuda_crt-linux-x86_64-13.3.73-archive.tar.xz",
        99_152,
        "1251aa9d668c607a103489cd2250773701e83a313e355578044622cf36713a9d"
    ),
    nv!(
        "libnvvm-13.3",
        CUDA_BUILD_FOLDER,
        "cuda/redist/libnvvm/linux-x86_64/libnvvm-linux-x86_64-13.3.73-archive.tar.xz",
        49_508_948,
        "206b1ab4979c09b5c32f8bf907c42bc9e16cd7454cf6036f524c45a58d060f93"
    ),
    nv!(
        "cuda_cudart-13.3",
        CUDA_BUILD_FOLDER,
        "cuda/redist/cuda_cudart/linux-x86_64/cuda_cudart-linux-x86_64-13.3.29-archive.tar.xz",
        1_573_744,
        "1e59c4888267d27ba1a9bd0f3669a6439db1334a96e754cd9013c7c73e18dc9d"
    ),
    nv!(
        "cccl-13.3",
        CUDA_BUILD_FOLDER,
        "cuda/redist/cccl/linux-x86_64/cccl-linux-x86_64-13.3.3.4.1-archive.tar.xz",
        1_262_552,
        "26957cede74f9341174ecaf0372f3f886e7c46ceccb98d6dc775fe2b68d19268"
    ),
];

/// The ONNX Runtime folder name under `runtime/`.
pub const ONNX_RUNTIME_FOLDER: &str = "onnxruntime-1.28.2";

/// Microsoft's ONNX Runtime 1.28.2 built for CUDA 13, loaded by the `ort` crate at run time. The
/// hash is the release asset's digest on GitHub.
pub const ONNX_RUNTIME_ARCHIVE: PinnedArchive = PinnedArchive {
    id: "onnxruntime",
    unpack_to: ONNX_RUNTIME_FOLDER,
    url: "https://github.com/microsoft/onnxruntime/releases/download/v1.28.2/onnxruntime-linux-x64-gpu_cuda13-1.28.2.tgz",
    size: 240_868_705,
    sha256: "118ca8dbc4e4bb9b3b7fea137d796a89d957c9aa70e1dc3a5199a302cdd5bb32",
};

/// Every runtime archive: the CUDA libraries, ONNX Runtime, then the CUDA 13.3 build pieces.
pub fn runtime_archives() -> impl Iterator<Item = &'static PinnedArchive> {
    CUDA_ARCHIVES
        .iter()
        .chain(std::iter::once(&ONNX_RUNTIME_ARCHIVE))
        .chain(CUDA_BUILD_ARCHIVES)
}

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
