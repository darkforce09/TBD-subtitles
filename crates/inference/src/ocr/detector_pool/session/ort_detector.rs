//! A detector session on ONNX Runtime, with its input and output bound to fixed buffers.
//!
//! **Role:** open one PP-OCRv5 detector on the CUDA provider, or on TensorRT before CUDA, with a
//! fixed `[batch, 3, height, width]` input; bind a device input tensor and a device output
//! tensor once, and on each run copy the pinned staging tensor into the device input, run the
//! binding, copy the device output into the host result tensor and hand back the probability
//! maps from there.
//!
//! **Position:** `OrtOpener` is the pool's opener in production; the GPU path the tests cannot
//! reach, kept to the calls ONNX Runtime needs.
//!
//! **Signals and state:** one session, its `IoBinding`, the pinned staging tensor, the host result tensor
//! and the device input and output tensors, all owned by the session thread that opened them.
//!
//! **Invariants:** one optimisation level (3), one intra-op and one inter-op thread without
//! spinning, no environment providers; deterministic kernels exactly in the deterministic search
//! mode; both bound tensors live on the device and their addresses never change, so a captured
//! CUDA graph stays valid (ONNX Runtime refuses a host-memory output inside the run); a
//! TensorRT session's profile names the model's own input, and its engine and timing caches live
//! in the folder of its key.

use std::path::Path;

use ort::memory::{AllocationDevice, Allocator, AllocatorType, MemoryInfo, MemoryType};
use ort::session::builder::{GraphOptimizationLevel, SessionBuilder};
use ort::session::{IoBinding, Session};
use ort::value::{Tensor, TensorValueType, ValueType};

use super::engine_cache::{cache_key, check_identity, holds_engine, profile_shapes, sha256_hex};
use super::onnx_input::first_input_name;
use super::providers::{TensorRtPlan, deterministic_compute, providers};
use super::{CudaTuning, DetectorSession, Opened, SessionOpener, SessionSpec};
use crate::ocr::OcrError;

/// Opens real ONNX Runtime sessions.
pub struct OrtOpener;

impl SessionOpener for OrtOpener {
    fn open(&self, spec: &SessionSpec, tuning: CudaTuning) -> Result<Opened, OcrError> {
        OrtDetector::open(spec, tuning)
    }
}

/// One open detector session and its bound buffers.
struct OrtDetector {
    session: Session,
    binding: IoBinding,
    staging: Tensor<f32>,
    device_input: Tensor<f32>,
    result: Tensor<f32>,
    output_name: String,
    _allocators: [Allocator; 2],
}

fn failed(context: &str) -> impl Fn(ort::Error) -> OcrError + '_ {
    move |error| format!("{context}: {error}").into()
}

/// The TensorRT plan for `spec`, whether its engine still has to be built, and the input name
/// its profile uses.
fn tensorrt_plan(spec: &SessionSpec) -> Result<Option<(TensorRtPlan, bool, String)>, OcrError> {
    let Some(tensorrt) = &spec.tensorrt else {
        return Ok(None);
    };
    check_identity(&tensorrt.identity)?;
    let model = std::fs::read(&spec.model)
        .map_err(|error| format!("reading {}: {error}", spec.model.display()))?;
    let input = first_input_name(&model)?;
    let dims = spec.input.dims();
    let key = cache_key(&tensorrt.identity, &sha256_hex(&model), dims, tensorrt.fp16);
    let cache_dir = tensorrt.cache_root.join(key);
    std::fs::create_dir_all(&cache_dir)
        .map_err(|error| format!("creating {}: {error}", cache_dir.display()))?;
    let builds = !holds_engine(&cache_dir);
    let plan = TensorRtPlan {
        cache_dir,
        profile: profile_shapes(&input, dims),
        fp16: tensorrt.fp16,
        workspace_mib: tensorrt.workspace_mib,
    };
    Ok(Some((plan, builds, input)))
}

/// An error unless `dims`, as the model declares them (`-1` for free), admit `wanted`.
fn check_dims(what: &str, kind: &ValueType, wanted: [usize; 4]) -> Result<(), OcrError> {
    let ValueType::Tensor { shape, .. } = kind else {
        return Err(format!("the detector's {what} is not a tensor").into());
    };
    let fits = shape.len() == 4
        && shape
            .iter()
            .zip(wanted)
            .all(|(&dim, want)| dim < 0 || dim as usize == want);
    if fits {
        Ok(())
    } else {
        Err(format!("the detector's {what} {shape:?} does not admit {wanted:?}").into())
    }
}

/// A session builder with the pool's options and `spec`'s providers.
fn builder(
    spec: &SessionSpec,
    tuning: CudaTuning,
    plan: Option<&TensorRtPlan>,
) -> Result<SessionBuilder, OcrError> {
    let context = format!("configuring {}", spec.model.display());
    let step =
        |error: ort::Error<SessionBuilder>| -> OcrError { format!("{context}: {error}").into() };
    Session::builder()
        .map_err(failed(&context))?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(step)?
        .with_intra_threads(1)
        .map_err(step)?
        .with_inter_threads(1)
        .map_err(step)?
        .with_intra_op_spinning(false)
        .map_err(step)?
        .with_inter_op_spinning(false)
        .map_err(step)?
        .with_no_environment_execution_providers()
        .map_err(step)?
        .with_deterministic_compute(deterministic_compute(spec.search))
        .map_err(step)?
        .with_execution_providers(providers(spec, tuning, plan))
        .map_err(step)
}

impl OrtDetector {
    fn open(spec: &SessionSpec, tuning: CudaTuning) -> Result<Opened, OcrError> {
        check_model(&spec.model)?;
        let tensorrt = tensorrt_plan(spec)?;
        let plan = tensorrt.as_ref().map(|(plan, _, _)| plan);
        let session = builder(spec, tuning, plan)?
            .commit_from_file(&spec.model)
            .map_err(failed(&format!("opening {}", spec.model.display())))?;
        let input = session
            .inputs()
            .first()
            .ok_or("the detector has no input")?;
        let output = session
            .outputs()
            .first()
            .ok_or("the detector has no output")?;
        let dims = spec.input.dims();
        let output_dims = [dims[0], 1, dims[2], dims[3]];
        check_dims("input", input.dtype(), dims)?;
        check_dims("output", output.dtype(), output_dims)?;
        let input_name = input.name().to_owned();
        let output_name = output.name().to_owned();
        let mut notes = Vec::new();
        let mut builds_engine = false;
        if let Some((plan, builds, profiled)) = &tensorrt {
            if *profiled != input_name {
                return Err(format!(
                    "the model file names its input {profiled}, the session {input_name}"
                )
                .into());
            }
            builds_engine = *builds;
            notes.push((
                format!("{} engine cache", spec.role.label()),
                format!(
                    "{} {}",
                    if *builds { "built into" } else { "reused from" },
                    plan.cache_dir.display()
                ),
            ));
        }
        let detector = OrtDetector::bind(session, &input_name, output_name, dims, output_dims)?;
        Ok(Opened {
            session: Box::new(detector),
            builds_engine,
            notes,
        })
    }

    /// Allocate the buffers and bind the device input and the device output once.
    fn bind(
        session: Session,
        input_name: &str,
        output_name: String,
        dims: [usize; 4],
        output_dims: [usize; 4],
    ) -> Result<OrtDetector, OcrError> {
        let memory = |device, kind| {
            MemoryInfo::new(device, 0, AllocatorType::Device, kind)
                .and_then(|info| Allocator::new(&session, info))
                .map_err(failed("creating the detector's allocators"))
        };
        let device = memory(AllocationDevice::CUDA, MemoryType::Default)?;
        let pinned_in = memory(AllocationDevice::CUDA_PINNED, MemoryType::CPUInput)?;
        let tensors = failed("allocating the detector's buffers");
        let staging = Tensor::<f32>::new(&pinned_in, dims).map_err(&tensors)?;
        let device_input = Tensor::<f32>::new(&device, dims).map_err(&tensors)?;
        let device_output = Tensor::<f32>::new(&device, output_dims).map_err(&tensors)?;
        let result = Tensor::<f32>::new(&Allocator::default(), output_dims).map_err(&tensors)?;
        let mut binding = session
            .create_binding()
            .map_err(failed("binding the detector"))?;
        binding
            .bind_input(input_name, &device_input)
            .map_err(failed("binding the detector's input"))?;
        binding
            .bind_output(output_name.as_str(), device_output)
            .map_err(failed("binding the detector's output"))?;
        Ok(OrtDetector {
            session,
            binding,
            staging,
            device_input,
            result,
            output_name,
            _allocators: [device, pinned_in],
        })
    }
}

impl DetectorSession for OrtDetector {
    fn staging(&mut self) -> &mut [f32] {
        self.staging.extract_tensor_mut().1
    }

    fn run(
        &mut self,
        read: &mut dyn FnMut(&[f32]) -> Result<(), OcrError>,
    ) -> Result<(), OcrError> {
        self.staging
            .copy_into(&mut self.device_input)
            .map_err(failed("copying the batch to the GPU"))?;
        let outputs = self
            .session
            .run_binding(&self.binding)
            .map_err(failed("running the detector"))?;
        outputs
            .get(&self.output_name)
            .ok_or("the detector returned no probability map")?
            .downcast_ref::<TensorValueType<f32>>()
            .map_err(failed("reading the probability maps"))?
            .copy_into(&mut self.result)
            .map_err(failed("copying the probability maps from the GPU"))?;
        drop(outputs);
        read(self.result.extract_tensor().1)
    }
}

/// Whether `model` exists, as a clear error before ONNX Runtime reports it.
fn check_model(model: &Path) -> Result<(), OcrError> {
    if model.is_file() {
        Ok(())
    } else {
        Err(format!("the detector model {} is missing", model.display()).into())
    }
}
