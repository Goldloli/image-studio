//! Fixed-source installer. No URL, executable, or package selection crosses the IPC boundary.
use super::paths::AppPaths;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub const INSTALL_ID: &str = "z-image-turbo-mflux-v1";
pub const WORKER_BUILD_ID: &str = "studio-mflux-worker-0.1.0";
pub const WORKER_MODULE: &str = "studio_mflux_worker";
pub const KLEIN_INSTALL_ID: &str = "flux2-klein-4b-distilled";
pub const KLEIN_WORKER_BUILD_ID: &str = "studio-mflux-worker-0.2.0";
pub const KLEIN_WORKER_MODULE: &str = "studio_mflux_worker_v2";
pub const BIREFNET_INSTALL_ID: &str = "birefnet-general-epoch-244";
pub const BIREFNET_WORKER_BUILD_ID: &str = "studio-onnx-worker-0.1.0";
pub const BIREFNET_WORKER_MODULE: &str = "studio_onnx_worker";
pub const REALESRGAN_INSTALL_ID: &str = "realesrgan-x4plus";
pub const REALESRGAN_WORKER_BUILD_ID: &str = "studio-realesrgan-worker-0.1.0";
pub const REALESRGAN_WORKER_MODULE: &str = "realesrgan-native-worker";
pub const QWEN_IMAGE_21_INSTALL_ID: &str = "qwen-image-2.1-mflux-v1";
pub const QWEN_IMAGE_21_TURBO_INSTALL_ID: &str = "qwen-image-2.1-turbo-mflux-v1";
pub const MOSS_IMAGE_SFT_INSTALL_ID: &str = "moss-image-sft-mflux-v1";
pub const WINDOWS_ZIMAGE_INSTALL_ID: &str = "z-image-turbo-diffusers-cuda-nf4-v1";
pub const WINDOWS_ZIMAGE_WORKER_BUILD_ID: &str = "studio-zimage-cuda-worker-0.1.0";
pub const WINDOWS_ZIMAGE_WORKER_MODULE: &str = "studio_zimage_cuda_worker";
pub const RECIPES_V3_WORKER_BUILD_ID: &str = "studio-mflux-worker-0.3.0";
pub const RECIPES_V3_WORKER_MODULE: &str = "studio_mflux_worker_v3";
const MODEL_MANIFEST: &str =
    include_str!("../../../../resources/local-generation/model-manifest.json");
const RUNTIME_MANIFEST: &str =
    include_str!("../../../../resources/local-generation/runtime-manifest.json");
const LOCK: &str = include_str!("../../../../resources/local-generation/requirements.lock");
const WORKER_WHEEL: &[u8] = include_bytes!(
    "../../../../resources/local-generation/studio_mflux_worker-0.1.0-py3-none-any.whl"
);
const KLEIN_MODEL_MANIFEST: &str =
    include_str!("../../../../resources/planned-models/klein/model-manifest.json");
const KLEIN_RUNTIME_MANIFEST: &str =
    include_str!("../../../../resources/planned-models/klein/runtime-manifest.json");
const KLEIN_LOCK: &str =
    include_str!("../../../../resources/planned-models/klein/requirements.lock");
const KLEIN_WORKER_WHEEL: &[u8] = include_bytes!(
    "../../../../resources/planned-models/klein/studio_mflux_worker_v2-0.2.0-py3-none-any.whl"
);
const BIREFNET_MODEL_MANIFEST: &str =
    include_str!("../../../../resources/planned-models/birefnet/model-manifest.json");
const BIREFNET_RUNTIME_MANIFEST: &str =
    include_str!("../../../../resources/planned-models/birefnet/runtime-manifest.json");
const BIREFNET_LOCK: &str =
    include_str!("../../../../resources/planned-models/birefnet/requirements.lock");
const BIREFNET_WORKER_WHEEL: &[u8] = include_bytes!(
    "../../../../resources/planned-models/birefnet/studio_onnx_worker-0.1.0-py3-none-any.whl"
);
const REALESRGAN_MODEL_MANIFEST: &str =
    include_str!("../../../../resources/planned-models/realesrgan/model-manifest.json");
const REALESRGAN_RUNTIME_MANIFEST: &str =
    include_str!("../../../../resources/planned-models/realesrgan/runtime-manifest.json");
// macOS-only Qwen/MOSS profiles: Windows installs use the frozen v2 runtime
// manifests under resources/windows-parity instead.
#[cfg(not(windows))]
const QWEN_IMAGE_21_MODEL_MANIFEST: &str =
    include_str!("../../../../resources/planned-models/qwen21/model-manifest.json");
#[cfg(not(windows))]
const QWEN_IMAGE_21_TURBO_MODEL_MANIFEST: &str =
    include_str!("../../../../resources/planned-models/qwen21-turbo/model-manifest.json");
#[cfg(not(windows))]
const MOSS_IMAGE_SFT_MODEL_MANIFEST: &str =
    include_str!("../../../../resources/planned-models/moss-image-sft/model-manifest.json");
const WINDOWS_ZIMAGE_MODEL_MANIFEST: &str = include_str!(
    "../../../../resources/windows-foundation/z-image-diffusers/product-model-manifest.json"
);
const WINDOWS_ZIMAGE_RUNTIME_MANIFEST: &str = include_str!(
    "../../../../resources/windows-foundation/python-runtime/product-runtime-manifest.json"
);
const WINDOWS_ZIMAGE_LOCK: &str =
    include_str!("../../../../resources/windows-foundation/python-runtime/requirements.lock");
const WINDOWS_ZIMAGE_WORKER_WHEEL: &[u8] = include_bytes!(
    "../../../../resources/windows-foundation/z-image-diffusers/studio_zimage_cuda_worker-0.1.0-py3-none-any.whl"
);
#[cfg(not(windows))]
const RECIPES_V3_RUNTIME_MANIFEST: &str =
    include_str!("../../../../resources/planned-models/qwen21/runtime-manifest.json");
#[cfg(not(windows))]
const RECIPES_V3_LOCK: &str =
    include_str!("../../../../resources/planned-models/qwen21/requirements.lock");
#[cfg(not(windows))]
const RECIPES_V3_WORKER_WHEEL: &[u8] = include_bytes!(
    "../../../../resources/planned-models/qwen21/studio_mflux_worker_v3-0.3.0-py3-none-any.whl"
);
const RESERVE: u64 = 4 * 1024 * 1024 * 1024;
const SEGMENTED_DOWNLOAD_THRESHOLD: u64 = 2 * 1024 * 1024 * 1024;
const DOWNLOAD_SEGMENTS: usize = 8;
const SEGMENT_ATTEMPTS: usize = 5;

// Windows model-parity install identities (design.md section 11). The installer
// id / model variant id doubles as the model manifest id and install directory.
pub const WINDOWS_KLEIN_INSTALL_ID: &str = "flux2-klein-4b-distilled-windows-v1";
pub const WINDOWS_BIREFNET_INSTALL_ID: &str = "birefnet-general-epoch-244-windows-v1";
pub const WINDOWS_REALESRGAN_INSTALL_ID: &str = "realesrgan-x4plus-windows-v1";
pub const WINDOWS_QWEN_IMAGE_21_INSTALL_ID: &str = "qwen-image-2.1-windows-v1";
pub const WINDOWS_QWEN_IMAGE_21_TURBO_INSTALL_ID: &str = "qwen-image-2.1-turbo-windows-v1";
pub const WINDOWS_MOSS_IMAGE_SFT_INSTALL_ID: &str = "moss-image-sft-windows-v1";

#[cfg(windows)]
const WINDOWS_V2_RUNTIME_MANIFEST: &str =
    include_str!("../../../../resources/windows-parity/runtime-v2/product-runtime-manifest.json");
#[cfg(windows)]
const WINDOWS_V2_REQUIREMENTS_LOCK: &str =
    include_str!("../../../../resources/windows-parity/runtime-v2/requirements.lock");
#[cfg(windows)]
const WINDOWS_KLEIN_MODEL_MANIFEST: &str = include_str!(
    "../../../../resources/windows-parity/models/flux2-klein-4b-distilled-windows-v1/model-manifest.json"
);
#[cfg(windows)]
const WINDOWS_BIREFNET_MODEL_MANIFEST: &str = include_str!(
    "../../../../resources/windows-parity/models/birefnet-general-epoch-244-windows-v1/model-manifest.json"
);
#[cfg(windows)]
const WINDOWS_REALESRGAN_MODEL_MANIFEST: &str = include_str!(
    "../../../../resources/windows-parity/models/realesrgan-x4plus-windows-v1/model-manifest.json"
);
#[cfg(windows)]
const WINDOWS_QWEN_IMAGE_21_MODEL_MANIFEST: &str = include_str!(
    "../../../../resources/windows-parity/models/qwen-image-2.1-windows-v1/model-manifest.json"
);
#[cfg(windows)]
const WINDOWS_QWEN_IMAGE_21_TURBO_MODEL_MANIFEST: &str = include_str!(
    "../../../../resources/windows-parity/models/qwen-image-2.1-turbo-windows-v1/model-manifest.json"
);
#[cfg(windows)]
const WINDOWS_MOSS_IMAGE_SFT_MODEL_MANIFEST: &str = include_str!(
    "../../../../resources/windows-parity/models/moss-image-sft-windows-v1/model-manifest.json"
);
#[cfg(windows)]
const WINDOWS_REALESRGAN_RUNTIME_MANIFEST: &str = include_str!(
    "../../../../resources/windows-parity/realesrgan-ncnn-vulkan/runtime-manifest.json"
);
#[cfg(windows)]
const WINDOWS_DIFFUSERS_WORKER_WHEEL: &[u8] = include_bytes!(
    "../../../../resources/windows-parity/workers/studio_diffusers_worker-0.1.0-py3-none-any.whl"
);
#[cfg(windows)]
const WINDOWS_ONNX_WORKER_WHEEL: &[u8] = include_bytes!(
    "../../../../resources/windows-parity/workers/studio_onnx_worker-0.1.0-py3-none-any.whl"
);
/// The shared v2 runtime serves every Python Windows model, so both worker wheels
/// must be present regardless of which model is installed first. The frozen
/// product runtime manifest does not declare them yet (`worker_wheels_pending`),
/// so the installer embeds them as fixed application resources instead.
#[cfg(windows)]
const WINDOWS_SHARED_WORKER_WHEELS: &[(&str, &[u8])] = &[
    (
        "studio_diffusers_worker-0.1.0-py3-none-any.whl",
        WINDOWS_DIFFUSERS_WORKER_WHEEL,
    ),
    (
        "studio_onnx_worker-0.1.0-py3-none-any.whl",
        WINDOWS_ONNX_WORKER_WHEEL,
    ),
];
#[cfg(windows)]
const WINDOWS_BIREFNET_LICENSES: &[(&str, &[u8])] = &[(
    "LICENSE-BiRefNet.txt",
    include_bytes!("../../../../resources/planned-models/birefnet/LICENSE-BiRefNet.txt"),
)];
#[cfg(windows)]
const WINDOWS_REALESRGAN_LICENSES: &[(&str, &[u8])] = &[
    (
        "LICENSE-RealESRGAN.txt",
        include_bytes!(
            "../../../../resources/windows-parity/realesrgan-ncnn-vulkan/LICENSE-RealESRGAN.txt"
        ),
    ),
    (
        "LICENSE-ncnn-app.txt",
        include_bytes!(
            "../../../../resources/windows-parity/realesrgan-ncnn-vulkan/LICENSE-ncnn-app.txt"
        ),
    ),
];

#[derive(Clone, Copy)]
struct InstallProfile {
    install_id: &'static str,
    model_manifest: &'static str,
    runtime_manifest: &'static str,
    requirements_lock: &'static str,
    worker_wheel: &'static [u8],
    worker_module: &'static str,
    worker_build_id: &'static str,
    engine: &'static str,
    scheduler: &'static str,
    probe_device: &'static str,
    precision: &'static str,
    ready_relative: &'static str,
    installing_relative: &'static str,
    probe_with_model: bool,
    worker_model_argument: bool,
    /// Fixed license texts that cannot be downloaded from an allowlisted URL and
    /// are archived under `<data>/licenses/<install_id>/` at install time.
    bundle_licenses: &'static [(&'static str, &'static [u8])],
    /// Staged Qwen/MOSS models build a quantized cache with the worker at install
    /// time; the prepared directory is recorded in the ready record.
    prepare_with_model: bool,
    /// Conservative space reserve for that prepared cache (design.md section 10).
    prepared_cache_reserve: u64,
    /// Fixed worker wheels embedded for the shared v2 runtime when the frozen
    /// runtime manifest does not declare them (Windows model parity).
    shared_worker_wheels: &'static [(&'static str, &'static [u8])],
}

struct SegmentDownload<'a> {
    url: &'a str,
    artifact_path: &'a str,
    path: &'a Path,
    metadata_path: &'a Path,
    sha256: &'a str,
    etag: &'a str,
    start: u64,
    end: u64,
    total: u64,
}

const Z_PROFILE: InstallProfile = InstallProfile {
    install_id: INSTALL_ID,
    model_manifest: MODEL_MANIFEST,
    runtime_manifest: RUNTIME_MANIFEST,
    requirements_lock: LOCK,
    worker_wheel: WORKER_WHEEL,
    worker_module: WORKER_MODULE,
    worker_build_id: WORKER_BUILD_ID,
    engine: "MFLUX 0.24.0 / MLX",
    scheduler: "linear",
    probe_device: "gpu",
    precision: "native;compute_precision=None;float32=False",
    ready_relative: "installation-ready.json",
    installing_relative: "installation-installing.json",
    probe_with_model: false,
    worker_model_argument: false,
    bundle_licenses: &[],
    prepare_with_model: false,
    prepared_cache_reserve: 0,
    shared_worker_wheels: &[],
};

const KLEIN_PROFILE: InstallProfile = InstallProfile {
    install_id: KLEIN_INSTALL_ID,
    model_manifest: KLEIN_MODEL_MANIFEST,
    runtime_manifest: KLEIN_RUNTIME_MANIFEST,
    requirements_lock: KLEIN_LOCK,
    worker_wheel: KLEIN_WORKER_WHEEL,
    worker_module: KLEIN_WORKER_MODULE,
    worker_build_id: KLEIN_WORKER_BUILD_ID,
    engine: "MFLUX 0.24.0 / MLX",
    scheduler: "flow_match_euler_discrete",
    probe_device: "gpu",
    precision: "native;compute_precision=None;float32=False",
    ready_relative: "installations/models/flux2-klein-4b-distilled.json",
    installing_relative: "installations/models/flux2-klein-4b-distilled.installing.json",
    probe_with_model: false,
    worker_model_argument: false,
    bundle_licenses: &[],
    prepare_with_model: false,
    prepared_cache_reserve: 0,
    shared_worker_wheels: &[],
};

const BIREFNET_PROFILE: InstallProfile = InstallProfile {
    install_id: BIREFNET_INSTALL_ID,
    model_manifest: BIREFNET_MODEL_MANIFEST,
    runtime_manifest: BIREFNET_RUNTIME_MANIFEST,
    requirements_lock: BIREFNET_LOCK,
    worker_wheel: BIREFNET_WORKER_WHEEL,
    worker_module: BIREFNET_WORKER_MODULE,
    worker_build_id: BIREFNET_WORKER_BUILD_ID,
    engine: "ONNX Runtime 1.31.1 / CPU",
    scheduler: "birefnet-1024-rgb-v1",
    probe_device: "cpu",
    precision: "float32",
    ready_relative: "installations/models/birefnet-general-epoch-244.json",
    installing_relative: "installations/models/birefnet-general-epoch-244.installing.json",
    probe_with_model: true,
    worker_model_argument: false,
    bundle_licenses: &[],
    prepare_with_model: false,
    prepared_cache_reserve: 0,
    shared_worker_wheels: &[],
};

const REALESRGAN_PROFILE: InstallProfile = InstallProfile {
    install_id: REALESRGAN_INSTALL_ID,
    model_manifest: REALESRGAN_MODEL_MANIFEST,
    runtime_manifest: REALESRGAN_RUNTIME_MANIFEST,
    requirements_lock: "",
    worker_wheel: &[],
    worker_module: REALESRGAN_WORKER_MODULE,
    worker_build_id: REALESRGAN_WORKER_BUILD_ID,
    engine: "Real-ESRGAN ncnn Vulkan v0.2.0",
    scheduler: "realesrgan-x4plus-t128-v1",
    probe_device: "gpu",
    precision: "vulkan-fp16;tile=128",
    ready_relative: "installations/models/realesrgan-x4plus.json",
    installing_relative: "installations/models/realesrgan-x4plus.installing.json",
    probe_with_model: true,
    worker_model_argument: false,
    bundle_licenses: &[],
    prepare_with_model: false,
    prepared_cache_reserve: 0,
    shared_worker_wheels: &[],
};

#[cfg(not(windows))]
const QWEN_IMAGE_21_PROFILE: InstallProfile = InstallProfile {
    install_id: QWEN_IMAGE_21_INSTALL_ID,
    model_manifest: QWEN_IMAGE_21_MODEL_MANIFEST,
    runtime_manifest: RECIPES_V3_RUNTIME_MANIFEST,
    requirements_lock: RECIPES_V3_LOCK,
    worker_wheel: RECIPES_V3_WORKER_WHEEL,
    worker_module: RECIPES_V3_WORKER_MODULE,
    worker_build_id: RECIPES_V3_WORKER_BUILD_ID,
    engine: "MFLUX 0.24.0 / MLX",
    scheduler: "linear",
    probe_device: "gpu",
    precision: "q8 transformer;bfloat16 text encoder;float32 vae",
    ready_relative: "installations/models/qwen-image-2.1-mflux-v1.json",
    installing_relative: "installations/models/qwen-image-2.1-mflux-v1.installing.json",
    probe_with_model: false,
    worker_model_argument: true,
    bundle_licenses: &[],
    prepare_with_model: false,
    prepared_cache_reserve: 0,
    shared_worker_wheels: &[],
};

#[cfg(not(windows))]
const QWEN_IMAGE_21_TURBO_PROFILE: InstallProfile = InstallProfile {
    install_id: QWEN_IMAGE_21_TURBO_INSTALL_ID,
    model_manifest: QWEN_IMAGE_21_TURBO_MODEL_MANIFEST,
    runtime_manifest: RECIPES_V3_RUNTIME_MANIFEST,
    requirements_lock: RECIPES_V3_LOCK,
    worker_wheel: RECIPES_V3_WORKER_WHEEL,
    worker_module: RECIPES_V3_WORKER_MODULE,
    worker_build_id: RECIPES_V3_WORKER_BUILD_ID,
    engine: "MFLUX 0.24.0 / MLX",
    scheduler: "linear",
    probe_device: "gpu",
    precision: "q8 transformer;bfloat16 text encoder;float32 vae",
    ready_relative: "installations/models/qwen-image-2.1-turbo-mflux-v1.json",
    installing_relative: "installations/models/qwen-image-2.1-turbo-mflux-v1.installing.json",
    probe_with_model: false,
    worker_model_argument: true,
    bundle_licenses: &[],
    prepare_with_model: false,
    prepared_cache_reserve: 0,
    shared_worker_wheels: &[],
};

#[cfg(not(windows))]
const MOSS_IMAGE_SFT_PROFILE: InstallProfile = InstallProfile {
    install_id: MOSS_IMAGE_SFT_INSTALL_ID,
    model_manifest: MOSS_IMAGE_SFT_MODEL_MANIFEST,
    runtime_manifest: RECIPES_V3_RUNTIME_MANIFEST,
    requirements_lock: RECIPES_V3_LOCK,
    worker_wheel: RECIPES_V3_WORKER_WHEEL,
    worker_module: RECIPES_V3_WORKER_MODULE,
    worker_build_id: RECIPES_V3_WORKER_BUILD_ID,
    engine: "MFLUX 0.24.0 / MLX",
    scheduler: "moss_image",
    probe_device: "gpu",
    precision: "q8 transformer/text encoder;bfloat16 vae",
    ready_relative: "installations/models/moss-image-sft-mflux-v1.json",
    installing_relative: "installations/models/moss-image-sft-mflux-v1.installing.json",
    probe_with_model: false,
    worker_model_argument: true,
    bundle_licenses: &[],
    prepare_with_model: false,
    prepared_cache_reserve: 0,
    shared_worker_wheels: &[],
};

const WINDOWS_ZIMAGE_PROFILE: InstallProfile = InstallProfile {
    install_id: WINDOWS_ZIMAGE_INSTALL_ID,
    model_manifest: WINDOWS_ZIMAGE_MODEL_MANIFEST,
    runtime_manifest: WINDOWS_ZIMAGE_RUNTIME_MANIFEST,
    requirements_lock: WINDOWS_ZIMAGE_LOCK,
    worker_wheel: WINDOWS_ZIMAGE_WORKER_WHEEL,
    worker_module: WINDOWS_ZIMAGE_WORKER_MODULE,
    worker_build_id: WINDOWS_ZIMAGE_WORKER_BUILD_ID,
    engine: studio_core::generation::Z_IMAGE_WINDOWS_ENGINE,
    scheduler: studio_core::generation::Z_IMAGE_WINDOWS_SCHEDULER,
    probe_device: "gpu",
    precision: studio_core::generation::Z_IMAGE_WINDOWS_PRECISION,
    ready_relative: "installations/models/z-image-turbo-diffusers-cuda-nf4-v1.json",
    installing_relative: "installations/models/z-image-turbo-diffusers-cuda-nf4-v1.installing.json",
    probe_with_model: true,
    worker_model_argument: false,
    bundle_licenses: &[],
    prepare_with_model: false,
    prepared_cache_reserve: 0,
    shared_worker_wheels: &[],
};

#[cfg(windows)]
const WINDOWS_KLEIN_PROFILE: InstallProfile = InstallProfile {
    install_id: WINDOWS_KLEIN_INSTALL_ID,
    model_manifest: WINDOWS_KLEIN_MODEL_MANIFEST,
    runtime_manifest: WINDOWS_V2_RUNTIME_MANIFEST,
    requirements_lock: WINDOWS_V2_REQUIREMENTS_LOCK,
    worker_wheel: &[],
    worker_module: "studio_diffusers_worker",
    worker_build_id: studio_core::generation::WINDOWS_DIFFUSERS_WORKER,
    engine: studio_core::generation::WINDOWS_DIFFUSERS_ENGINE,
    scheduler: "flow_match_euler_discrete",
    probe_device: "gpu",
    precision: "nf4;load_strategy=nf4-component-offload-v1",
    ready_relative: "installations/models/flux2-klein-4b-distilled-windows-v1.json",
    installing_relative: "installations/models/flux2-klein-4b-distilled-windows-v1.installing.json",
    probe_with_model: true,
    worker_model_argument: true,
    bundle_licenses: &[],
    prepare_with_model: false,
    prepared_cache_reserve: 0,
    shared_worker_wheels: WINDOWS_SHARED_WORKER_WHEELS,
};

#[cfg(windows)]
const WINDOWS_BIREFNET_PROFILE: InstallProfile = InstallProfile {
    install_id: WINDOWS_BIREFNET_INSTALL_ID,
    model_manifest: WINDOWS_BIREFNET_MODEL_MANIFEST,
    runtime_manifest: WINDOWS_V2_RUNTIME_MANIFEST,
    requirements_lock: WINDOWS_V2_REQUIREMENTS_LOCK,
    worker_wheel: &[],
    worker_module: "studio_onnx_worker",
    worker_build_id: studio_core::generation::WINDOWS_ONNX_WORKER,
    engine: studio_core::generation::WINDOWS_ONNX_ENGINE,
    scheduler: "birefnet-1024-rgb-v1",
    probe_device: "windows-cpu",
    precision: "float32",
    ready_relative: "installations/models/birefnet-general-epoch-244-windows-v1.json",
    installing_relative: "installations/models/birefnet-general-epoch-244-windows-v1.installing.json",
    probe_with_model: true,
    worker_model_argument: true,
    bundle_licenses: WINDOWS_BIREFNET_LICENSES,
    prepare_with_model: false,
    prepared_cache_reserve: 0,
    shared_worker_wheels: WINDOWS_SHARED_WORKER_WHEELS,
};

#[cfg(windows)]
const WINDOWS_REALESRGAN_PROFILE: InstallProfile = InstallProfile {
    install_id: WINDOWS_REALESRGAN_INSTALL_ID,
    model_manifest: WINDOWS_REALESRGAN_MODEL_MANIFEST,
    runtime_manifest: WINDOWS_REALESRGAN_RUNTIME_MANIFEST,
    requirements_lock: "",
    worker_wheel: &[],
    worker_module: REALESRGAN_WORKER_MODULE,
    worker_build_id: studio_core::generation::WINDOWS_REALESRGAN_WORKER,
    engine: studio_core::generation::WINDOWS_NCNN_ENGINE,
    scheduler: "realesrgan-x4plus-windows-t128-v1",
    probe_device: "gpu",
    precision: "vulkan-fp16;tile=128",
    ready_relative: "installations/models/realesrgan-x4plus-windows-v1.json",
    installing_relative: "installations/models/realesrgan-x4plus-windows-v1.installing.json",
    probe_with_model: true,
    worker_model_argument: true,
    bundle_licenses: WINDOWS_REALESRGAN_LICENSES,
    prepare_with_model: false,
    prepared_cache_reserve: 0,
    shared_worker_wheels: &[],
};

#[cfg(windows)]
const WINDOWS_QWEN_IMAGE_21_PROFILE: InstallProfile = InstallProfile {
    install_id: WINDOWS_QWEN_IMAGE_21_INSTALL_ID,
    model_manifest: WINDOWS_QWEN_IMAGE_21_MODEL_MANIFEST,
    runtime_manifest: WINDOWS_V2_RUNTIME_MANIFEST,
    requirements_lock: WINDOWS_V2_REQUIREMENTS_LOCK,
    worker_wheel: &[],
    worker_module: "studio_diffusers_worker",
    worker_build_id: studio_core::generation::WINDOWS_DIFFUSERS_WORKER,
    engine: studio_core::generation::WINDOWS_DIFFUSERS_ENGINE,
    scheduler: "linear",
    probe_device: "gpu",
    precision: "nf4;load_strategy=staged-nf4-v1",
    ready_relative: "installations/models/qwen-image-2.1-windows-v1.json",
    installing_relative: "installations/models/qwen-image-2.1-windows-v1.installing.json",
    probe_with_model: true,
    worker_model_argument: true,
    bundle_licenses: &[],
    prepare_with_model: true,
    prepared_cache_reserve: 10_050_000_000,
    shared_worker_wheels: WINDOWS_SHARED_WORKER_WHEELS,
};

#[cfg(windows)]
const WINDOWS_QWEN_IMAGE_21_TURBO_PROFILE: InstallProfile = InstallProfile {
    install_id: WINDOWS_QWEN_IMAGE_21_TURBO_INSTALL_ID,
    model_manifest: WINDOWS_QWEN_IMAGE_21_TURBO_MODEL_MANIFEST,
    runtime_manifest: WINDOWS_V2_RUNTIME_MANIFEST,
    requirements_lock: WINDOWS_V2_REQUIREMENTS_LOCK,
    worker_wheel: &[],
    worker_module: "studio_diffusers_worker",
    worker_build_id: studio_core::generation::WINDOWS_DIFFUSERS_WORKER,
    engine: studio_core::generation::WINDOWS_DIFFUSERS_ENGINE,
    scheduler: "linear",
    probe_device: "gpu",
    precision: "nf4;load_strategy=staged-nf4-v1",
    ready_relative: "installations/models/qwen-image-2.1-turbo-windows-v1.json",
    installing_relative: "installations/models/qwen-image-2.1-turbo-windows-v1.installing.json",
    probe_with_model: true,
    worker_model_argument: true,
    bundle_licenses: &[],
    prepare_with_model: true,
    prepared_cache_reserve: 10_050_000_000,
    shared_worker_wheels: WINDOWS_SHARED_WORKER_WHEELS,
};

#[cfg(windows)]
const WINDOWS_MOSS_IMAGE_SFT_PROFILE: InstallProfile = InstallProfile {
    install_id: WINDOWS_MOSS_IMAGE_SFT_INSTALL_ID,
    model_manifest: WINDOWS_MOSS_IMAGE_SFT_MODEL_MANIFEST,
    runtime_manifest: WINDOWS_V2_RUNTIME_MANIFEST,
    requirements_lock: WINDOWS_V2_REQUIREMENTS_LOCK,
    worker_wheel: &[],
    worker_module: "studio_diffusers_worker",
    worker_build_id: studio_core::generation::WINDOWS_DIFFUSERS_WORKER,
    engine: studio_core::generation::WINDOWS_DIFFUSERS_ENGINE,
    scheduler: "moss_image",
    probe_device: "gpu",
    precision: "nf4;vae=float32",
    ready_relative: "installations/models/moss-image-sft-windows-v1.json",
    installing_relative: "installations/models/moss-image-sft-windows-v1.installing.json",
    probe_with_model: true,
    worker_model_argument: true,
    bundle_licenses: &[],
    prepare_with_model: true,
    prepared_cache_reserve: 8_470_000_000,
    shared_worker_wheels: WINDOWS_SHARED_WORKER_WHEELS,
};

#[derive(Clone, Debug, Deserialize)]
struct Artifact {
    path: String,
    url: String,
    bytes: u64,
    sha256: String,
}
#[derive(Clone, Debug, Deserialize)]
struct ModelManifest {
    id: String,
    revision: String,
    component_bytes: u64,
    #[serde(default)]
    files: Vec<Artifact>,
    #[serde(default)]
    archives: Vec<ArchiveArtifact>,
}
#[derive(Clone, Debug, Deserialize)]
struct RuntimeManifest {
    #[serde(default)]
    kind: String,
    id: String,
    #[serde(default)]
    python: Option<String>,
    #[serde(default)]
    interpreter: Option<Artifact>,
    #[serde(default)]
    wheels: Vec<Artifact>,
    #[serde(default)]
    packages: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    platform: String,
    #[serde(default)]
    archives: Vec<ArchiveArtifact>,
    #[serde(default)]
    executable: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
struct ArchiveArtifact {
    path: String,
    url: String,
    bytes: u64,
    sha256: String,
    members: Vec<ArchiveMember>,
}
#[derive(Clone, Debug, Deserialize)]
struct ArchiveMember {
    archive_path: String,
    path: String,
    bytes: u64,
    sha256: String,
    #[serde(default)]
    executable: bool,
}
#[derive(Clone, Debug)]
struct InstalledArtifact {
    path: String,
    bytes: u64,
    sha256: String,
    executable: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstallationSnapshot {
    pub install_id: String,
    pub phase: String,
    pub received_bytes: u64,
    pub total_bytes: u64,
    pub model_bytes: u64,
    pub runtime_bytes: u64,
    pub required_bytes: u64,
    pub error: Option<String>,
    pub runtime_ready: bool,
    pub model_ready: bool,
    pub device_verified: bool,
}
#[derive(Clone, Debug)]
pub struct ReadyPaths {
    pub worker_program: PathBuf,
    pub worker_args: Vec<String>,
    pub model_path: PathBuf,
    pub worker_module: String,
    pub worker_build_id: String,
    pub runtime_lock_id: String,
    pub model_snapshot_id: String,
}
#[derive(Clone)]
pub struct Installation {
    inner: Arc<Inner>,
}
struct Inner {
    paths: AppPaths,
    profile: InstallProfile,
    model: ModelManifest,
    runtime: RuntimeManifest,
    state: Mutex<InstallationSnapshot>,
    active: AtomicBool,
    cancelled: AtomicBool,
    aborted: AtomicBool,
}

#[derive(Clone)]
pub struct Installations {
    #[cfg(not(windows))]
    legacy: Installation,
    #[cfg(not(windows))]
    klein: Installation,
    #[cfg(not(windows))]
    birefnet: Installation,
    #[cfg(not(windows))]
    realesrgan: Installation,
    #[cfg(not(windows))]
    qwen_image_21: Installation,
    #[cfg(not(windows))]
    qwen_image_21_turbo: Installation,
    #[cfg(not(windows))]
    moss_image_sft: Installation,
    #[cfg(windows)]
    windows_zimage: Installation,
    #[cfg(windows)]
    windows_klein: Installation,
    #[cfg(windows)]
    windows_birefnet: Installation,
    #[cfg(windows)]
    windows_realesrgan: Installation,
    #[cfg(windows)]
    windows_qwen_image_21: Installation,
    #[cfg(windows)]
    windows_qwen_image_21_turbo: Installation,
    #[cfg(windows)]
    windows_moss_image_sft: Installation,
}

impl Installations {
    pub fn new(paths: AppPaths) -> Result<Self, String> {
        #[cfg(not(windows))]
        {
            Ok(Self {
                legacy: Installation::new(paths.clone())?,
                klein: Installation::new_klein(paths.clone())?,
                birefnet: Installation::new_birefnet(paths.clone())?,
                realesrgan: Installation::new_realesrgan(paths.clone())?,
                qwen_image_21: Installation::new_for(paths.clone(), QWEN_IMAGE_21_PROFILE)?,
                qwen_image_21_turbo: Installation::new_for(
                    paths.clone(),
                    QWEN_IMAGE_21_TURBO_PROFILE,
                )?,
                moss_image_sft: Installation::new_for(paths.clone(), MOSS_IMAGE_SFT_PROFILE)?,
            })
        }
        #[cfg(windows)]
        {
            Ok(Self {
                windows_zimage: Installation::new_for(paths.clone(), WINDOWS_ZIMAGE_PROFILE)?,
                windows_klein: Installation::new_for(paths.clone(), WINDOWS_KLEIN_PROFILE)?,
                windows_birefnet: Installation::new_for(paths.clone(), WINDOWS_BIREFNET_PROFILE)?,
                windows_realesrgan: Installation::new_for(
                    paths.clone(),
                    WINDOWS_REALESRGAN_PROFILE,
                )?,
                windows_qwen_image_21: Installation::new_for(
                    paths.clone(),
                    WINDOWS_QWEN_IMAGE_21_PROFILE,
                )?,
                windows_qwen_image_21_turbo: Installation::new_for(
                    paths.clone(),
                    WINDOWS_QWEN_IMAGE_21_TURBO_PROFILE,
                )?,
                windows_moss_image_sft: Installation::new_for(
                    paths,
                    WINDOWS_MOSS_IMAGE_SFT_PROFILE,
                )?,
            })
        }
    }

    pub fn snapshots(&self) -> Vec<InstallationSnapshot> {
        #[cfg(windows)]
        {
            // Fixed Windows order: Z-Image first, then the six parity models in
            // design.md section 11 order.
            vec![
                self.windows_zimage.snapshot(),
                self.windows_klein.snapshot(),
                self.windows_birefnet.snapshot(),
                self.windows_realesrgan.snapshot(),
                self.windows_qwen_image_21.snapshot(),
                self.windows_qwen_image_21_turbo.snapshot(),
                self.windows_moss_image_sft.snapshot(),
            ]
        }
        #[cfg(not(windows))]
        vec![
            self.legacy.snapshot(),
            self.klein.snapshot(),
            self.birefnet.snapshot(),
            self.realesrgan.snapshot(),
            self.qwen_image_21.snapshot(),
            self.qwen_image_21_turbo.snapshot(),
            self.moss_image_sft.snapshot(),
        ]
    }

    pub fn snapshot(&self, install_id: &str) -> Option<InstallationSnapshot> {
        self.for_id(install_id).map(Installation::snapshot)
    }

    pub fn start(&self, install_id: &str) -> Result<InstallationSnapshot, String> {
        if self.is_busy() {
            return Err("另一个型号正在安装，请等待完成或停止后再试。".into());
        }
        self.for_id(install_id)
            .ok_or_else(|| "未登记的安装标识。".to_string())?
            .start(install_id)
    }

    pub fn cancel(&self, install_id: &str) -> Result<InstallationSnapshot, String> {
        self.for_id(install_id)
            .ok_or_else(|| "该型号没有活动安装。".to_string())?
            .cancel();
        Ok(self
            .snapshot(install_id)
            .expect("validated installation identity"))
    }

    pub fn ready_paths(&self, install_id: &str) -> Result<ReadyPaths, String> {
        self.for_id(install_id)
            .ok_or_else(|| "未登记的安装标识。".to_string())?
            .ready_paths()
    }

    pub fn plan_environment(
        &self,
        install_id: &str,
    ) -> Result<studio_core::generation::PlanEnvironment, String> {
        self.for_id(install_id)
            .ok_or_else(|| "未登记的安装标识。".to_string())?
            .plan_environment()
    }

    pub fn is_busy(&self) -> bool {
        self.snapshots().into_iter().any(|snapshot| {
            matches!(
                snapshot.phase.as_str(),
                "preparing"
                    | "preparing_model"
                    | "downloading"
                    | "verifying"
                    | "installing_runtime"
                    | "probing"
            )
        })
    }

    pub fn shutdown(&self) {
        #[cfg(not(windows))]
        {
            self.legacy.cancel();
            self.klein.cancel();
            self.birefnet.cancel();
            self.realesrgan.cancel();
            self.qwen_image_21.cancel();
            self.qwen_image_21_turbo.cancel();
            self.moss_image_sft.cancel();
        }
        #[cfg(windows)]
        {
            self.windows_zimage.cancel();
            self.windows_klein.cancel();
            self.windows_birefnet.cancel();
            self.windows_realesrgan.cancel();
            self.windows_qwen_image_21.cancel();
            self.windows_qwen_image_21_turbo.cancel();
            self.windows_moss_image_sft.cancel();
        }
    }

    fn for_id(&self, install_id: &str) -> Option<&Installation> {
        #[cfg(windows)]
        match install_id {
            WINDOWS_ZIMAGE_INSTALL_ID => Some(&self.windows_zimage),
            WINDOWS_KLEIN_INSTALL_ID => Some(&self.windows_klein),
            WINDOWS_BIREFNET_INSTALL_ID => Some(&self.windows_birefnet),
            WINDOWS_REALESRGAN_INSTALL_ID => Some(&self.windows_realesrgan),
            WINDOWS_QWEN_IMAGE_21_INSTALL_ID => Some(&self.windows_qwen_image_21),
            WINDOWS_QWEN_IMAGE_21_TURBO_INSTALL_ID => Some(&self.windows_qwen_image_21_turbo),
            WINDOWS_MOSS_IMAGE_SFT_INSTALL_ID => Some(&self.windows_moss_image_sft),
            _ => None,
        }
        #[cfg(not(windows))]
        match install_id {
            INSTALL_ID => Some(&self.legacy),
            KLEIN_INSTALL_ID => Some(&self.klein),
            BIREFNET_INSTALL_ID => Some(&self.birefnet),
            REALESRGAN_INSTALL_ID => Some(&self.realesrgan),
            QWEN_IMAGE_21_INSTALL_ID => Some(&self.qwen_image_21),
            QWEN_IMAGE_21_TURBO_INSTALL_ID => Some(&self.qwen_image_21_turbo),
            MOSS_IMAGE_SFT_INSTALL_ID => Some(&self.moss_image_sft),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct LicenseFileRecord {
    path: String,
    bytes: u64,
    sha256: String,
}
/// One file of the installer-visible prepared quantized cache. The installer
/// hashes the whole cache after `--prepare` and binds that inventory into the
/// ready record so a stale or tampered cache is never reused.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct PreparedFileRecord {
    path: String,
    bytes: u64,
    sha256: String,
}
/// Result of a staged prepare run: fixed cache location, deterministic inventory
/// digest and the measured memory preflight values.
#[derive(Debug)]
struct PreparedCacheOutcome {
    path: String,
    files: Vec<PreparedFileRecord>,
    digest: String,
    preflight: PreparePreflightRecord,
}
/// Identity of one embedded worker wheel installed into the shared v2 runtime.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct WorkerWheelRecord {
    path: String,
    bytes: u64,
    sha256: String,
}
/// Measured memory values for the install-time prepare preflight.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct PreparePreflightRecord {
    available_physical_bytes: u64,
    available_commit_bytes: u64,
    minimum_available_physical_bytes: u64,
    minimum_commit_headroom_bytes: u64,
}
#[derive(Serialize, Deserialize)]
struct ReadyRecord {
    runtime_lock_id: String,
    model_snapshot_id: String,
    #[serde(default)]
    runtime_manifest_sha256: String,
    #[serde(default)]
    model_manifest_sha256: String,
    #[serde(default)]
    licenses: Vec<LicenseFileRecord>,
    /// `<data>/models/<model.id>.prepared` for staged Qwen/MOSS models.
    #[serde(default)]
    prepared_dir: Option<String>,
    /// Deterministic inventory of the prepared cache written by this install.
    #[serde(default)]
    prepared_files: Vec<PreparedFileRecord>,
    /// SHA-256 over `prepared_files` (sorted path/bytes/hash).
    #[serde(default)]
    prepared_digest: Option<String>,
    /// Embedded worker wheels installed into the shared runtime by this build.
    #[serde(default)]
    worker_wheels: Vec<WorkerWheelRecord>,
    /// Measured prepare preflight values (staged models only).
    #[serde(default)]
    prepare_preflight: Option<PreparePreflightRecord>,
    runtime_files: Vec<InventoryEntry>,
    probe: serde_json::Value,
}
impl ReadyRecord {
    fn matches_profile(&self, profile: InstallProfile) -> bool {
        self.runtime_manifest_sha256 == manifest_digest(profile.runtime_manifest)
            && self.model_manifest_sha256 == manifest_digest(profile.model_manifest)
            && licenses_match_profile(profile, &self.licenses)
            && worker_wheels_match(profile, &self.worker_wheels)
    }
}
fn license_root(data: &Path, profile: InstallProfile) -> PathBuf {
    data.join("licenses").join(profile.install_id)
}
fn license_records(profile: InstallProfile) -> Vec<LicenseFileRecord> {
    profile
        .bundle_licenses
        .iter()
        .map(|(path, bytes)| LicenseFileRecord {
            path: (*path).into(),
            bytes: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
        })
        .collect()
}
fn bundle_license_bytes(profile: InstallProfile) -> u64 {
    profile
        .bundle_licenses
        .iter()
        .map(|(_, bytes)| bytes.len() as u64)
        .sum()
}
/// The worker derives the prepared cache from the model path (`<model>.prepared`),
/// so the installer must use exactly that fixed location (design contract with the
/// Windows diffusers worker).
fn prepared_cache_dir(data: &Path, model_snapshot_id: &str) -> PathBuf {
    data.join("models")
        .join(format!("{model_snapshot_id}.prepared"))
}
fn prepared_cache_ready(dir: &Path) -> Result<(), String> {
    for component in ["text_encoder", "transformer"] {
        let config = dir.join(component).join("config.json");
        if !config.is_file() {
            return Err(format!(
                "量化缓存缺少 {}/config.json；请重新安装该型号以重建量化缓存。",
                component
            ));
        }
    }
    Ok(())
}
/// Deterministic inventory of the prepared cache: recursive files, no symlinks,
/// forward-slash relative paths, sorted for a stable digest.
fn prepared_cache_inventory(dir: &Path) -> Result<Vec<PreparedFileRecord>, String> {
    let mut files = Vec::new();
    prepared_inventory_walk(dir, dir, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    if files.is_empty() {
        return Err("量化缓存为空；请重新安装该型号以重建量化缓存。".into());
    }
    Ok(files)
}
fn prepared_inventory_walk(
    root: &Path,
    path: &Path,
    out: &mut Vec<PreparedFileRecord>,
) -> Result<(), String> {
    for entry in fs::read_dir(path).map_err(err)? {
        let entry = entry.map_err(err)?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(err)?;
        if metadata.file_type().is_symlink() {
            return Err("量化缓存包含符号链接；请重新安装该型号。".into());
        }
        if metadata.is_dir() {
            prepared_inventory_walk(root, &path, out)?;
        } else if metadata.is_file() {
            let mut file = File::open(&path).map_err(err)?;
            let mut digest = Sha256::new();
            std::io::copy(&mut file, &mut digest).map_err(err)?;
            out.push(PreparedFileRecord {
                path: path
                    .strip_prefix(root)
                    .map_err(err)?
                    .to_string_lossy()
                    .replace('\\', "/"),
                bytes: metadata.len(),
                sha256: format!("{:x}", digest.finalize()),
            });
        } else {
            return Err("量化缓存包含特殊文件；请重新安装该型号。".into());
        }
    }
    Ok(())
}
fn prepared_inventory_digest(files: &[PreparedFileRecord]) -> Result<String, String> {
    if files.is_empty() {
        return Err("量化缓存为空；请重新安装该型号以重建量化缓存。".into());
    }
    let mut digest = Sha256::new();
    for file in files {
        digest.update(file.path.as_bytes());
        digest.update([0]);
        digest.update(file.bytes.to_le_bytes());
        digest.update([0]);
        digest.update(file.sha256.as_bytes());
        digest.update(b"\n");
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
/// Startup check: the ready record binds a non-empty prepared-cache inventory and
/// every recorded file is present at its recorded size. Cheap; the full digest is
/// re-hashed in `prepared_cache_matches` before execution.
fn prepared_cache_metadata_matches(
    data: &Path,
    model_snapshot_id: &str,
    profile: InstallProfile,
    record: &ReadyRecord,
) -> bool {
    if !profile.prepare_with_model {
        return true;
    }
    let expected = prepared_cache_dir(data, model_snapshot_id);
    if record.prepared_dir.as_deref() != expected.to_str() {
        return false;
    }
    let Some(digest) = record.prepared_digest.as_deref() else {
        return false;
    };
    if !valid_digest(digest) || record.prepared_files.is_empty() {
        return false;
    }
    if prepared_cache_ready(&expected).is_err() {
        return false;
    }
    record.prepared_files.iter().all(|file| {
        safe_relative(&file.path).is_ok()
            && fs::symlink_metadata(expected.join(&file.path))
                .is_ok_and(|metadata| metadata.is_file() && metadata.len() == file.bytes)
    })
}
fn prepared_cache_matches(
    data: &Path,
    model_snapshot_id: &str,
    profile: InstallProfile,
    record: &ReadyRecord,
) -> Result<(), String> {
    if !profile.prepare_with_model {
        return Ok(());
    }
    let expected = prepared_cache_dir(data, model_snapshot_id);
    let recorded = record
        .prepared_dir
        .as_deref()
        .ok_or("就绪记录缺少量化缓存路径；请重新安装。")?;
    if Path::new(recorded) != expected {
        return Err("量化缓存路径与固定安装布局不符；请重新安装。".into());
    }
    prepared_cache_ready(&expected)?;
    let files = prepared_cache_inventory(&expected)?;
    let digest = prepared_inventory_digest(&files)?;
    let recorded_digest = record
        .prepared_digest
        .as_deref()
        .ok_or("就绪记录缺少量化缓存摘要；请重新安装所选型号以重建量化缓存。")?;
    if recorded_digest != digest || record.prepared_files != files {
        return Err("量化缓存内容与安装记录不符；请重新安装所选型号以重建量化缓存。".into());
    }
    Ok(())
}
fn prepare_worker_args(
    profile: InstallProfile,
    model_snapshot_id: &str,
    model_dir: &Path,
    prepared_dir: &Path,
    force_rebuild: bool,
) -> Result<Vec<String>, String> {
    let mut args = vec![
        "-I".into(),
        "-m".into(),
        profile.worker_module.into(),
        "--model-snapshot-id".into(),
        model_snapshot_id.into(),
        "--prepare".into(),
        "--model-dir".into(),
        model_dir.to_str().ok_or("模型路径不是 UTF-8")?.into(),
        "--prepared-dir".into(),
        prepared_dir
            .to_str()
            .ok_or("量化缓存路径不是 UTF-8")?
            .into(),
    ];
    if force_rebuild {
        args.push("--no-reuse-prepared".into());
    }
    Ok(args)
}
fn licenses_match_profile(profile: InstallProfile, recorded: &[LicenseFileRecord]) -> bool {
    let expected = license_records(profile);
    expected.len() == recorded.len()
        && expected.iter().zip(recorded).all(|(expected, recorded)| {
            expected.path == recorded.path
                && expected.bytes == recorded.bytes
                && expected.sha256 == recorded.sha256
        })
}
/// Startup/activation check: the archived license bytes are present at their
/// recorded identity. Full hashing still runs in `verify_ready`.
fn licenses_healthy(data: &Path, profile: InstallProfile) -> Result<(), String> {
    let root = license_root(data, profile);
    for license in license_records(profile) {
        safe_relative(&license.path)?;
        let metadata = fs::symlink_metadata(root.join(&license.path))
            .map_err(|_| format!("许可证文件缺失：{}", license.path))?;
        if !metadata.is_file() || metadata.len() != license.bytes {
            return Err(format!("许可证文件类型或大小不符：{}", license.path));
        }
    }
    Ok(())
}
const RUNTIME_ACTIVATION_RECORD: &str = "runtime-activation.json";
#[derive(Serialize, Deserialize)]
struct RuntimeActivation {
    runtime_lock_id: String,
    manifest_sha256: String,
    /// Embedded worker wheel identities installed into this runtime. A runtime
    /// built by an older app build is not reusable for python profiles because it
    /// still carries the old worker wheels; it is rebuilt instead.
    #[serde(default)]
    worker_wheels: Vec<WorkerWheelRecord>,
    files: Vec<InventoryEntry>,
}
/// A completed shared runtime is reusable only for the exact manifest identity,
/// the same embedded worker wheels, and while every recorded file still exists at
/// its recorded size. This is what makes installing the second model on a shared
/// runtime cheap and non-destructive, while a worker-wheel update self-heals.
fn runtime_activation_matches(
    root: &Path,
    runtime_id: &str,
    manifest_sha256: &str,
    worker_wheels: &[WorkerWheelRecord],
) -> bool {
    fs::read(root.join(RUNTIME_ACTIVATION_RECORD))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<RuntimeActivation>(&bytes).ok())
        .is_some_and(|activation| {
            activation.runtime_lock_id == runtime_id
                && activation.manifest_sha256 == manifest_sha256
                && activation.worker_wheels == worker_wheels
                && !activation.files.is_empty()
                && activation
                    .files
                    .iter()
                    .all(|entry| inventory_metadata_matches(root, entry).is_ok())
        })
}
fn write_runtime_activation(
    root: &Path,
    runtime_id: &str,
    manifest_sha256: &str,
    worker_wheels: &[WorkerWheelRecord],
) -> Result<(), String> {
    let mut files = Vec::new();
    inventory(root, root, &mut files)?;
    atomic_json(
        &root.join(RUNTIME_ACTIVATION_RECORD),
        &RuntimeActivation {
            runtime_lock_id: runtime_id.into(),
            manifest_sha256: manifest_sha256.into(),
            worker_wheels: worker_wheels.to_vec(),
            files,
        },
    )
}
#[cfg(any(windows, test))]
fn activate_staged_runtime(staging: &Path, runtime: &Path) -> Result<(), String> {
    if runtime.exists() {
        fs::remove_dir_all(runtime).map_err(err)?;
    }
    fs::rename(staging, runtime).map_err(err)?;
    sync_directory(runtime.parent().ok_or("运行时路径无父目录")?)
}
/// design.md section 11 freezes a worker snapshot identity for every new Windows
/// model; legacy profiles keep the model manifest id as their worker snapshot.
fn execution_snapshot_id(install_id: &str) -> Option<&'static str> {
    match install_id {
        WINDOWS_KLEIN_INSTALL_ID => Some(studio_core::generation::KLEIN_WINDOWS_SNAPSHOT),
        WINDOWS_BIREFNET_INSTALL_ID => Some(studio_core::generation::BIREFNET_WINDOWS_SNAPSHOT),
        WINDOWS_REALESRGAN_INSTALL_ID => Some(studio_core::generation::REALESRGAN_WINDOWS_SNAPSHOT),
        WINDOWS_QWEN_IMAGE_21_INSTALL_ID => {
            Some(studio_core::generation::QWEN_IMAGE_21_WINDOWS_SNAPSHOT)
        }
        WINDOWS_QWEN_IMAGE_21_TURBO_INSTALL_ID => {
            Some(studio_core::generation::QWEN_IMAGE_21_TURBO_WINDOWS_SNAPSHOT)
        }
        WINDOWS_MOSS_IMAGE_SFT_INSTALL_ID => {
            Some(studio_core::generation::MOSS_IMAGE_SFT_WINDOWS_SNAPSHOT)
        }
        _ => None,
    }
}
/// Frozen execution-plan device identity; legacy macOS profiles keep using the
/// device name reported by their install probe.
fn plan_device_id(install_id: &str) -> Option<&'static str> {
    match install_id {
        WINDOWS_ZIMAGE_INSTALL_ID => Some(studio_core::generation::Z_IMAGE_WINDOWS_DEVICE),
        WINDOWS_KLEIN_INSTALL_ID
        | WINDOWS_QWEN_IMAGE_21_INSTALL_ID
        | WINDOWS_QWEN_IMAGE_21_TURBO_INSTALL_ID
        | WINDOWS_MOSS_IMAGE_SFT_INSTALL_ID => Some(studio_core::generation::WINDOWS_CUDA_DEVICE),
        WINDOWS_BIREFNET_INSTALL_ID => Some(studio_core::generation::WINDOWS_CPU_DEVICE),
        WINDOWS_REALESRGAN_INSTALL_ID => Some(studio_core::generation::WINDOWS_VULKAN_DEVICE),
        _ => None,
    }
}
/// Fixed worker wheels embedded in the binary for the shared v2 runtime. They are
/// treated as runtime artifacts even though the frozen product runtime manifest
/// still marks them as pending.
fn bundled_runtime_wheels(profile: InstallProfile) -> Vec<Artifact> {
    profile
        .shared_worker_wheels
        .iter()
        .map(|(path, bytes)| Artifact {
            path: (*path).into(),
            url: "bundled:worker-wheel".into(),
            bytes: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
        })
        .collect()
}
/// Embedded worker wheel identities of this build. They are part of the runtime
/// activation identity and the ready record, so a future app build that ships new
/// worker wheels rebuilds the shared runtime instead of deadlocking the probe.
fn bundled_worker_wheel_records(profile: InstallProfile) -> Vec<WorkerWheelRecord> {
    profile
        .shared_worker_wheels
        .iter()
        .map(|(path, bytes)| WorkerWheelRecord {
            path: (*path).into(),
            bytes: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
        })
        .collect()
}
fn worker_wheels_match(profile: InstallProfile, recorded: &[WorkerWheelRecord]) -> bool {
    bundled_worker_wheel_records(profile) == recorded
}
/// Identifies why an existing ready record no longer matches this build. Returns
/// `None` when every identity dimension matches.
fn ready_identity_error(
    record: &ReadyRecord,
    profile: InstallProfile,
    runtime_id: &str,
    snapshot_id: &str,
) -> Option<String> {
    if record.runtime_lock_id != runtime_id || record.model_snapshot_id != snapshot_id {
        return Some("安装清单已变化或记录缺少字节身份，请修复安装。".into());
    }
    if record.runtime_manifest_sha256 != manifest_digest(profile.runtime_manifest)
        || record.model_manifest_sha256 != manifest_digest(profile.model_manifest)
    {
        return Some("安装清单已变化或记录缺少字节身份，请修复安装。".into());
    }
    if !licenses_match_profile(profile, &record.licenses) {
        return Some("安装许可证归档已变化，请修复安装。".into());
    }
    if !worker_wheels_match(profile, &record.worker_wheels) {
        return Some("内嵌计算模块已更新（worker wheel 身份不一致），请重新安装所选型号。".into());
    }
    None
}
fn ready_record_matches(
    record: &ReadyRecord,
    profile: InstallProfile,
    runtime_id: &str,
    snapshot_id: &str,
) -> bool {
    record.runtime_lock_id == runtime_id
        && record.model_snapshot_id == snapshot_id
        && record.matches_profile(profile)
}
fn manifest_digest(manifest: &str) -> String {
    format!("{:x}", Sha256::digest(manifest.as_bytes()))
}
fn canonical_requirements_lock(lock: &str) -> String {
    lock.replace("\r\n", "\n")
}
/// Formats a filesystem path for a child process that opens paths with legacy
/// Win32 APIs (`_wfopen` in the official ncnn build) and cannot read the
/// `\\?\` verbatim form that Rust canonicalization returns on Windows. Internal
/// installer paths, hashing and inventory keep the canonical form.
fn external_process_path(path: &Path) -> std::borrow::Cow<'_, str> {
    let text = path.to_string_lossy();
    #[cfg(windows)]
    {
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            return std::borrow::Cow::Owned(format!(r"\\{rest}"));
        }
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            return std::borrow::Cow::Owned(rest.to_string());
        }
    }
    text
}
/// Native runtimes are trusted only for the operating system their manifest froze.
fn native_runtime_platform() -> &'static str {
    if cfg!(windows) {
        "windows-amd64"
    } else {
        "macos-universal2"
    }
}

/// Install-time prepare memory gate (design.md section 10.2 item 3). Gate 0 measured
/// the staged Qwen text-encoder quantization transient at about 23.4 GiB of process
/// commit while system available RAM dropped as low as 0.58 GiB, so the installer
/// requires either enough available physical RAM or enough commit headroom before
/// it starts `--prepare`; otherwise the install fails before any quantization work.
const PREPARE_MIN_AVAILABLE_PHYSICAL_BYTES: u64 = 28 * 1024 * 1024 * 1024;
const PREPARE_MIN_COMMIT_HEADROOM_BYTES: u64 = 26 * 1024 * 1024 * 1024;
const GIB_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MemoryFacts {
    available_physical_bytes: u64,
    available_commit_bytes: u64,
}
impl MemoryFacts {
    fn preflight_record(self) -> PreparePreflightRecord {
        PreparePreflightRecord {
            available_physical_bytes: self.available_physical_bytes,
            available_commit_bytes: self.available_commit_bytes,
            minimum_available_physical_bytes: PREPARE_MIN_AVAILABLE_PHYSICAL_BYTES,
            minimum_commit_headroom_bytes: PREPARE_MIN_COMMIT_HEADROOM_BYTES,
        }
    }
}
fn gib_ratio(bytes: u64) -> f64 {
    bytes as f64 / GIB_BYTES as f64
}
fn prepare_memory_error(facts: MemoryFacts) -> String {
    format!(
        "可用内存不足，未开始构建 Qwen/MOSS 量化缓存（未激活任何安装文件）。当前可用物理内存 {:.1} GiB（要求 ≥{} GiB）或可用提交余量 {:.1} GiB（要求 ≥{} GiB）；请关闭其它占用内存的程序、确认系统虚拟内存/提交上限充足后重试安装。",
        gib_ratio(facts.available_physical_bytes),
        PREPARE_MIN_AVAILABLE_PHYSICAL_BYTES / GIB_BYTES,
        gib_ratio(facts.available_commit_bytes),
        PREPARE_MIN_COMMIT_HEADROOM_BYTES / GIB_BYTES,
    )
}
/// Pure decision over measured values, unit-tested at both boundaries.
fn prepare_memory_preflight(facts: MemoryFacts) -> Result<(), String> {
    if facts.available_physical_bytes >= PREPARE_MIN_AVAILABLE_PHYSICAL_BYTES
        || facts.available_commit_bytes >= PREPARE_MIN_COMMIT_HEADROOM_BYTES
    {
        Ok(())
    } else {
        Err(prepare_memory_error(facts))
    }
}

#[cfg(windows)]
#[repr(C)]
#[derive(Default)]
struct MemoryStatusEx {
    dw_length: u32,
    dw_memory_load: u32,
    ull_total_phys: u64,
    ull_avail_phys: u64,
    ull_total_page_file: u64,
    ull_avail_page_file: u64,
    ull_total_virtual: u64,
    ull_avail_virtual: u64,
    ull_avail_extended_virtual: u64,
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
}

#[cfg(windows)]
fn system_memory_facts() -> Result<MemoryFacts, String> {
    let mut status = MemoryStatusEx {
        dw_length: std::mem::size_of::<MemoryStatusEx>() as u32,
        ..MemoryStatusEx::default()
    };
    // SAFETY: `status` is a valid, correctly sized and initialized MEMORYSTATUSEX
    // buffer owned by this stack frame for the duration of the call.
    let succeeded = unsafe { GlobalMemoryStatusEx(&mut status) };
    if succeeded == 0 {
        return Err(format!(
            "无法读取系统内存状态（{}）；请修复系统后重试安装。",
            std::io::Error::last_os_error()
        ));
    }
    Ok(MemoryFacts {
        available_physical_bytes: status.ull_avail_phys,
        available_commit_bytes: status.ull_avail_page_file,
    })
}

#[cfg(not(windows))]
fn system_memory_facts() -> Result<MemoryFacts, String> {
    Err("分阶段模型的内存预检仅支持 Windows；未开始量化缓存构建。".into())
}
#[derive(Serialize, Deserialize)]
struct InventoryEntry {
    path: String,
    sha256: String,
    bytes: u64,
    #[serde(default)]
    link_target: Option<String>,
    #[serde(default)]
    mode: u32,
}

impl ArchiveArtifact {
    fn download(&self) -> Artifact {
        Artifact {
            path: self.path.clone(),
            url: self.url.clone(),
            bytes: self.bytes,
            sha256: self.sha256.clone(),
        }
    }

    fn members(&self) -> Vec<InstalledArtifact> {
        self.members
            .iter()
            .map(|member| InstalledArtifact {
                path: member.path.clone(),
                bytes: member.bytes,
                sha256: member.sha256.clone(),
                executable: member.executable,
            })
            .collect()
    }
}

impl ModelManifest {
    fn downloads(&self) -> Vec<Artifact> {
        self.files
            .iter()
            .cloned()
            .chain(self.archives.iter().map(ArchiveArtifact::download))
            .collect()
    }

    fn installed_files(&self) -> Vec<InstalledArtifact> {
        self.files
            .iter()
            .map(|file| InstalledArtifact {
                path: file.path.clone(),
                bytes: file.bytes,
                sha256: file.sha256.clone(),
                executable: false,
            })
            .chain(self.archives.iter().flat_map(ArchiveArtifact::members))
            .collect()
    }

    fn component_sources(&self) -> Vec<(String, InstalledArtifact)> {
        self.files
            .iter()
            .filter(|file| file.path != "README.md")
            .map(|file| {
                (
                    file.url.clone(),
                    InstalledArtifact {
                        path: file.path.clone(),
                        bytes: file.bytes,
                        sha256: file.sha256.clone(),
                        executable: false,
                    },
                )
            })
            .chain(self.archives.iter().flat_map(|archive| {
                archive
                    .members()
                    .into_iter()
                    .map(|member| (archive.url.clone(), member))
            }))
            .collect()
    }
}

impl RuntimeManifest {
    fn is_native(&self) -> bool {
        self.kind == "native"
    }

    fn downloads(&self) -> Vec<Artifact> {
        if self.is_native() {
            return self
                .archives
                .iter()
                .map(ArchiveArtifact::download)
                .collect();
        }
        self.interpreter
            .iter()
            .cloned()
            .chain(self.wheels.iter().cloned())
            .collect()
    }

    fn native_installed_files(&self) -> Vec<InstalledArtifact> {
        self.archives
            .iter()
            .flat_map(ArchiveArtifact::members)
            .collect()
    }

    fn python_version(&self) -> Result<&str, String> {
        self.python
            .as_deref()
            .ok_or_else(|| "Python 运行时清单缺少解释器版本。".into())
    }

    fn interpreter(&self) -> Result<&Artifact, String> {
        self.interpreter
            .as_ref()
            .ok_or_else(|| "Python 运行时清单缺少解释器归档。".into())
    }
}

impl Installation {
    pub fn new(paths: AppPaths) -> Result<Self, String> {
        Self::new_for(paths, Z_PROFILE)
    }

    pub fn new_klein(paths: AppPaths) -> Result<Self, String> {
        Self::new_for(paths, KLEIN_PROFILE)
    }

    pub fn new_birefnet(paths: AppPaths) -> Result<Self, String> {
        Self::new_for(paths, BIREFNET_PROFILE)
    }

    pub fn new_realesrgan(paths: AppPaths) -> Result<Self, String> {
        Self::new_for(paths, REALESRGAN_PROFILE)
    }

    fn new_for(mut paths: AppPaths, profile: InstallProfile) -> Result<Self, String> {
        paths.data = paths.data.canonicalize().map_err(err)?;
        paths.cache = paths.cache.canonicalize().map_err(err)?;
        paths.logs = paths.logs.canonicalize().map_err(err)?;
        let model: ModelManifest = serde_json::from_str(profile.model_manifest).map_err(err)?;
        let runtime: RuntimeManifest =
            serde_json::from_str(profile.runtime_manifest).map_err(err)?;
        for archive in &model.archives {
            validate_archive(archive)?;
        }
        for archive in &runtime.archives {
            validate_archive(archive)?;
        }
        let model_downloads = model.downloads();
        let mut runtime_downloads = runtime.downloads();
        for wheel in bundled_runtime_wheels(profile) {
            if !runtime_downloads
                .iter()
                .any(|existing| existing.path == wheel.path)
            {
                runtime_downloads.push(wheel);
            }
        }
        let snapshot_id = execution_snapshot_id(profile.install_id)
            .map_or_else(|| model.id.clone(), str::to_owned);
        for file in model_downloads.iter().chain(&runtime_downloads) {
            validate_artifact(file)?;
        }
        let installed_model_bytes = model
            .installed_files()
            .iter()
            .filter(|file| file.path != "README.md")
            .map(|file| file.bytes)
            .sum::<u64>();
        if installed_model_bytes != model.component_bytes {
            return Err(format!(
                "{} 模型清单组件字节与解压文件不符。",
                profile.install_id
            ));
        }
        if runtime.is_native() {
            if runtime.platform != native_runtime_platform() {
                return Err(format!(
                    "原生运行时平台不符；当前系统只接受 {} 清单。",
                    native_runtime_platform()
                ));
            }
            if runtime.executable.is_none()
                || runtime.interpreter.is_some()
                || !runtime.wheels.is_empty()
            {
                return Err("原生运行时清单结构无效。".into());
            }
        } else {
            runtime.python_version()?;
            runtime.interpreter()?;
            if !runtime.archives.is_empty() {
                return Err("Python 运行时不得包含原生归档。".into());
            }
        }
        let runtime_bytes = runtime_downloads.iter().map(|file| file.bytes).sum::<u64>();
        let model_file_bytes = model_downloads.iter().map(|file| file.bytes).sum::<u64>();
        let license_bytes = bundle_license_bytes(profile);
        // Initial upper bound is available before any network or large hashing:
        // full download cache + target model copy + runtime expansion + reserve.
        // start() refines missing cache bytes and checks both volumes separately.
        let initial_required_bytes = model_file_bytes * 2
            + runtime_bytes * 7
            + license_bytes
            + profile.prepared_cache_reserve
            + RESERVE;
        let mut state = InstallationSnapshot {
            install_id: profile.install_id.into(),
            phase: "not_installed".into(),
            received_bytes: 0,
            total_bytes: model_file_bytes + runtime_bytes,
            model_bytes: model.component_bytes,
            runtime_bytes,
            required_bytes: initial_required_bytes,
            error: None,
            runtime_ready: false,
            model_ready: false,
            device_verified: false,
        };
        // Startup reads activation metadata and checks only critical filesystem entries.
        // Full runtime inventory hashing happens off-thread before execution.
        if let Ok(record) = fs::read(paths.data.join(profile.ready_relative))
            .and_then(|b| serde_json::from_slice::<ReadyRecord>(&b).map_err(std::io::Error::other))
        {
            if let Some(identity_error) =
                ready_identity_error(&record, profile, &runtime.id, &snapshot_id)
            {
                state.phase = "unavailable".into();
                state.error = Some(identity_error);
            } else {
                let runtime_root = paths.data.join("runtimes").join(&runtime.id);
                let model_root = paths.data.join("models").join(&model.id);
                let runtime_critical_ok = if runtime.is_native() {
                    runtime.native_installed_files().iter().all(|file| {
                        record
                            .runtime_files
                            .iter()
                            .find(|entry| entry.path == file.path)
                            .is_some_and(|entry| {
                                inventory_metadata_matches(&runtime_root, entry).is_ok()
                                    && (!file.executable || entry.mode & 0o111 != 0)
                            })
                    })
                } else {
                    #[cfg(windows)]
                    let required = [
                        "python.exe".to_string(),
                        "python312._pth".to_string(),
                        format!("Lib/site-packages/{}/__init__.py", profile.worker_module),
                        format!("Lib/site-packages/{}/__main__.py", profile.worker_module),
                    ];
                    #[cfg(not(windows))]
                    let required = [
                        "python/bin/python3.13".to_string(),
                        "python/bin/python3".to_string(),
                        "venv/bin/python3".to_string(),
                        format!(
                            "venv/lib/python3.13/site-packages/{}/__init__.py",
                            profile.worker_module
                        ),
                        format!(
                            "venv/lib/python3.13/site-packages/{}/__main__.py",
                            profile.worker_module
                        ),
                    ];
                    required.iter().all(|path| {
                        record
                            .runtime_files
                            .iter()
                            .find(|entry| entry.path == *path)
                            .is_some_and(|entry| {
                                inventory_metadata_matches(&runtime_root, entry).is_ok()
                            })
                    })
                };
                let healthy = record.probe["device"] == profile.probe_device
                    && record.probe["identity"]["build_id"] == profile.worker_build_id
                    && record.probe["identity"]["runtime_lock_id"] == runtime.id
                    && record.probe["identity"]["model_snapshot_id"] == snapshot_id
                    && record.probe["device_name"]
                        .as_str()
                        .is_some_and(|s| !s.is_empty())
                    && !record.runtime_files.is_empty()
                    && model.installed_files().iter().all(|file| {
                        fs::symlink_metadata(model_root.join(&file.path)).is_ok_and(|metadata| {
                            metadata.is_file() && metadata.len() == file.bytes
                        })
                    })
                    && runtime_critical_ok
                    && licenses_healthy(&paths.data, profile).is_ok()
                    && prepared_cache_metadata_matches(&paths.data, &model.id, profile, &record);
                if healthy {
                    state.phase = "ready".into();
                    state.runtime_ready = true;
                    state.model_ready = true;
                    state.device_verified = true;
                } else {
                    state.phase = "unavailable".into();
                    state.error = Some("安装文件缺失或身份异常，请修复安装。".into());
                }
            }
        } else if paths.data.join(profile.installing_relative).exists() {
            state.phase = "interrupted".into();
        }
        Ok(Self {
            inner: Arc::new(Inner {
                paths,
                profile,
                model,
                runtime,
                state: Mutex::new(state),
                active: AtomicBool::new(false),
                cancelled: AtomicBool::new(false),
                aborted: AtomicBool::new(false),
            }),
        })
    }
    pub fn snapshot(&self) -> InstallationSnapshot {
        self.inner.state.lock().unwrap().clone()
    }
    pub fn start(&self, install_id: &str) -> Result<InstallationSnapshot, String> {
        if install_id != self.inner.profile.install_id {
            return Err("未登记的安装标识。".into());
        }
        #[cfg(not(windows))]
        if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            return Err("此运行时仅验证 macOS arm64。".into());
        }
        if self.snapshot().phase == "ready" {
            return Ok(self.snapshot());
        }
        if self
            .inner
            .active
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(self.snapshot());
        }
        self.inner.cancelled.store(false, Ordering::SeqCst);
        self.inner.aborted.store(false, Ordering::SeqCst);
        self.update(|s| {
            s.phase = "preparing".into();
            s.error = None;
            s.received_bytes = 0;
        });
        let this = self.clone();
        std::thread::spawn(move || {
            let result = this.install();
            if let Err(message) = result {
                this.update(|s| {
                    s.phase = if this.inner.cancelled.load(Ordering::SeqCst) {
                        "cancelled"
                    } else {
                        "failed"
                    }
                    .into();
                    s.error = Some(message);
                    s.runtime_ready = false;
                    s.model_ready = false;
                    s.device_verified = false;
                });
            }
            this.inner.active.store(false, Ordering::SeqCst);
        });
        Ok(self.snapshot())
    }
    pub fn cancel(&self) -> InstallationSnapshot {
        self.inner.cancelled.store(true, Ordering::SeqCst);
        self.snapshot()
    }
    pub fn ready_paths(&self) -> Result<ReadyPaths, String> {
        if self.inner.active.load(Ordering::SeqCst) {
            return Err("安装尚未完成。".into());
        }
        let result = self.verify_ready();
        if let Err(message) = &result {
            self.update(|s| {
                s.phase = "unavailable".into();
                s.error = Some(message.clone());
                s.model_ready = false;
                s.runtime_ready = false;
                s.device_verified = false;
            });
        }
        result
    }
    /// Frozen metadata, independent of UI/demo presets. Call after ready_paths has
    /// verified the installed bytes; this method performs no second 33 GB scan.
    pub fn plan_environment(&self) -> Result<studio_core::generation::PlanEnvironment, String> {
        use studio_core::{
            contracts::{ComponentSnapshot, DeviceSnapshot, RuntimeSnapshot, SourceRevision},
            domain::ContentHash,
            generation::PlanEnvironment,
        };
        let record: ReadyRecord = serde_json::from_slice(
            &fs::read(
                self.inner
                    .paths
                    .data
                    .join(self.inner.profile.ready_relative),
            )
            .map_err(err)?,
        )
        .map_err(err)?;
        if !ready_record_matches(
            &record,
            self.inner.profile,
            &self.inner.runtime.id,
            &self.snapshot_id(),
        ) || record.probe["device"] != self.inner.profile.probe_device
        {
            return Err("安装探针身份不符。".into());
        }
        let device = record.probe["device_name"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("安装探针缺少设备身份；请修复运行环境。")?;
        let components = self
            .inner
            .model
            .component_sources()
            .into_iter()
            .enumerate()
            .map(|(index, (source, file))| {
                Ok(ComponentSnapshot {
                    role: format!("component-{index:02}-{}", file.path.replace('/', "-")),
                    source: SourceRevision {
                        source,
                        revision: self.inner.model.revision.clone(),
                    },
                    content_hash: ContentHash::new(file.sha256.clone()).map_err(err)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(PlanEnvironment {
            components,
            runtime: RuntimeSnapshot {
                package_id: self.inner.runtime.id.clone(),
                engine: self.inner.profile.engine.into(),
                source: SourceRevision {
                    source: format!("image-studio:runtime-manifest:{}", self.inner.runtime.id),
                    revision: manifest_digest(self.inner.profile.runtime_manifest),
                },
            },
            device: DeviceSnapshot {
                device_id: plan_device_id(self.inner.profile.install_id)
                    .map_or_else(|| device.into(), str::to_owned),
                precision: self.inner.profile.precision.into(),
            },
            scheduler: self.inner.profile.scheduler.into(),
        })
    }
    fn runtime_dir(&self) -> PathBuf {
        self.inner
            .paths
            .data
            .join("runtimes")
            .join(&self.inner.runtime.id)
    }
    fn model_dir(&self) -> PathBuf {
        self.inner
            .paths
            .data
            .join("models")
            .join(&self.inner.model.id)
    }
    /// Worker/protocol snapshot identity (design.md section 11 for the new Windows models).
    fn snapshot_id(&self) -> String {
        execution_snapshot_id(self.inner.profile.install_id)
            .map_or_else(|| self.inner.model.id.clone(), str::to_owned)
    }
    fn runtime_wheels(&self) -> Vec<Artifact> {
        let mut wheels = self.inner.runtime.wheels.clone();
        for wheel in bundled_runtime_wheels(self.inner.profile) {
            if !wheels.iter().any(|existing| existing.path == wheel.path) {
                wheels.push(wheel);
            }
        }
        wheels
    }
    fn runtime_downloads(&self) -> Vec<Artifact> {
        let mut downloads = self.inner.runtime.downloads();
        for wheel in bundled_runtime_wheels(self.inner.profile) {
            if !downloads.iter().any(|existing| existing.path == wheel.path) {
                downloads.push(wheel);
            }
        }
        downloads
    }
    fn bundled_worker_wheel_bytes(&self, path: &str) -> Option<&'static [u8]> {
        self.inner
            .profile
            .shared_worker_wheels
            .iter()
            .find(|(candidate, _)| *candidate == path)
            .map(|(_, bytes)| *bytes)
            .or_else(|| {
                (!self.inner.profile.worker_wheel.is_empty())
                    .then_some(self.inner.profile.worker_wheel)
            })
    }
    fn cache(&self) -> PathBuf {
        self.inner.paths.cache.join("downloads")
    }
    fn update(&self, f: impl FnOnce(&mut InstallationSnapshot)) {
        f(&mut self.inner.state.lock().unwrap());
    }
    fn check_cancel(&self) -> Result<(), String> {
        if self.inner.cancelled.load(Ordering::SeqCst) {
            Err("安装已取消；已校验文件可供继续。".into())
        } else if self.inner.aborted.load(Ordering::SeqCst) {
            Err("另一个文件下载失败，本次安装已停止。".into())
        } else {
            Ok(())
        }
    }
    fn verify_ready(&self) -> Result<ReadyPaths, String> {
        let record: ReadyRecord = serde_json::from_slice(
            &fs::read(
                self.inner
                    .paths
                    .data
                    .join(self.inner.profile.ready_relative),
            )
            .map_err(err)?,
        )
        .map_err(err)?;
        if !worker_wheels_match(self.inner.profile, &record.worker_wheels) {
            return Err(
                "内嵌计算模块已更新（worker wheel 身份不一致），请重新安装所选型号。".into(),
            );
        }
        if !ready_record_matches(
            &record,
            self.inner.profile,
            &self.inner.runtime.id,
            &self.snapshot_id(),
        ) {
            return Err("安装身份已变化，请重新安装。".into());
        }
        prepared_cache_matches(
            &self.inner.paths.data,
            &self.inner.model.id,
            self.inner.profile,
            &record,
        )?;
        for file in self.inner.model.installed_files() {
            verify(&self.model_dir().join(&file.path), file.bytes, &file.sha256)?;
        }
        if record.runtime_files.is_empty()
            || record.probe["device"] != self.inner.profile.probe_device
        {
            return Err("运行时激活记录不完整。".into());
        }
        for f in record.runtime_files {
            inventory_metadata_matches(&self.runtime_dir(), &f)?;
            if f.link_target.is_none() {
                verify(&self.runtime_dir().join(f.path), f.bytes, &f.sha256)?;
            }
        }
        let license_root = license_root(&self.inner.paths.data, self.inner.profile);
        for license in license_records(self.inner.profile) {
            verify(
                &license_root.join(&license.path),
                license.bytes,
                &license.sha256,
            )?;
        }
        let (worker_program, worker_args) = if self.inner.runtime.is_native() {
            let executable = self
                .inner
                .runtime
                .executable
                .as_deref()
                .ok_or("原生运行时缺少 executable")?;
            let program = self.runtime_dir().join(executable);
            (
                program.clone(),
                vec![
                    "--ncnn-binary".into(),
                    program.to_str().ok_or("ncnn 路径不是 UTF-8")?.into(),
                ],
            )
        } else {
            let mut arguments = vec![
                "-I".into(),
                "-m".into(),
                self.inner.profile.worker_module.into(),
            ];
            if self.inner.profile.worker_model_argument {
                arguments.push("--model-snapshot-id".into());
                arguments.push(self.snapshot_id());
            }
            #[cfg(windows)]
            let program = self.runtime_dir().join("python.exe");
            #[cfg(not(windows))]
            let program = self.runtime_dir().join("venv/bin/python3");
            (program, arguments)
        };
        Ok(ReadyPaths {
            worker_program,
            worker_args,
            model_path: self.model_dir(),
            worker_module: self.inner.profile.worker_module.into(),
            worker_build_id: self.inner.profile.worker_build_id.into(),
            runtime_lock_id: self.inner.runtime.id.clone(),
            model_snapshot_id: self.snapshot_id(),
        })
    }
    fn install(&self) -> Result<(), String> {
        ensure_child_dir(&self.inner.paths.cache, &self.cache())?;
        ensure_child_dir(
            &self.inner.paths.data,
            self.inner
                .paths
                .data
                .join(self.inner.profile.installing_relative)
                .parent()
                .ok_or("安装记录无父目录")?,
        )?;
        let lock = open_regular(
            &self.inner.paths.data.join("installation.lock"),
            true,
            false,
        )?;
        fs2::FileExt::try_lock_exclusive(&lock)
            .map_err(|_| "另一个进程正在安装，请等待后重试。".to_string())?;
        let mut all = self.inner.model.downloads();
        all.extend(self.runtime_downloads());
        let missing: u64 = all
            .iter()
            .filter(|a| !self.cache().join(&a.sha256).exists())
            .map(|a| a.bytes)
            .sum();
        let target = self.inner.model.component_bytes;
        // Conservative full target copy and private expansion budget. Cache and data
        // may share a volume, in which case both peaks must fit simultaneously.
        let runtime_download_bytes = self
            .runtime_downloads()
            .iter()
            .map(|artifact| artifact.bytes)
            .sum::<u64>();
        let runtime_budget =
            runtime_download_bytes * if self.inner.runtime.is_native() { 3 } else { 6 };
        let license_bytes = bundle_license_bytes(self.inner.profile);
        let prepared_reserve = self.inner.profile.prepared_cache_reserve;
        let required =
            missing + target + license_bytes + prepared_reserve + runtime_budget + RESERVE;
        self.update(|s| s.required_bytes = required);
        // Check both volumes independently; never rely on cross-volume rename.
        let data_free = fs2::available_space(&self.inner.paths.data).map_err(err)?;
        let cache_free = fs2::available_space(&self.inner.paths.cache).map_err(err)?;
        if !installation_space_available(
            data_free,
            cache_free,
            missing,
            target
                .saturating_add(license_bytes)
                .saturating_add(prepared_reserve),
            runtime_budget,
            RESERVE,
            same_volume(&self.inner.paths.data, &self.inner.paths.cache)?,
        ) {
            return Err("磁盘空间不足；请释放安装计划所需空间后继续。".into());
        }
        atomic_json(
            &self
                .inner
                .paths
                .data
                .join(self.inner.profile.installing_relative),
            &serde_json::json!({"runtime_lock_id":self.inner.runtime.id,"model_snapshot_id":self.snapshot_id()}),
        )?;
        self.update(|s| s.phase = "downloading".into());
        // Four bounded workers, each keeps at most a 1 MiB transfer buffer.
        let first_error = Mutex::new(None);
        std::thread::scope(|scope| -> Result<(), String> {
            let split = all.len().div_ceil(4);
            let error = &first_error;
            let handles: Vec<_> = all
                .chunks(split)
                .map(|chunk| {
                    scope.spawn(move || {
                        let result = (|| -> Result<(), String> {
                            for artifact in chunk {
                                self.check_cancel()?;
                                self.download(artifact)?;
                            }
                            Ok(())
                        })();
                        if let Err(message) = result {
                            let mut first = error.lock().unwrap_or_else(|p| p.into_inner());
                            if first.is_none() {
                                *first = Some(message);
                            }
                            self.inner.aborted.store(true, Ordering::SeqCst);
                        }
                    })
                })
                .collect();
            for handle in handles {
                handle.join().map_err(|_| "下载线程异常。".to_string())?;
            }
            Ok(())
        })?;
        if let Some(error) = first_error.into_inner().unwrap_or_else(|p| p.into_inner()) {
            return Err(error);
        }
        self.check_cancel()?;
        self.update(|s| s.phase = "verifying".into());
        ensure_child_dir(&self.inner.paths.data, &self.model_dir())?;
        for artifact in &self.inner.model.files {
            self.check_cancel()?;
            copy_verified(
                &self.cache().join(&artifact.sha256),
                &self.model_dir().join(&artifact.path),
                artifact,
                &self.inner.paths.data,
            )?;
        }
        for archive in &self.inner.model.archives {
            self.install_archive_members(archive, &self.model_dir())?;
        }
        self.update(|s| {
            s.model_ready = true;
            s.phase = "installing_runtime".into();
        });
        let mut record = self.prepare_runtime()?;
        record.worker_wheels = bundled_worker_wheel_records(self.inner.profile);
        if self.inner.profile.prepare_with_model {
            let prepared = self.prepare_quantized_cache()?;
            record.prepared_dir = Some(prepared.path);
            record.prepared_files = prepared.files;
            record.prepared_digest = Some(prepared.digest);
            record.prepare_preflight = Some(prepared.preflight);
        }
        // Fixed license texts are archived only after the runtime/probe succeeded,
        // and before the activation record so a failed write never activates.
        record.licenses = self.install_licenses()?;
        ensure_child_dir(
            &self.inner.paths.data,
            self.inner
                .paths
                .data
                .join(self.inner.profile.ready_relative)
                .parent()
                .ok_or("激活记录无父目录")?,
        )?;
        atomic_json(
            &self
                .inner
                .paths
                .data
                .join(self.inner.profile.ready_relative),
            &record,
        )?;
        let _ = fs::remove_file(
            self.inner
                .paths
                .data
                .join(self.inner.profile.installing_relative),
        );
        self.update(|s| {
            s.phase = "ready".into();
            s.runtime_ready = true;
            s.model_ready = true;
            s.device_verified = true;
            s.error = None;
            s.received_bytes = s.total_bytes;
        });
        Ok(())
    }
    /// Archive the fixed, allowlist-external license texts under
    /// `<data>/licenses/<install_id>/`. The identity is the embedded bytes, so a
    /// reinstall is idempotent and never touches another model's directory.
    fn install_licenses(&self) -> Result<Vec<LicenseFileRecord>, String> {
        let root = license_root(&self.inner.paths.data, self.inner.profile);
        for &(relative, bytes) in self.inner.profile.bundle_licenses {
            self.check_cancel()?;
            safe_relative(relative)?;
            let target = root.join(relative);
            ensure_child_dir(
                &self.inner.paths.data,
                target.parent().ok_or("许可证路径无父目录")?,
            )?;
            if fs::read(&target).is_ok_and(|existing| existing.as_slice() == bytes) {
                continue;
            }
            let temporary = target.with_extension("install-tmp");
            let mut file = open_regular(&temporary, true, true)?;
            file.write_all(bytes).map_err(err)?;
            file.sync_all().map_err(err)?;
            drop(file);
            let record = LicenseFileRecord {
                path: relative.into(),
                bytes: bytes.len() as u64,
                sha256: format!("{:x}", Sha256::digest(bytes)),
            };
            verify(&temporary, record.bytes, &record.sha256)?;
            fs::rename(&temporary, &target).map_err(err)?;
            sync_directory(target.parent().ok_or("许可证路径无父目录")?)?;
        }
        Ok(license_records(self.inner.profile))
    }
    fn prepare_runtime(&self) -> Result<ReadyRecord, String> {
        if self.inner.runtime.is_native() {
            return self.prepare_native_runtime();
        }
        self.prepare_python_runtime()
    }
    fn prepare_python_runtime(&self) -> Result<ReadyRecord, String> {
        let interpreter = self.inner.runtime.interpreter()?;
        verify(
            &self.cache().join(&interpreter.sha256),
            interpreter.bytes,
            &interpreter.sha256,
        )?;
        let runtime = self.runtime_dir();
        ensure_child_dir(&self.inner.paths.data, &runtime)?;
        let runtime_manifest_sha256 = manifest_digest(self.inner.profile.runtime_manifest);
        let worker_wheels = bundled_worker_wheel_records(self.inner.profile);
        // Shared runtimes (several Windows v2 models, several macOS v3 models) must
        // never be torn down by another model's install. A completed runtime with the
        // same manifest identity and the same embedded worker wheels is reused as-is;
        // a worker-wheel change forces a staged rebuild that installs the new wheels.
        let reuse = runtime_activation_matches(
            &runtime,
            &self.inner.runtime.id,
            &runtime_manifest_sha256,
            &worker_wheels,
        );
        // On Windows the interpreter is a self-contained embeddable tree, so a rebuild
        // can be prepared in a staging directory and swapped in only after the wheels
        // and the probe succeed. A cancelled rebuild therefore cannot strand a model
        // that already uses the shared runtime.
        #[cfg(windows)]
        let (work, staged) = if reuse {
            (runtime.clone(), false)
        } else {
            let staging = self.staging_runtime_dir();
            if staging.exists() {
                fs::remove_dir_all(&staging).map_err(err)?;
            }
            ensure_child_dir(&self.inner.paths.data, &staging)?;
            (staging, true)
        };
        #[cfg(not(windows))]
        let work = {
            // A macOS venv cannot be renamed safely, so a manifest change rebuilds in
            // place; the activation record still makes the steady state reusable.
            if !reuse {
                fs::remove_dir_all(&runtime).map_err(err)?;
                ensure_child_dir(&self.inner.paths.data, &runtime)?;
            }
            runtime.clone()
        };
        if !reuse {
            extract_runtime_archive(
                &self.cache().join(&interpreter.sha256),
                &work,
                &interpreter.path,
            )?;
            #[cfg(windows)]
            {
                configure_embeddable_python(&work)?;
                let site_packages = work.join("Lib/site-packages");
                ensure_child_dir(&self.inner.paths.data, &site_packages)?;
                for wheel in self.runtime_wheels() {
                    self.check_cancel()?;
                    install_wheel(
                        &self.cache().join(&wheel.sha256),
                        &site_packages,
                        &self.inner.paths.data,
                    )?;
                }
                fs::write(
                    work.join("requirements.lock"),
                    canonical_requirements_lock(self.inner.profile.requirements_lock),
                )
                .map_err(err)?;
            }
            #[cfg(not(windows))]
            {
                let base_python = work.join("python/bin/python3");
                self.check_cancel()?;
                self.command(
                    Command::new(&base_python)
                        .args(["-I", "-m", "venv", "--clear"])
                        .arg(work.join("venv")),
                )?;
                let wheelhouse = work.join("wheelhouse");
                ensure_child_dir(&self.inner.paths.data, &wheelhouse)?;
                for wheel in self.runtime_wheels() {
                    copy_verified(
                        &self.cache().join(&wheel.sha256),
                        &wheelhouse.join(&wheel.path),
                        &wheel,
                        &self.inner.paths.data,
                    )?;
                }
                fs::write(
                    work.join("requirements.lock"),
                    canonical_requirements_lock(self.inner.profile.requirements_lock),
                )
                .map_err(err)?;
                self.command(
                    Command::new(work.join("venv/bin/python3"))
                        .args([
                            "-I",
                            "-m",
                            "pip",
                            "install",
                            "--no-input",
                            "--no-index",
                            "--no-deps",
                            "--require-hashes",
                            "--only-binary=:all:",
                            "--no-compile",
                            "--find-links",
                        ])
                        .arg(&wheelhouse)
                        .arg("-r")
                        .arg(work.join("requirements.lock")),
                )?;
            }
        }
        #[cfg(windows)]
        let python = work.join("python.exe");
        #[cfg(not(windows))]
        let python = work.join("venv/bin/python3");
        self.check_cancel()?;
        self.update(|s| s.phase = "probing".into());
        let snapshot_id = self.snapshot_id();
        let mut probe = Command::new(&python);
        probe.args(["-I", "-m", self.inner.profile.worker_module]);
        if self.inner.profile.worker_model_argument {
            probe.arg("--model-snapshot-id").arg(&snapshot_id);
        }
        probe.arg("--probe");
        if self.inner.profile.probe_with_model {
            probe.arg(self.model_dir());
        }
        let probe_output = self.command(&mut probe)?;
        let probe: serde_json::Value =
            serde_json::from_slice(&probe_output).map_err(|e| format!("探针输出无效：{e}"))?;
        if probe["python"].as_str() != Some(self.inner.runtime.python_version()?)
            || probe["identity"]["build_id"] != self.inner.profile.worker_build_id
            || probe["identity"]["runtime_lock_id"] != self.inner.runtime.id
            || probe["identity"]["model_snapshot_id"] != snapshot_id
            || probe["device"] != self.inner.profile.probe_device
        {
            return Err("运行时探针版本或设备不符。".into());
        }
        let versions=self.command(Command::new(&python).args(["-I","-c","import importlib.metadata,json;print(json.dumps({d.metadata['Name'].lower().replace('_','-'):d.version for d in importlib.metadata.distributions()}))"]))?;
        let versions: std::collections::BTreeMap<String, String> =
            serde_json::from_slice(&versions).map_err(err)?;
        for (name, version) in &self.inner.runtime.packages {
            if versions.get(name) != Some(version) {
                return Err(format!("依赖版本不符：{name}"));
            }
        }
        self.check_cancel()?;
        if !reuse {
            write_runtime_activation(
                &work,
                &self.inner.runtime.id,
                &runtime_manifest_sha256,
                &worker_wheels,
            )?;
        }
        #[cfg(windows)]
        if staged {
            activate_staged_runtime(&work, &runtime)?;
        }
        self.check_cancel()?;
        let mut runtime_files = Vec::new();
        inventory(&runtime, &runtime, &mut runtime_files)?;
        self.check_cancel()?;
        let record = ReadyRecord {
            runtime_lock_id: self.inner.runtime.id.clone(),
            model_snapshot_id: snapshot_id,
            runtime_manifest_sha256,
            model_manifest_sha256: manifest_digest(self.inner.profile.model_manifest),
            licenses: Vec::new(),
            prepared_dir: None,
            prepared_files: Vec::new(),
            prepared_digest: None,
            worker_wheels: Vec::new(),
            prepare_preflight: None,
            runtime_files,
            probe,
        };
        Ok(record)
    }
    #[cfg(windows)]
    fn staging_runtime_dir(&self) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        self.inner.paths.data.join("runtimes").join(format!(
            ".{}.staging-{}-{nonce}",
            self.inner.runtime.id,
            std::process::id()
        ))
    }
    fn prepare_native_runtime(&self) -> Result<ReadyRecord, String> {
        for archive in &self.inner.runtime.archives {
            verify(
                &self.cache().join(&archive.sha256),
                archive.bytes,
                &archive.sha256,
            )?;
        }
        let runtime = self.runtime_dir();
        ensure_child_dir(&self.inner.paths.data, &runtime)?;
        fs::remove_dir_all(&runtime).map_err(err)?;
        ensure_child_dir(&self.inner.paths.data, &runtime)?;
        for archive in &self.inner.runtime.archives {
            self.install_archive_members(archive, &runtime)?;
        }
        let executable = self
            .inner
            .runtime
            .executable
            .as_deref()
            .ok_or("原生运行时缺少 executable")?;
        let ncnn = runtime.join(executable);
        let executable_file = self
            .inner
            .runtime
            .native_installed_files()
            .into_iter()
            .find(|file| file.path == executable && file.executable)
            .ok_or("原生运行时 executable 未登记为可执行成员")?;
        verify(&ncnn, executable_file.bytes, &executable_file.sha256)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if ncnn.metadata().map_err(err)?.permissions().mode() & 0o111 == 0 {
                return Err("ncnn 可执行权限缺失。".into());
            }
        }
        self.check_cancel()?;
        self.update(|state| state.phase = "probing".into());
        let probe_dir = self.inner.paths.data.join("work/realesrgan-install-probe");
        if probe_dir.exists() {
            fs::remove_dir_all(&probe_dir).map_err(err)?;
        }
        ensure_child_dir(&self.inner.paths.data, &probe_dir)?;
        let input = probe_dir.join("input.png");
        let output = probe_dir.join("output.png");
        let mut image = image::RgbImage::new(8, 6);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = image::Rgb([
                (x * 31 % 256) as u8,
                (y * 47 % 256) as u8,
                ((x + y) * 19 % 256) as u8,
            ]);
        }
        image.save(&input).map_err(err)?;
        let model = self.model_dir();
        // The installer's own paths are Rust canonical paths, which carry the
        // `\\?\` verbatim prefix on Windows. The official ncnn executable opens
        // `-i/-o/-m` with `_wfopen` and cannot read verbatim paths, so external
        // process arguments get the plain Win32 form while internal hashing and
        // inventory keep using the canonical paths.
        let input_arg = external_process_path(&input);
        let output_arg = external_process_path(&output);
        let model_arg = external_process_path(&model);
        self.command(
            Command::new(&ncnn)
                .args([
                    "-i",
                    input_arg.as_ref(),
                    "-o",
                    output_arg.as_ref(),
                    "-s",
                    "4",
                    "-t",
                    "128",
                    "-m",
                    model_arg.as_ref(),
                    "-n",
                    "realesrgan-x4plus",
                    "-g",
                    "0",
                    "-j",
                    "1:1:1",
                    "-f",
                    "png",
                ])
                .args(["-v"]),
        )?;
        let dimensions = image::image_dimensions(&output).map_err(err)?;
        if dimensions != (32, 24) {
            return Err(format!("ncnn 探针输出尺寸不符：{dimensions:?}"));
        }
        let probe_log = fs::read_to_string(self.inner.paths.logs.join("installation-process.log"))
            .unwrap_or_default();
        let device_name = probe_log
            .lines()
            .find_map(|line| {
                line.strip_prefix("[0 ")
                    .and_then(|value| value.split_once(']'))
                    .map(|(name, _)| name.trim().to_string())
                    .filter(|name| !name.is_empty())
            })
            .ok_or("ncnn Vulkan 探针未报告 GPU 0")?;
        let probe_output_sha256 = format!("{:x}", Sha256::digest(fs::read(&output).map_err(err)?));
        let mut runtime_files = Vec::new();
        inventory(&runtime, &runtime, &mut runtime_files)?;
        fs::remove_dir_all(&probe_dir).map_err(err)?;
        self.check_cancel()?;
        Ok(ReadyRecord {
            runtime_lock_id: self.inner.runtime.id.clone(),
            model_snapshot_id: self.snapshot_id(),
            runtime_manifest_sha256: manifest_digest(self.inner.profile.runtime_manifest),
            model_manifest_sha256: manifest_digest(self.inner.profile.model_manifest),
            licenses: Vec::new(),
            prepared_dir: None,
            prepared_files: Vec::new(),
            prepared_digest: None,
            worker_wheels: Vec::new(),
            prepare_preflight: None,
            runtime_files,
            probe: serde_json::json!({
                "identity": {
                    "build_id": self.inner.profile.worker_build_id,
                    "runtime_lock_id": self.inner.runtime.id,
                    "model_snapshot_id": self.snapshot_id(),
                },
                "device": "gpu",
                "device_name": device_name,
                "gpu_id": 0,
                "backend": "ncnn Vulkan",
                "model": "realesrgan-x4plus",
                "scale": 4,
                "tile": 128,
                "threads": "1:1:1",
                "tta": false,
                "probe_input_size": [8, 6],
                "probe_output_size": [32, 24],
                "probe_output_sha256": probe_output_sha256,
            }),
        })
    }
    fn private_python(&self) -> PathBuf {
        let runtime = self.runtime_dir();
        #[cfg(windows)]
        {
            runtime.join("python.exe")
        }
        #[cfg(not(windows))]
        {
            runtime.join("venv/bin/python3")
        }
    }
    /// Best-effort append of the measured preflight values to the installer process
    /// log; the same numbers are persisted in the ready record.
    fn append_install_log(&self, message: &str) {
        use std::io::{Seek, SeekFrom};
        let path = self.inner.paths.logs.join("installation-process.log");
        if let Ok(mut file) = open_regular(&path, true, false) {
            let _ = file.seek(SeekFrom::End(0));
            let _ = writeln!(file, "{message}");
            let _ = file.sync_all();
        }
    }
    /// Whether the existing prepared cache can be reused instead of rebuilt. Reuse
    /// requires the exact same model manifest digest and a full current inventory
    /// digest equal to the previous ready record, so a model snapshot change or a
    /// tampered/stale cache always forces a clean deterministic rebuild.
    fn prepared_cache_reuse_allowed(&self, prepared: &Path) -> bool {
        if !prepared.is_dir() {
            return false;
        }
        let Ok(bytes) = fs::read(
            self.inner
                .paths
                .data
                .join(self.inner.profile.ready_relative),
        ) else {
            return false;
        };
        let Ok(previous) = serde_json::from_slice::<ReadyRecord>(&bytes) else {
            return false;
        };
        if previous.model_manifest_sha256 != manifest_digest(self.inner.profile.model_manifest) {
            return false;
        }
        if previous.prepared_dir.as_deref() != prepared.to_str() {
            return false;
        }
        let Ok(files) = prepared_cache_inventory(prepared) else {
            return false;
        };
        let Ok(digest) = prepared_inventory_digest(&files) else {
            return false;
        };
        previous.prepared_files == files
            && previous.prepared_digest.as_deref() == Some(digest.as_str())
    }
    /// Staged Qwen/MOSS installs build the deterministic NF4 cache through the
    /// diffusers worker. The worker derives the same path from the model path, so
    /// the fixed `<model>.prepared` location is both prepared and recorded here.
    fn prepare_quantized_cache(&self) -> Result<PreparedCacheOutcome, String> {
        self.prepare_quantized_cache_with_facts(system_memory_facts()?)
    }
    fn prepare_quantized_cache_with_facts(
        &self,
        facts: MemoryFacts,
    ) -> Result<PreparedCacheOutcome, String> {
        let log_line = format!(
            "[prepare-preflight] available_physical_bytes={} available_commit_bytes={} minimum_available_physical_bytes={} minimum_commit_headroom_bytes={}",
            facts.available_physical_bytes,
            facts.available_commit_bytes,
            PREPARE_MIN_AVAILABLE_PHYSICAL_BYTES,
            PREPARE_MIN_COMMIT_HEADROOM_BYTES
        );
        // The prepare subprocess truncates installation-process.log, so the measured
        // values are persisted separately as evidence as well.
        let _ = atomic_json(
            &self.inner.paths.logs.join("prepare-preflight.json"),
            &facts.preflight_record(),
        );
        // Fails before any quantization work and before the ready record exists.
        if let Err(error) = prepare_memory_preflight(facts) {
            self.append_install_log(&format!("{log_line} result=blocked"));
            return Err(error);
        }
        self.prepare_quantized_cache_after_preflight(facts, &log_line)
    }
    fn prepare_quantized_cache_after_preflight(
        &self,
        facts: MemoryFacts,
        log_line: &str,
    ) -> Result<PreparedCacheOutcome, String> {
        let prepared = prepared_cache_dir(&self.inner.paths.data, &self.inner.model.id);
        ensure_child_dir(
            &self.inner.paths.data,
            prepared.parent().ok_or("量化缓存无父目录")?,
        )?;
        self.check_cancel()?;
        self.update(|state| state.phase = "preparing_model".into());
        let reuse = self.prepared_cache_reuse_allowed(&prepared);
        if !reuse && prepared.exists() {
            // The model snapshot changed or the old cache failed its digest
            // binding: remove it so the rebuild cannot reuse stale components.
            fs::remove_dir_all(&prepared).map_err(err)?;
        }
        let model = self.model_dir();
        let mut command = Command::new(self.private_python());
        command.args(prepare_worker_args(
            self.inner.profile,
            &self.snapshot_id(),
            &model,
            &prepared,
            !reuse,
        )?);
        let output = self.command_with_deadline(&mut command, Duration::from_secs(1800));
        self.append_install_log(log_line);
        let output = output?;
        let report: serde_json::Value = serde_json::from_slice(&output)
            .map_err(|error| format!("量化缓存构建输出无效：{error}"))?;
        let status = report["status"].as_str().unwrap_or("");
        if !matches!(status, "prepared" | "reused") {
            let detail = report["error"]
                .as_str()
                .or_else(|| report["reason"].as_str())
                .unwrap_or("没有状态说明");
            return Err(format!(
                "量化缓存构建失败（{status}）：{detail}。请检查可用内存后重试安装。"
            ));
        }
        prepared_cache_ready(&prepared)?;
        let files = prepared_cache_inventory(&prepared)?;
        let digest = prepared_inventory_digest(&files)?;
        self.check_cancel()?;
        Ok(PreparedCacheOutcome {
            path: prepared.to_string_lossy().into_owned(),
            files,
            digest,
            preflight: facts.preflight_record(),
        })
    }
    fn install_archive_members(
        &self,
        archive: &ArchiveArtifact,
        destination: &Path,
    ) -> Result<(), String> {
        let archive_path = self.cache().join(&archive.sha256);
        verify(&archive_path, archive.bytes, &archive.sha256)?;
        let file = open_regular(&archive_path, false, false)?;
        let mut zip_archive = zip::ZipArchive::new(file).map_err(err)?;
        if zip_archive.len() > 4096 {
            return Err("归档成员数量超限。".into());
        }
        let mut names = std::collections::BTreeSet::new();
        for index in 0..zip_archive.len() {
            let entry = zip_archive.by_index(index).map_err(err)?;
            let entry_name = entry.name().map_err(err)?;
            let name = entry_name.trim_end_matches('/');
            safe_relative(name)?;
            let file_type = entry.unix_mode().unwrap_or(0) & 0o170000;
            if name.contains(['\\', ':', '\0'])
                || entry.enclosed_name().is_none()
                || entry.is_symlink()
                || !matches!(file_type, 0 | 0o100000 | 0o040000)
                || !names.insert(name.to_string())
            {
                return Err("归档包含链接、特殊文件、重复或越界路径。".into());
            }
        }
        for member in &archive.members {
            self.check_cancel()?;
            safe_relative(&member.path)?;
            safe_relative(&member.archive_path)?;
            let mut entry = zip_archive
                .by_name(&member.archive_path)
                .map_err(|error| format!("归档缺少 {}：{error}", member.archive_path))?;
            if !entry.is_file() || entry.size() != member.bytes {
                return Err(format!("归档成员类型或大小不符：{}", member.archive_path));
            }
            let target = destination.join(&member.path);
            ensure_child_dir(destination, target.parent().ok_or("归档成员目标无父目录")?)?;
            let temporary = target.with_extension("install-tmp");
            if temporary.exists() {
                fs::remove_file(&temporary).map_err(err)?;
            }
            let mut output = open_regular(&temporary, true, true)?;
            let copied = std::io::copy(&mut entry.by_ref().take(member.bytes + 1), &mut output)
                .map_err(err)?;
            output.sync_all().map_err(err)?;
            drop(output);
            if copied != member.bytes {
                let _ = fs::remove_file(&temporary);
                return Err(format!("归档成员字节数不符：{}", member.archive_path));
            }
            verify(&temporary, member.bytes, &member.sha256)?;
            if member.executable {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o755))
                        .map_err(err)?;
                }
            }
            if target.exists() {
                if verify(&target, member.bytes, &member.sha256).is_ok() {
                    fs::remove_file(&temporary).map_err(err)?;
                    continue;
                }
                fs::remove_file(&target).map_err(err)?;
            }
            fs::rename(&temporary, &target).map_err(err)?;
        }
        Ok(())
    }
    fn command(&self, command: &mut Command) -> Result<Vec<u8>, String> {
        self.command_with_deadline(command, Duration::from_secs(600))
    }
    fn command_with_deadline(
        &self,
        command: &mut Command,
        timeout: Duration,
    ) -> Result<Vec<u8>, String> {
        let log = open_regular(
            &self.inner.paths.logs.join("installation-process.log"),
            true,
            true,
        )?;
        command
            .env_remove("PYTHONPATH")
            .env_remove("PYTHONHOME")
            .env("PYTHONNOUSERSITE", "1")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .env("PIP_CONFIG_FILE", "/dev/null")
            .env("HF_HUB_OFFLINE", "1")
            .env("TRANSFORMERS_OFFLINE", "1")
            .env("HF_HUB_DISABLE_TELEMETRY", "1")
            .stdin(Stdio::null())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        #[cfg(windows)]
        let (mut child, job) =
            super::windows_process::spawn_suspended_in_job(command).map_err(err)?;
        #[cfg(not(windows))]
        let mut child = command.spawn().map_err(err)?;
        let mut stdout = child.stdout.take().ok_or("子进程没有输出通道。")?;
        let mut stderr = child.stderr.take().ok_or("子进程没有日志通道。")?;
        let capture = std::thread::spawn(move || -> std::io::Result<Vec<u8>> {
            let mut out = Vec::new();
            let mut buffer = [0; 8192];
            let mut overflow = false;
            loop {
                let n = stdout.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                if out.len() + n <= 1024 * 1024 {
                    out.extend_from_slice(&buffer[..n]);
                } else {
                    overflow = true;
                }
            }
            if overflow {
                Err(std::io::Error::other("安装探针输出超过 1 MiB"))
            } else {
                Ok(out)
            }
        });
        let logs = std::thread::spawn(move || -> std::io::Result<()> {
            let mut log = log;
            let mut count = 0;
            let mut buffer = [0; 8192];
            loop {
                let n = stderr.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                let keep = n.min((8 * 1024 * 1024_usize).saturating_sub(count));
                log.write_all(&buffer[..keep])?;
                count += keep;
            }
            log.sync_all()
        });
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if let Some(status) = child.try_wait().map_err(err)? {
                #[cfg(windows)]
                if let Err(error) = job.terminate() {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("无法清理安装探针进程树：{error}"));
                }
                let output = capture.join().map_err(|_| "探针读取失败。")?.map_err(err)?;
                logs.join()
                    .map_err(|_| "安装日志读取失败。")?
                    .map_err(err)?;
                if status.success() {
                    return Ok(output);
                }
                let detail = fs::read(self.inner.paths.logs.join("installation-process.log"))
                    .ok()
                    .map(|bytes| {
                        let skip = bytes.len().saturating_sub(4096);
                        String::from_utf8_lossy(&bytes[skip..]).into_owned()
                    })
                    .unwrap_or_default();
                return Err(format!(
                    "私有环境安装或探针失败 ({status})；安装日志末尾：{}",
                    detail.trim()
                ));
            }
            if self.check_cancel().is_err() || std::time::Instant::now() > deadline {
                #[cfg(windows)]
                if let Err(error) = job.terminate() {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("无法终止安装探针进程树：{error}"));
                }
                #[cfg(unix)]
                {
                    // SAFETY: this is the process group created for the still-owned child.
                    unsafe {
                        libc::kill(-(child.id() as i32), libc::SIGKILL);
                    }
                }
                let _ = child.kill();
                let _ = child.wait();
                let _ = capture.join();
                let _ = logs.join();
                return Err("安装子进程已取消或超时。".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    fn download(&self, a: &Artifact) -> Result<(), String> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(err)?
            .block_on(self.download_async(a))
    }
    async fn cancellable<T>(
        &self,
        operation: impl std::future::Future<Output = Result<T, reqwest::Error>>,
    ) -> Result<T, String> {
        tokio::pin!(operation);
        loop {
            tokio::select! {result=&mut operation=>return result.map_err(err),_=tokio::time::sleep(Duration::from_millis(100))=>self.check_cancel()?,}
        }
    }
    async fn download_async(&self, a: &Artifact) -> Result<(), String> {
        let final_path = self.cache().join(&a.sha256);
        if a.url == "bundled:worker-wheel" {
            let bytes = self
                .bundled_worker_wheel_bytes(&a.path)
                .ok_or_else(|| format!("缺少固定的 worker wheel 字节：{}", a.path))?;
            let mut file = open_regular(&final_path, true, true)?;
            file.write_all(bytes).map_err(err)?;
            file.sync_all().map_err(err)?;
            drop(file);
            verify(&final_path, a.bytes, &a.sha256)?;
            self.update(|s| s.received_bytes += a.bytes);
            return Ok(());
        }
        if final_path.exists() {
            if verify(&final_path, a.bytes, &a.sha256).is_ok() {
                self.update(|s| s.received_bytes += a.bytes);
                return Ok(());
            }
            fs::remove_file(&final_path).map_err(err)?;
        }
        if a.bytes >= SEGMENTED_DOWNLOAD_THRESHOLD && a.path.ends_with(".safetensors") {
            return self.download_segmented(a).await;
        }
        let client = http_client()?;
        let part = final_path.with_extension("part");
        let meta = final_path.with_extension("resume.json");
        let previous = fs::read(&meta)
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
        let offset = part.metadata().map(|m| m.len()).unwrap_or(0);
        let etag = previous
            .as_ref()
            .and_then(|m| m["etag"].as_str())
            .filter(|v| !v.starts_with("W/"));
        let resume = offset > 0
            && offset < a.bytes
            && etag.is_some()
            && previous.as_ref().is_some_and(|m| m["sha256"] == a.sha256);
        let mut request = client.get(&a.url);
        if resume {
            request = request
                .header(reqwest::header::RANGE, format!("bytes={offset}-"))
                .header(reqwest::header::IF_RANGE, etag.unwrap());
        }
        let mut response = self
            .cancellable(request.send())
            .await
            .map_err(|e| format!("网络中断：{}：{e}", a.path))?;
        let received_etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let append = resume
            && response.status() == reqwest::StatusCode::PARTIAL_CONTENT
            && received_etag == etag.unwrap_or("")
            && valid_range(
                response
                    .headers()
                    .get(reqwest::header::CONTENT_RANGE)
                    .and_then(|v| v.to_str().ok()),
                offset,
                a.bytes,
                a.bytes - offset,
            );
        if response.status() == reqwest::StatusCode::PARTIAL_CONTENT && !append {
            fs::remove_file(&part).ok();
            fs::remove_file(&meta).ok();
            return Err(format!(
                "{}：续传身份或范围不符，请继续以重新下载。",
                a.path
            ));
        }
        if !response.status().is_success() {
            return Err(format!("下载 {} 失败：HTTP {}", a.path, response.status()));
        }
        if let Some(length) = response.content_length()
            && length != a.bytes - if append { offset } else { 0 }
        {
            return Err(format!("{}：远端字节数不符。", a.path));
        }
        atomic_json(
            &meta,
            &serde_json::json!({"etag":received_etag,"sha256":a.sha256}),
        )?;
        let mut file = open_regular(&part, true, !append)?;
        if append {
            use std::io::{Seek, SeekFrom};
            file.seek(SeekFrom::End(0)).map_err(err)?;
        }
        let mut count = if append { offset } else { 0 };
        self.update(|s| s.received_bytes += count);
        loop {
            self.check_cancel()?;
            let Some(buffer) = self
                .cancellable(response.chunk())
                .await
                .map_err(|e| format!("网络中断：{}：{e}", a.path))?
            else {
                break;
            };
            let n = buffer.len();
            count += n as u64;
            if count > a.bytes {
                return Err("下载超过清单字节数。".into());
            }
            file.write_all(&buffer[..n]).map_err(err)?;
            self.update(|s| s.received_bytes += n as u64);
        }
        file.sync_all().map_err(err)?;
        drop(file);
        verify(&part, a.bytes, &a.sha256)?;
        fs::rename(&part, &final_path).map_err(err)?;
        fs::remove_file(&meta).ok();
        Ok(())
    }

    async fn download_segmented(&self, a: &Artifact) -> Result<(), String> {
        let final_path = self.cache().join(&a.sha256);
        let part = final_path.with_extension("part");
        let meta = final_path.with_extension("resume.json");
        let seed_marker = final_path.with_extension("segments-seeded");
        let ranges = segment_ranges(a.bytes, DOWNLOAD_SEGMENTS)?;
        let segment_paths = (0..ranges.len())
            .map(|index| part.with_extension(format!("segment-{index:02}")))
            .collect::<Vec<_>>();
        let segment_meta_paths = (0..ranges.len())
            .map(|index| part.with_extension(format!("segment-{index:02}.json")))
            .collect::<Vec<_>>();
        let previous = fs::read(&meta)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
        let mut etag = previous
            .as_ref()
            .and_then(|value| value["etag"].as_str())
            .filter(|value| !value.is_empty() && !value.starts_with("W/"))
            .map(str::to_owned);
        let client = http_client()?;
        if etag.is_none() {
            let response = self
                .cancellable(
                    client
                        .get(&a.url)
                        .header(reqwest::header::RANGE, "bytes=0-0")
                        .send(),
                )
                .await?;
            if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                return Err(format!(
                    "{}：服务器不支持分段续传（HTTP {}）。",
                    a.path,
                    response.status()
                ));
            }
            etag = response
                .headers()
                .get(reqwest::header::ETAG)
                .and_then(|value| value.to_str().ok())
                .filter(|value| !value.is_empty() && !value.starts_with("W/"))
                .map(str::to_owned);
            if !valid_range(
                response
                    .headers()
                    .get(reqwest::header::CONTENT_RANGE)
                    .and_then(|value| value.to_str().ok()),
                0,
                a.bytes,
                1,
            ) {
                return Err(format!("{}：分段探针范围不符。", a.path));
            }
        }
        let etag = etag.ok_or_else(|| format!("{}：缺少可靠 ETag，无法分段。", a.path))?;

        if !seed_marker.exists() {
            for path in segment_paths.iter().chain(&segment_meta_paths) {
                if path.exists() {
                    fs::remove_file(path).map_err(err)?;
                }
            }
            let seeded = if part.exists() {
                seed_segments(&part, &ranges, &segment_paths)?
            } else {
                for path in &segment_paths {
                    open_regular(path, true, true)?;
                }
                0
            };
            atomic_json(&seed_marker, &serde_json::json!({"sha256":a.sha256}))?;
            self.update(|state| state.received_bytes += seeded);
        } else {
            let existing = resumable_segment_bytes(
                &segment_paths,
                &segment_meta_paths,
                &ranges,
                &a.sha256,
                &etag,
            );
            self.update(|state| state.received_bytes += existing);
        }
        if part.exists() {
            fs::remove_file(&part).map_err(err)?;
        }

        // Keep one long-lived authenticated response in flight at a time.
        // The fixed ranges still provide restartability without splitting a
        // redirected CDN object across concurrent streams.
        for (index, (start, end)) in ranges.iter().copied().enumerate() {
            let path = segment_paths[index].clone();
            let metadata_path = segment_meta_paths[index].clone();
            let mut last_error = None;
            for attempt in 1..=SEGMENT_ATTEMPTS {
                match self
                    .download_segment(
                        &client,
                        SegmentDownload {
                            url: &a.url,
                            artifact_path: &a.path,
                            path: &path,
                            metadata_path: &metadata_path,
                            sha256: &a.sha256,
                            etag: &etag,
                            start,
                            end,
                            total: a.bytes,
                        },
                    )
                    .await
                {
                    Ok(()) => {
                        last_error = None;
                        break;
                    }
                    Err(error) => {
                        self.check_cancel()?;
                        last_error = Some(error);
                        if attempt < SEGMENT_ATTEMPTS {
                            tokio::time::sleep(Duration::from_secs(attempt as u64)).await;
                        }
                    }
                }
            }
            if let Some(error) = last_error {
                return Err(format!(
                    "{}：分段下载在 {SEGMENT_ATTEMPTS} 次尝试后失败：{error}",
                    a.path
                ));
            }
        }
        for ((start, end), path) in ranges.iter().zip(&segment_paths) {
            let length = end - start;
            if fs::metadata(path).map_err(err)?.len() != length {
                return Err(format!("{}：分段文件长度不符。", a.path));
            }
        }
        let mut output = open_regular(&part, true, true)?;
        for path in &segment_paths {
            let mut input = open_regular(path, false, false)?;
            std::io::copy(&mut input, &mut output).map_err(err)?;
        }
        output.sync_all().map_err(err)?;
        drop(output);
        if let Err(error) = verify(&part, a.bytes, &a.sha256) {
            let cleanup =
                clear_segment_state(&part, &segment_paths, &segment_meta_paths, &seed_marker);
            return Err(match cleanup {
                Ok(()) => format!("{error}；损坏分段已清除，请重试下载"),
                Err(cleanup_error) => {
                    format!("{error}；损坏分段清理失败：{cleanup_error}")
                }
            });
        }
        fs::rename(&part, &final_path).map_err(err)?;
        for path in segment_paths.iter().chain(&segment_meta_paths) {
            fs::remove_file(path).ok();
        }
        fs::remove_file(seed_marker).ok();
        fs::remove_file(meta).ok();
        Ok(())
    }

    async fn download_segment(
        &self,
        client: &reqwest::Client,
        request: SegmentDownload<'_>,
    ) -> Result<(), String> {
        let SegmentDownload {
            url,
            artifact_path,
            path,
            metadata_path,
            sha256,
            etag,
            start,
            end,
            total,
        } = request;
        let expected = end - start;
        let offset = fs::metadata(path).map(|value| value.len()).unwrap_or(0);
        if offset > expected {
            fs::remove_file(path).map_err(err)?;
        }
        let raw_offset = if path.exists() {
            fs::metadata(path).map_err(err)?.len()
        } else {
            open_regular(path, true, true)?;
            0
        };
        let previous = fs::read(metadata_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
        let identity_matches = previous.as_ref().is_some_and(|value| {
            value["sha256"] == sha256
                && value["etag"] == etag
                && value["start"] == start
                && value["end"] == end
        });
        if raw_offset == expected
            && identity_matches
            && previous
                .as_ref()
                .is_some_and(|value| value["offset"] == raw_offset)
        {
            atomic_json(
                metadata_path,
                &serde_json::json!({
                    "sha256": sha256,
                    "etag": etag,
                    "start": start,
                    "end": end,
                    "total": total,
                    "offset": raw_offset,
                }),
            )?;
            return Ok(());
        }
        let resumable = raw_offset > 0 && raw_offset < expected && identity_matches;
        let offset = if resumable { raw_offset } else { 0 };
        atomic_json(
            metadata_path,
            &serde_json::json!({
                "sha256": sha256,
                "etag": etag,
                "start": start,
                "end": end,
                "total": total,
                "offset": offset,
            }),
        )?;
        let request_start = start + offset;
        let response = self
            .cancellable(
                client
                    .get(url)
                    .header(
                        reqwest::header::RANGE,
                        format!("bytes={request_start}-{}", end - 1),
                    )
                    .header(reqwest::header::IF_RANGE, etag)
                    .send(),
            )
            .await?;
        let status = response.status();
        let received_etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok());
        let content_range = response
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|value| value.to_str().ok());
        if status != reqwest::StatusCode::PARTIAL_CONTENT
            || received_etag != Some(etag)
            || !valid_range(content_range, request_start, total, end - request_start)
        {
            return Err(format!(
                "{artifact_path}：分段 Range 响应身份或范围不符（HTTP {status}，ETag {received_etag:?}，期望 {etag:?}，Content-Range {content_range:?}，请求 bytes={request_start}-{}）。",
                end - 1
            ));
        }
        if let Some(length) = response.content_length()
            && length != end - request_start
        {
            return Err("分段响应字节数不符。".into());
        }
        let mut file = open_regular(path, true, offset == 0)?;
        if offset > 0 {
            use std::io::{Seek, SeekFrom};
            file.seek(SeekFrom::Start(offset)).map_err(err)?;
        }
        let mut response = response;
        let mut received = offset;
        loop {
            self.check_cancel()?;
            let Some(chunk) = self
                .cancellable(response.chunk())
                .await
                .map_err(|error| format!("{artifact_path}：分段读取中断：{error}"))?
            else {
                break;
            };
            received += chunk.len() as u64;
            if received > expected {
                return Err("分段下载超过清单范围。".into());
            }
            file.write_all(&chunk).map_err(err)?;
            self.update(|state| state.received_bytes += chunk.len() as u64);
        }
        file.sync_all().map_err(err)?;
        if received != expected {
            return Err("分段下载未达到清单长度。".into());
        }
        atomic_json(
            metadata_path,
            &serde_json::json!({
                "sha256": sha256,
                "etag": etag,
                "start": start,
                "end": end,
                "total": total,
                "offset": received,
            }),
        )?;
        Ok(())
    }
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn open_regular(path: &Path, write: bool, truncate: bool) -> Result<File, String> {
    super::safe_fs::open_regular(path, write, write, truncate).map_err(err)
}
fn ensure_child_dir(root: &Path, path: &Path) -> Result<(), String> {
    let relative = path.strip_prefix(root).map_err(err)?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(value) = component else {
            return Err("管理目录越界。".into());
        };
        current.push(value);
        match fs::symlink_metadata(&current) {
            Ok(m) if m.file_type().is_dir() => {}
            Ok(_) => return Err("管理目录包含链接或特殊文件。".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current).map_err(err)?
            }
            Err(e) => return Err(err(e)),
        }
    }
    Ok(())
}
fn same_volume(a: &Path, b: &Path) -> Result<bool, String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(fs::metadata(a).map_err(err)?.dev() == fs::metadata(b).map_err(err)?.dev())
    }
    #[cfg(not(unix))]
    {
        let _ = (a, b);
        Ok(true)
    }
}
fn inventory_metadata_matches(root: &Path, entry: &InventoryEntry) -> Result<(), String> {
    safe_relative(&entry.path)?;
    let path = root.join(&entry.path);
    if !path
        .parent()
        .ok_or("运行时路径无父目录。")?
        .canonicalize()
        .map_err(err)?
        .starts_with(root)
    {
        return Err("运行时父目录逃逸。".into());
    }
    let metadata = fs::symlink_metadata(&path).map_err(err)?;
    if let Some(target) = &entry.link_target {
        if !metadata.file_type().is_symlink()
            || fs::read_link(&path).map_err(err)?.to_string_lossy() != *target
            || !path.canonicalize().map_err(err)?.starts_with(root)
            || metadata.len() != entry.bytes
            || format!("{:x}", Sha256::digest(target.as_bytes())) != entry.sha256
        {
            return Err("运行时链接身份已变化。".into());
        }
    } else if !metadata.is_file() || metadata.len() != entry.bytes || {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            entry.mode != 0 && metadata.permissions().mode() != entry.mode
        }
        #[cfg(not(unix))]
        {
            false
        }
    } {
        return Err("运行时文件缺失、类型或大小不符。".into());
    }
    Ok(())
}
fn safe_relative(value: &str) -> Result<(), String> {
    if value.is_empty()
        || Path::new(value)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("清单或归档包含越界路径。".into());
    }
    Ok(())
}
fn allowed_url(url: &reqwest::Url) -> bool {
    let host = url.host_str();
    url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && host.is_some_and(|host| {
            host == "huggingface.co"
                || host.ends_with(".cdn.hf.co")
                || host.ends_with(".xethub.hf.co")
                || host == "modelscope.cn"
                || host.ends_with(".modelscope.cn")
                || matches!(
                    host,
                    "github.com"
                        | "release-assets.githubusercontent.com"
                        | "objects.githubusercontent.com"
                        | "files.pythonhosted.org"
                        | "www.python.org"
                        | "download.pytorch.org"
                        | "download-r2.pytorch.org"
                )
        })
}
fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(concat!("ImageStudio/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(30))
        .read_timeout(Duration::from_secs(45))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 6 || !allowed_url(attempt.url()) {
                attempt.error("下载重定向不在许可域名中")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(err)
}
fn segment_ranges(total: u64, count: usize) -> Result<Vec<(u64, u64)>, String> {
    if total == 0 || count == 0 || usize::try_from(total).is_err() {
        return Err("分段下载范围无效。".into());
    }
    let size = total.div_ceil(count as u64);
    let mut ranges = Vec::with_capacity(count);
    let mut start = 0;
    while start < total {
        let end = (start + size).min(total);
        ranges.push((start, end));
        start = end;
    }
    Ok(ranges)
}

fn resumable_segment_bytes(
    segment_paths: &[std::path::PathBuf],
    metadata_paths: &[std::path::PathBuf],
    ranges: &[(u64, u64)],
    sha256: &str,
    etag: &str,
) -> u64 {
    segment_paths
        .iter()
        .zip(metadata_paths)
        .zip(ranges)
        .filter_map(|((path, metadata_path), (start, end))| {
            let bytes = fs::metadata(path).ok()?.len();
            let metadata = fs::read(metadata_path).ok()?;
            let metadata = serde_json::from_slice::<serde_json::Value>(&metadata).ok()?;
            (bytes <= end - start
                && metadata["offset"] == bytes
                && metadata["sha256"] == sha256
                && metadata["etag"] == etag
                && metadata["start"] == *start
                && metadata["end"] == *end)
                .then_some(bytes)
        })
        .sum()
}
fn seed_segments(
    source_path: &Path,
    ranges: &[(u64, u64)],
    segment_paths: &[PathBuf],
) -> Result<u64, String> {
    use std::io::{Seek, SeekFrom};
    let mut source = open_regular(source_path, false, false)?;
    let available = source.metadata().map_err(err)?.len();
    let mut total = 0;
    for ((start, end), path) in ranges.iter().zip(segment_paths) {
        let mut output = open_regular(path, true, true)?;
        if available <= *start {
            continue;
        }
        let length = (available.min(*end) - *start).min(end - start);
        source.seek(SeekFrom::Start(*start)).map_err(err)?;
        if std::io::copy(
            &mut std::io::Read::by_ref(&mut source).take(length),
            &mut output,
        )
        .map_err(err)?
            != length
        {
            return Err("旧下载恢复文件无法完整分段。".into());
        }
        output.sync_all().map_err(err)?;
        total += length;
    }
    Ok(total)
}
fn clear_segment_state(
    part: &Path,
    segment_paths: &[PathBuf],
    segment_meta_paths: &[PathBuf],
    seed_marker: &Path,
) -> Result<(), String> {
    for path in std::iter::once(part)
        .chain(segment_paths.iter().map(PathBuf::as_path))
        .chain(segment_meta_paths.iter().map(PathBuf::as_path))
        .chain(std::iter::once(seed_marker))
    {
        if path.exists() {
            fs::remove_file(path).map_err(err)?;
        }
    }
    Ok(())
}
fn installation_space_available(
    data_free: u64,
    cache_free: u64,
    missing_cache: u64,
    target_files: u64,
    runtime_budget: u64,
    reserve: u64,
    same_volume: bool,
) -> bool {
    let data_required = target_files
        .saturating_add(runtime_budget)
        .saturating_add(reserve);
    let cache_required = missing_cache.saturating_add(reserve);
    data_free >= data_required
        && cache_free >= cache_required
        && (!same_volume
            || data_free
                >= missing_cache
                    .saturating_add(target_files)
                    .saturating_add(runtime_budget)
                    .saturating_add(reserve))
}
fn validate_archive(archive: &ArchiveArtifact) -> Result<(), String> {
    safe_relative(&archive.path)?;
    if !archive.path.ends_with(".zip") || archive.members.is_empty() {
        return Err("归档清单格式无效。".into());
    }
    let artifact = archive.download();
    validate_artifact(&artifact)?;
    let mut paths = std::collections::BTreeSet::new();
    for member in &archive.members {
        safe_relative(&member.archive_path)?;
        safe_relative(&member.path)?;
        if !paths.insert(&member.path)
            || member.bytes == 0
            || member.sha256.len() != 64
            || !member.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("归档成员身份无效或目标重复。".into());
        }
    }
    Ok(())
}
fn validate_artifact(a: &Artifact) -> Result<(), String> {
    safe_relative(&a.path)?;
    if a.bytes == 0 || a.sha256.len() != 64 || !a.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("清单缺少完整字节身份。".into());
    }
    if a.url != "bundled:worker-wheel" && !allowed_url(&reqwest::Url::parse(&a.url).map_err(err)?) {
        return Err("清单来源不在允许范围。".into());
    }
    Ok(())
}
fn valid_range(value: Option<&str>, offset: u64, total: u64, length: u64) -> bool {
    length > 0
        && offset < total
        && value.is_some_and(|s| s == format!("bytes {offset}-{}/{total}", offset + length - 1))
}
fn verify(path: &Path, bytes: u64, sha: &str) -> Result<(), String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| format!("文件缺失：{}", path.display()))?;
    if !metadata.is_file() || metadata.len() != bytes {
        return Err(format!("文件类型或大小不符：{}", path.display()));
    }
    let mut file = open_regular(path, false, false)?.take(bytes + 1);
    let mut digest = Sha256::new();
    let mut buffer = vec![0; 1024 * 1024];
    let mut count = 0;
    loop {
        let n = file.read(&mut buffer).map_err(err)?;
        if n == 0 {
            break;
        }
        count += n as u64;
        digest.update(&buffer[..n]);
    }
    if count != bytes {
        return Err("校验期间文件大小改变。".into());
    }
    if format!("{:x}", digest.finalize()) != sha {
        return Err(format!(
            "SHA-256 校验失败：{}；请继续以修复。",
            path.display()
        ));
    }
    Ok(())
}
fn copy_verified(source: &Path, dest: &Path, a: &Artifact, root: &Path) -> Result<(), String> {
    ensure_child_dir(root, dest.parent().ok_or("目标目录缺失。")?)?;
    if dest.exists() && verify(dest, a.bytes, &a.sha256).is_ok() {
        return Ok(());
    }
    let temporary = dest.with_extension("install-tmp");
    let mut input = open_regular(source, false, false)?.take(a.bytes + 1);
    let mut output = open_regular(&temporary, true, true)?;
    if std::io::copy(&mut input, &mut output).map_err(err)? != a.bytes {
        return Err("安装副本字节数不符。".into());
    }
    output.sync_all().map_err(err)?;
    drop(output);
    verify(&temporary, a.bytes, &a.sha256)?;
    fs::rename(temporary, dest).map_err(err)?;
    sync_directory(dest.parent().ok_or("目标目录缺失。")?)?;
    Ok(())
}
fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let temp = path.with_extension("json.tmp");
    let mut file = open_regular(&temp, true, true)?;
    file.write_all(&serde_json::to_vec(value).map_err(err)?)
        .map_err(err)?;
    file.sync_all().map_err(err)?;
    fs::rename(&temp, path).map_err(err)?;
    sync_directory(path.parent().ok_or("父目录缺失。")?)
}
fn sync_directory(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    let file = File::open(path).map_err(err)?;
    #[cfg(windows)]
    let file = {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        OpenOptions::new()
            .write(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
            .map_err(err)?
    };
    file.sync_all().map_err(err)
}
fn extract(archive: &Path, dest: &Path) -> Result<(), String> {
    let decoder = flate2::read::GzDecoder::new(File::open(archive).map_err(err)?);
    let mut archive = tar::Archive::new(decoder);
    for entry in archive.entries().map_err(err)? {
        let mut entry = entry.map_err(err)?;
        let path = entry.path().map_err(err)?.into_owned();
        safe_relative(path.to_str().ok_or("归档路径非 UTF-8。")?)?;
        let kind = entry.header().entry_type();
        if !kind.is_file() && !kind.is_dir() && !kind.is_symlink() && !kind.is_hard_link() {
            return Err("归档包含特殊文件。".into());
        }
        if let Some(link) = entry.link_name().map_err(err)? {
            let joined = if kind.is_symlink() {
                path.parent().ok_or("链接父目录缺失。")?.join(link.as_ref())
            } else {
                link.into_owned()
            };
            let mut depth = 0_i32;
            for c in joined.components() {
                match c {
                    Component::Normal(_) => depth += 1,
                    Component::ParentDir => {
                        depth -= 1;
                        if depth < 0 {
                            return Err("归档链接越界。".into());
                        }
                    }
                    Component::CurDir => {}
                    _ => return Err("归档链接为绝对路径。".into()),
                }
            }
        }
        if !entry.unpack_in(dest).map_err(err)? {
            return Err("归档路径越界。".into());
        }
    }
    Ok(())
}

fn extract_runtime_archive(archive: &Path, dest: &Path, artifact_path: &str) -> Result<(), String> {
    if Path::new(artifact_path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
    {
        extract_python_zip(archive, dest)
    } else {
        extract(archive, dest)
    }
}

fn extract_python_zip(archive: &Path, dest: &Path) -> Result<(), String> {
    const MAX_EXTRACTED_BYTES: u64 = 512 * 1024 * 1024;
    let mut zip = zip::ZipArchive::new(File::open(archive).map_err(err)?).map_err(err)?;
    if zip.len() > 4096 {
        return Err("Python 解释器归档条目过多。".into());
    }
    let mut extracted = 0u64;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(err)?;
        let name = entry.name().map_err(err)?.into_owned();
        safe_relative(&name)?;
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err("Python 解释器归档包含符号链接。".into());
        }
        let target = dest.join(name.replace('/', std::path::MAIN_SEPARATOR_STR));
        if entry.is_dir() {
            ensure_child_dir(dest, &target)?;
            continue;
        }
        extracted = extracted
            .checked_add(entry.size())
            .ok_or("Python 解释器归档展开大小溢出")?;
        if extracted > MAX_EXTRACTED_BYTES {
            return Err("Python 解释器归档展开后超过 512 MiB。".into());
        }
        ensure_child_dir(dest, target.parent().ok_or("解释器文件无父目录")?)?;
        let mut output = open_regular(&target, true, true)?;
        std::io::copy(&mut entry, &mut output).map_err(err)?;
        output.sync_all().map_err(err)?;
    }
    sync_directory(dest)
}

#[cfg(windows)]
fn configure_embeddable_python(runtime: &Path) -> Result<(), String> {
    let path_file = runtime.join("python312._pth");
    let mut content = fs::read_to_string(&path_file).map_err(err)?;
    if !content
        .lines()
        .any(|line| line.trim() == "Lib\\site-packages")
    {
        if let Some(index) = content.find("\n.\n") {
            let insert = index + 3;
            content.insert_str(insert, "Lib\\site-packages\n");
        } else {
            content.push_str("\nLib\\site-packages\n");
        }
    }
    content = content.replace("#import site", "import site");
    if !content.lines().any(|line| line.trim() == "import site") {
        content.push_str("import site\n");
    }
    let mut file = open_regular(&path_file, true, true)?;
    file.write_all(content.as_bytes()).map_err(err)?;
    file.sync_all().map_err(err)?;
    if !runtime.join("python.exe").is_file() {
        return Err("Windows 私有解释器缺少 python.exe。".into());
    }
    Ok(())
}

#[cfg(windows)]
fn install_wheel(archive: &Path, site_packages: &Path, data_root: &Path) -> Result<(), String> {
    const MAX_EXTRACTED_BYTES: u64 = 8 * 1024 * 1024 * 1024;
    let mut zip = zip::ZipArchive::new(File::open(archive).map_err(err)?).map_err(err)?;
    if zip.len() > 20_000 {
        return Err("wheel 归档条目过多。".into());
    }
    let mut extracted = 0u64;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(err)?;
        let name = entry.name().map_err(err)?.into_owned();
        safe_relative(&name)?;
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err("wheel 归档包含符号链接。".into());
        }
        if entry.is_dir() {
            continue;
        }
        let Some(relative) = wheel_site_path(&name)? else {
            continue;
        };
        extracted = extracted
            .checked_add(entry.size())
            .ok_or("wheel 展开大小溢出")?;
        if extracted > MAX_EXTRACTED_BYTES {
            return Err("wheel 展开后超过 8 GiB。".into());
        }
        let target = site_packages.join(relative);
        ensure_child_dir(data_root, target.parent().ok_or("wheel 文件无父目录")?)?;
        let mut output = open_regular(&target, true, true)?;
        std::io::copy(&mut entry, &mut output).map_err(err)?;
    }
    sync_directory(site_packages)
}

#[cfg(windows)]
fn wheel_site_path(name: &str) -> Result<Option<PathBuf>, String> {
    let parts = name.split('/').collect::<Vec<_>>();
    let Some(first) = parts.first() else {
        return Err("wheel 条目为空。".into());
    };
    if !first.ends_with(".data") {
        return Ok(Some(PathBuf::from(
            parts.join(std::path::MAIN_SEPARATOR_STR),
        )));
    }
    let kind = *parts.get(1).ok_or("wheel data 条目缺少类别")?;
    match kind {
        "purelib" | "platlib" => {
            let relative = &parts[2..];
            if relative.is_empty() {
                return Err("wheel data 库条目为空。".into());
            }
            Ok(Some(PathBuf::from(
                relative.join(std::path::MAIN_SEPARATOR_STR),
            )))
        }
        "data" | "scripts" | "headers" => Ok(None),
        _ => Err(format!("未知 wheel data 类别：{kind}")),
    }
}
fn inventory(root: &Path, path: &Path, out: &mut Vec<InventoryEntry>) -> Result<(), String> {
    for e in fs::read_dir(path).map_err(err)? {
        let e = e.map_err(err)?;
        let p = e.path();
        let metadata = fs::symlink_metadata(&p).map_err(err)?;
        if e.file_name() == "__pycache__" {
            continue;
        }
        if metadata.is_dir() {
            inventory(root, &p, out)?;
        } else if metadata.is_file() {
            let mut file = File::open(&p).map_err(err)?;
            let mut h = Sha256::new();
            std::io::copy(&mut file, &mut h).map_err(err)?;
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                metadata.permissions().mode()
            };
            #[cfg(not(unix))]
            let mode = 0;
            out.push(InventoryEntry {
                path: p
                    .strip_prefix(root)
                    .map_err(err)?
                    .to_string_lossy()
                    .replace('\\', "/"),
                sha256: format!("{:x}", h.finalize()),
                bytes: metadata.len(),
                link_target: None,
                mode,
            });
        } else if metadata.file_type().is_symlink() {
            if !p.canonicalize().map_err(err)?.starts_with(root) {
                return Err("私有运行时包含逃逸链接。".into());
            }
            let target = fs::read_link(&p)
                .map_err(err)?
                .to_string_lossy()
                .into_owned();
            out.push(InventoryEntry {
                path: p
                    .strip_prefix(root)
                    .map_err(err)?
                    .to_string_lossy()
                    .replace('\\', "/"),
                sha256: format!("{:x}", Sha256::digest(target.as_bytes())),
                bytes: metadata.len(),
                link_target: Some(target),
                mode: 0,
            });
        } else {
            return Err("私有运行时包含特殊文件。".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn external_process_paths_drop_the_windows_verbatim_prefix() {
        let canonical = Path::new(r"\\?\C:\ist-a\data\models\realesrgan-x4plus-windows-v1");
        assert_eq!(
            external_process_path(canonical).as_ref(),
            r"C:\ist-a\data\models\realesrgan-x4plus-windows-v1"
        );
        let unc = Path::new(r"\\?\UNC\server\share\models");
        assert_eq!(
            external_process_path(unc).as_ref(),
            r"\\server\share\models"
        );
        let plain = Path::new(r"C:\ist-a\models");
        assert_eq!(external_process_path(plain).as_ref(), r"C:\ist-a\models");
    }
    #[cfg(not(windows))]
    #[test]
    fn external_process_paths_are_unchanged_off_windows() {
        let path = Path::new("/tmp/models");
        assert_eq!(external_process_path(path).as_ref(), "/tmp/models");
    }
    #[cfg(windows)]
    fn seed_windows_validation_cache(installer: &Installation) {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let runtime_source = repo.join("target/windows-gate0/downloads");
        let model_source = repo.join("target/windows-gate0/model");
        let worker_source = repo.join(
            "resources/windows-foundation/z-image-diffusers/studio_zimage_cuda_worker-0.1.0-py3-none-any.whl",
        );
        ensure_child_dir(&installer.inner.paths.cache, &installer.cache()).unwrap();
        for artifact in installer.inner.runtime.downloads() {
            if artifact.url == "bundled:worker-wheel" {
                verify(&worker_source, artifact.bytes, &artifact.sha256).unwrap();
                fs::hard_link(&worker_source, installer.cache().join(&artifact.sha256)).unwrap();
                continue;
            }
            let source = runtime_source.join(&artifact.path);
            verify(&source, artifact.bytes, &artifact.sha256).unwrap();
            fs::hard_link(source, installer.cache().join(&artifact.sha256)).unwrap();
        }
        for artifact in installer.inner.model.downloads() {
            let source = model_source.join(&artifact.path);
            verify(&source, artifact.bytes, &artifact.sha256).unwrap();
            fs::hard_link(source, installer.cache().join(&artifact.sha256)).unwrap();
        }
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "explicit acceptance: installs the complete Windows product closure from the Gate 0 validation cache"]
    fn actual_windows_product_installs_from_the_validation_cache() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::resolve(Some(root.path())).unwrap();
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let manager = Installations::new(paths.clone()).unwrap();
        let installer = manager.for_id(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
        seed_windows_validation_cache(installer);

        let started = std::time::Instant::now();
        manager.start(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(600);
        loop {
            let snapshot = manager.snapshot(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
            if !matches!(
                snapshot.phase.as_str(),
                "preparing" | "downloading" | "verifying" | "installing_runtime" | "probing"
            ) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Windows product installation timed out in {}: {}",
                snapshot.phase,
                snapshot.error.as_deref().unwrap_or("no error")
            );
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        let snapshot = manager.snapshot(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
        assert_eq!(snapshot.phase, "ready", "{:?}", snapshot.error);
        assert!(snapshot.runtime_ready && snapshot.model_ready && snapshot.device_verified);

        let ready = installer.ready_paths().unwrap();
        assert_eq!(ready.worker_module, WINDOWS_ZIMAGE_WORKER_MODULE);
        assert_eq!(ready.worker_build_id, WINDOWS_ZIMAGE_WORKER_BUILD_ID);
        assert_eq!(ready.runtime_lock_id, installer.inner.runtime.id);
        assert_eq!(ready.model_snapshot_id, WINDOWS_ZIMAGE_INSTALL_ID);
        assert!(ready.worker_program.is_file());
        assert!(ready.model_path.join("model_index.json").is_file());
        let environment = installer.plan_environment().unwrap();
        assert!(!environment.components.is_empty());
        assert_eq!(
            environment.device.device_id,
            studio_core::generation::Z_IMAGE_WINDOWS_DEVICE
        );
        assert_eq!(
            environment.device.precision,
            "transformer+text_encoder=nf4;compute=bfloat16;vae=bfloat16"
        );

        let record: ReadyRecord = serde_json::from_slice(
            &fs::read(paths.data.join(installer.inner.profile.ready_relative)).unwrap(),
        )
        .unwrap();
        assert_eq!(record.probe["device"], "gpu");
        let protocol_output = repo.join("verification/windows-gate0/product-lifecycle-protocol");
        let protocol_report =
            repo.join("verification/windows-gate0/product-install-worker-protocol.json");
        let protocol = Command::new(&ready.worker_program)
            .arg("-I")
            .arg(repo.join("tools/zimage-cuda-worker/tests/run_real_protocol.py"))
            .arg("--python")
            .arg(&ready.worker_program)
            .arg("--supervisor")
            .arg(repo.join("target/debug/inference-supervisor.exe"))
            .arg("--model")
            .arg(&ready.model_path)
            .arg("--output")
            .arg(&protocol_output)
            .arg("--report")
            .arg(&protocol_report)
            .arg("--prompt")
            .arg("雨后的上海街角，一位撑伞的行人，写实摄影，真实自然光")
            .arg("--seed")
            .arg("20261101")
            .env("HF_HUB_OFFLINE", "1")
            .env("TRANSFORMERS_OFFLINE", "1")
            .env("HF_HUB_DISABLE_TELEMETRY", "1")
            .env("DO_NOT_TRACK", "1")
            .status()
            .unwrap();
        assert!(
            protocol.success(),
            "installed product worker protocol failed"
        );
        assert!(protocol_report.is_file());
        let library = Arc::new(Mutex::new(
            crate::adapters::library::Library::open(
                &paths.data,
                vec![paths.cache.clone(), paths.logs.clone(), paths.work.clone()],
            )
            .unwrap(),
        ));
        let submit_product_job = |suffix: &str| {
            let frozen = studio_core::generation::freeze_windows_z_image(
                studio_core::generation::GenerationInput {
                    request_id: studio_core::domain::RequestId::new(format!(
                        "request:windows-product:{suffix}"
                    ))
                    .unwrap(),
                    project_id: studio_core::domain::ProjectId::new("default").unwrap(),
                    idempotency_key: studio_core::domain::IdempotencyKey::new(format!(
                        "intent:windows-product:{suffix}"
                    ))
                    .unwrap(),
                    plan_id: studio_core::domain::PlanId::new(format!(
                        "plan:windows-product:{suffix}"
                    ))
                    .unwrap(),
                    prompt: "雨后的上海街角，一位撑伞的行人，写实摄影，真实自然光".into(),
                    width: 1024,
                    height: 1024,
                    count: 1,
                    seed: Some(20261101),
                },
                20261101,
                installer.plan_environment().unwrap(),
            )
            .unwrap();
            library
                .lock()
                .unwrap()
                .submit(frozen.request, frozen.plan)
                .unwrap()
                .job
        };
        let submitted = submit_product_job("primary");
        let controller = crate::adapters::generation::GenerationController::new(
            paths.clone(),
            library.clone(),
            repo.join("target/debug/inference-supervisor.exe"),
        );
        controller.start(submitted.clone(), ready.clone()).unwrap();
        let wait_controller = || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(900);
            while controller.is_busy() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "Windows product controller generation timed out"
                );
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        };
        wait_controller();
        let completed = library.lock().unwrap().job(&submitted.job_id).unwrap();
        assert_eq!(
            completed.state,
            studio_core::generation::AttemptState::Succeeded
        );
        let manifest = completed.manifest.as_ref().unwrap();
        assert_eq!(manifest.outputs.len(), 1);
        assert_eq!(
            manifest.outputs[0].data_type,
            studio_core::domain::AssetKind::ImageRgb
        );
        let asset_bytes = library
            .lock()
            .unwrap()
            .read_asset_png(&manifest.outputs[0].asset_id)
            .unwrap();
        let controller_png = repo.join("verification/windows-gate0/product-controller-primary.png");
        fs::write(&controller_png, &asset_bytes).unwrap();
        atomic_json(
            &repo.join("verification/windows-gate0/product-controller-generation.json"),
            &serde_json::json!({
                "schema_version": 1,
                "kind": "windows-product-controller-generation",
                "status": "passed",
                "job_id": submitted.job_id,
                "attempt_id": submitted.attempt.attempt_id,
                "asset_id": manifest.outputs[0].asset_id,
                "asset_sha256": manifest.outputs[0].content_hash,
                "asset_bytes": asset_bytes.len(),
                "output": "verification/windows-gate0/product-controller-primary.png",
            }),
        )
        .unwrap();

        let loading_cancel = submit_product_job("cancel-loading");
        controller
            .start(loading_cancel.clone(), ready.clone())
            .unwrap();
        let cancel_deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
        loop {
            let view = controller.snapshot();
            if view.stage == "loading" {
                controller.cancel(&loading_cancel.job_id).unwrap();
                break;
            }
            assert!(
                std::time::Instant::now() < cancel_deadline,
                "did not observe loading before cancel deadline: {}",
                view.stage
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        wait_controller();
        let loading_record = library.lock().unwrap().job(&loading_cancel.job_id).unwrap();
        assert_eq!(
            loading_record.state,
            studio_core::generation::AttemptState::Cancelled
        );
        assert!(
            loading_record
                .manifest
                .as_ref()
                .is_some_and(|manifest| manifest.outputs.is_empty())
        );

        let sampling_cancel = submit_product_job("cancel-sampling");
        controller
            .start(sampling_cancel.clone(), ready.clone())
            .unwrap();
        let sampling_deadline = std::time::Instant::now() + std::time::Duration::from_secs(900);
        loop {
            let view = controller.snapshot();
            if view.stage == "sampling" && view.step.unwrap_or(0) >= 1 {
                controller.cancel(&sampling_cancel.job_id).unwrap();
                break;
            }
            assert!(
                std::time::Instant::now() < sampling_deadline,
                "did not observe sampling before cancel deadline: {}",
                view.stage
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        wait_controller();
        let sampling_record = library
            .lock()
            .unwrap()
            .job(&sampling_cancel.job_id)
            .unwrap();
        assert_eq!(
            sampling_record.state,
            studio_core::generation::AttemptState::Cancelled
        );
        assert!(
            sampling_record
                .manifest
                .as_ref()
                .is_some_and(|manifest| manifest.outputs.is_empty())
        );

        let killed = submit_product_job("kill-supervisor");
        controller.start(killed.clone(), ready.clone()).unwrap();
        let killed_run = library
            .lock()
            .unwrap()
            .attempt_run_dir(&killed.attempt)
            .unwrap();
        let lifecycle = killed_run.join(inference_supervisor::LIFECYCLE_RECORD_NAME);
        let lifecycle_deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
        while !lifecycle.is_file() {
            assert!(
                std::time::Instant::now() < lifecycle_deadline,
                "supervisor lifecycle was not published"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let lifecycle_record: serde_json::Value =
            serde_json::from_slice(&fs::read(&lifecycle).unwrap()).unwrap();
        let supervisor_pid = lifecycle_record["supervisor_pid"].as_u64().unwrap();
        let supervisor_pid_text = supervisor_pid.to_string();
        let killed_status = Command::new("taskkill.exe")
            .args(["/PID", supervisor_pid_text.as_str(), "/F"])
            .status()
            .unwrap();
        assert!(killed_status.success());
        wait_controller();
        let killed_record = library.lock().unwrap().job(&killed.job_id).unwrap();
        assert_eq!(
            killed_record.state,
            studio_core::generation::AttemptState::Failed
        );
        assert!(
            killed_record
                .manifest
                .as_ref()
                .is_some_and(|manifest| manifest.outputs.is_empty())
        );

        let recovery = submit_product_job("recovery");
        controller.start(recovery.clone(), ready.clone()).unwrap();
        wait_controller();
        let recovery_record = library.lock().unwrap().job(&recovery.job_id).unwrap();
        assert_eq!(
            recovery_record.state,
            studio_core::generation::AttemptState::Succeeded
        );
        let recovery_manifest = recovery_record.manifest.as_ref().unwrap();
        let recovery_bytes = library
            .lock()
            .unwrap()
            .read_asset_png(&recovery_manifest.outputs[0].asset_id)
            .unwrap();
        fs::write(
            repo.join("verification/windows-gate0/product-recovery.png"),
            &recovery_bytes,
        )
        .unwrap();
        atomic_json(
            &repo.join("verification/windows-gate0/product-cancel-recovery.json"),
            &serde_json::json!({
                "schema_version": 1,
                "kind": "windows-product-cancel-recovery",
                "status": "passed",
                "loading_cancel": {
                    "job_id": loading_cancel.job_id,
                    "state": loading_record.state,
                    "outputs": loading_record.manifest.as_ref().map_or(0, |manifest| manifest.outputs.len()),
                },
                "sampling_cancel": {
                    "job_id": sampling_cancel.job_id,
                    "state": sampling_record.state,
                    "outputs": sampling_record.manifest.as_ref().map_or(0, |manifest| manifest.outputs.len()),
                },
                "supervisor_kill": {
                    "job_id": killed.job_id,
                    "state": killed_record.state,
                    "outputs": killed_record.manifest.as_ref().map_or(0, |manifest| manifest.outputs.len()),
                    "pid": supervisor_pid,
                },
                "recovery": {
                    "job_id": recovery.job_id,
                    "state": recovery_record.state,
                    "asset_id": recovery_manifest.outputs[0].asset_id,
                    "asset_sha256": recovery_manifest.outputs[0].content_hash,
                    "output": "verification/windows-gate0/product-recovery.png",
                }
            }),
        )
        .unwrap();
        controller.shutdown();
        let worker_wheel_sha256 = installer
            .inner
            .runtime
            .wheels
            .iter()
            .find(|artifact| artifact.url == "bundled:worker-wheel")
            .unwrap()
            .sha256
            .clone();
        atomic_json(
            &repo.join("verification/windows-gate0/product-install.json"),
            &serde_json::json!({
                "schema_version": 1,
                "kind": "windows-product-install",
                "status": "passed",
                "network_denied": std::env::var("STUDIO_ACCEPTANCE_NETWORK_DENIED").as_deref() == Ok("1"),
                "elapsed_seconds": started.elapsed().as_secs_f64(),
                "runtime_lock_id": ready.runtime_lock_id,
                "worker_build_id": ready.worker_build_id,
                "worker_wheel_sha256": worker_wheel_sha256,
                "model_snapshot_id": ready.model_snapshot_id,
                "model_files": installer.inner.model.files.len(),
                "model_component_bytes": installer.inner.model.component_bytes,
                "runtime_inventory_entries": record.runtime_files.len(),
                "generation_components": environment.components.len(),
                "probe": record.probe,
                "worker_protocol_report": "verification/windows-gate0/product-install-worker-protocol.json",
                "worker_protocol_output": "verification/windows-gate0/product-lifecycle-protocol",
                "controller_generation_report": "verification/windows-gate0/product-controller-generation.json",
                "controller_generation_output": "verification/windows-gate0/product-controller-primary.png",
                "cancel_recovery_report": "verification/windows-gate0/product-cancel-recovery.json",
                "recovery_output": "verification/windows-gate0/product-recovery.png",
            }),
        )
        .unwrap();
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "explicit acceptance: cancels an empty-cache ModelScope install and resumes it"]
    fn actual_windows_product_installs_from_an_empty_cache_and_resumes_after_cancel() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::resolve(Some(root.path())).unwrap();
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let manager = Installations::new(paths.clone()).unwrap();
        let installer = manager.for_id(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
        let cache_initial_entries = fs::read_dir(installer.cache())
            .map(|entries| entries.count())
            .unwrap_or(0);
        assert_eq!(
            cache_initial_entries, 0,
            "acceptance requires an empty cache"
        );

        let model_downloads = installer.inner.model.downloads();
        assert_eq!(model_downloads.len(), 20);
        assert!(model_downloads.iter().all(|artifact| {
            reqwest::Url::parse(&artifact.url)
                .is_ok_and(|url| url.host_str() == Some("www.modelscope.cn"))
        }));
        let runtime_downloads = installer.inner.runtime.downloads();
        let model_download_bytes = model_downloads
            .iter()
            .map(|artifact| artifact.bytes)
            .sum::<u64>();
        let runtime_download_bytes = runtime_downloads
            .iter()
            .map(|artifact| artifact.bytes)
            .sum::<u64>();
        let mut runtime_hosts = std::collections::BTreeSet::new();
        for artifact in runtime_downloads
            .iter()
            .filter(|artifact| artifact.url.starts_with("https://"))
        {
            runtime_hosts.insert(
                reqwest::Url::parse(&artifact.url)
                    .unwrap()
                    .host_str()
                    .unwrap()
                    .to_string(),
            );
        }

        let started_at_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let started = std::time::Instant::now();
        let deadline = started + std::time::Duration::from_secs(3 * 60 * 60);
        let interruption_threshold = 256 * 1024 * 1024;
        let mut next_progress = started + std::time::Duration::from_secs(10);
        let mut phase_history = vec![
            manager
                .snapshot(WINDOWS_ZIMAGE_INSTALL_ID)
                .unwrap()
                .phase
                .clone(),
        ];
        let mut received_peak = 0;
        let mut timed_out = false;
        let mut interrupted_received = None;
        manager.start(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
        loop {
            let snapshot = manager.snapshot(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
            received_peak = received_peak.max(snapshot.received_bytes);
            if phase_history.last() != Some(&snapshot.phase) {
                phase_history.push(snapshot.phase.clone());
            }
            if matches!(
                snapshot.phase.as_str(),
                "preparing" | "downloading" | "verifying" | "installing_runtime" | "probing"
            ) {
                if interrupted_received.is_none()
                    && snapshot.received_bytes >= interruption_threshold
                {
                    interrupted_received = Some(snapshot.received_bytes);
                    manager.cancel(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
                }
                if std::time::Instant::now() >= deadline {
                    timed_out = true;
                    manager.cancel(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
                } else {
                    if std::time::Instant::now() >= next_progress {
                        eprintln!(
                            "Windows interrupted install: phase={} received={}/{} bytes elapsed={}s",
                            snapshot.phase,
                            snapshot.received_bytes,
                            snapshot.total_bytes,
                            started.elapsed().as_secs()
                        );
                        next_progress =
                            std::time::Instant::now() + std::time::Duration::from_secs(10);
                    }
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    continue;
                }
            }
            break;
        }

        let interrupted_snapshot = manager.snapshot(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
        let interrupted_received = interrupted_received.unwrap_or(received_peak);
        let cache_entries_after_cancel = fs::read_dir(installer.cache())
            .map(|entries| entries.count())
            .unwrap_or(0);
        let (segment_files_after_cancel, segment_bytes_after_cancel) =
            fs::read_dir(installer.cache())
                .map(|entries| {
                    entries
                        .filter_map(Result::ok)
                        .filter_map(|entry| {
                            let name = entry.file_name().to_string_lossy().into_owned();
                            (name.contains(".segment-") && !name.ends_with(".json"))
                                .then(|| entry.metadata().ok())
                                .flatten()
                        })
                        .fold((0_usize, 0_u64), |(count, bytes), metadata| {
                            (count + 1, bytes + metadata.len())
                        })
                })
                .unwrap_or((0, 0));
        let interrupted_elapsed_seconds = started.elapsed().as_secs_f64();
        let resumed = !timed_out
            && interrupted_snapshot.phase == "cancelled"
            && segment_files_after_cancel > 0;
        let mut resume_phase_history = vec![interrupted_snapshot.phase.clone()];
        let resume_started = std::time::Instant::now();
        let mut resume_next_progress = resume_started + std::time::Duration::from_secs(10);
        if resumed {
            manager.start(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
            loop {
                let snapshot = manager.snapshot(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
                received_peak = received_peak.max(snapshot.received_bytes);
                if resume_phase_history.last() != Some(&snapshot.phase) {
                    resume_phase_history.push(snapshot.phase.clone());
                }
                if matches!(
                    snapshot.phase.as_str(),
                    "preparing" | "downloading" | "verifying" | "installing_runtime" | "probing"
                ) {
                    if std::time::Instant::now() >= deadline {
                        timed_out = true;
                        manager.cancel(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
                    } else {
                        if std::time::Instant::now() >= resume_next_progress {
                            eprintln!(
                                "Windows resumed install: phase={} received={}/{} bytes elapsed={}s",
                                snapshot.phase,
                                snapshot.received_bytes,
                                snapshot.total_bytes,
                                started.elapsed().as_secs()
                            );
                            resume_next_progress =
                                std::time::Instant::now() + std::time::Duration::from_secs(10);
                        }
                        std::thread::sleep(std::time::Duration::from_secs(1));
                        continue;
                    }
                }
                break;
            }
        }

        let snapshot = manager.snapshot(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
        let passed = !timed_out && snapshot.phase == "ready";
        let ready = passed.then(|| installer.ready_paths().unwrap());
        let record = passed.then(|| {
            serde_json::from_slice::<ReadyRecord>(
                &fs::read(paths.data.join(installer.inner.profile.ready_relative)).unwrap(),
            )
            .unwrap()
        });
        atomic_json(
            &repo.join("verification/windows-gate0/product-interrupted-resume.json"),
            &serde_json::json!({
                "schema_version": 1,
                "kind": "windows-product-interrupted-resume",
                "status": if passed { "passed" } else { "failed" },
                "started_at_unix": started_at_unix,
                "elapsed_seconds": started.elapsed().as_secs_f64(),
                "timed_out": timed_out,
                "network_denied": false,
                "validation_cache_seeded": false,
                "cache_initial_entries": cache_initial_entries,
                "model_download_source": "modelscope",
                "model_upstream_source": "huggingface",
                "model_files": model_downloads.len(),
                "model_download_bytes": model_download_bytes,
                "model_component_bytes": installer.inner.model.component_bytes,
                "runtime_artifacts": runtime_downloads.len(),
                "runtime_download_bytes": runtime_download_bytes,
                "runtime_hosts": runtime_hosts,
                "total_download_bytes": model_download_bytes + runtime_download_bytes,
                "received_peak_bytes": received_peak,
                "interruption": {
                    "status": if resumed { "cancelled_with_resume_state" } else { "failed" },
                    "threshold_bytes": interruption_threshold,
                    "observed_received_bytes": interrupted_received,
                    "elapsed_seconds": interrupted_elapsed_seconds,
                    "cache_entries_after_cancel": cache_entries_after_cancel,
                    "segment_files_after_cancel": segment_files_after_cancel,
                    "segment_bytes_after_cancel": segment_bytes_after_cancel,
                    "phase_history": phase_history,
                    "phase": interrupted_snapshot.phase,
                    "error": interrupted_snapshot.error,
                },
                "resume": {
                    "status": if passed { "passed" } else { "failed" },
                    "elapsed_seconds": resume_started.elapsed().as_secs_f64(),
                    "phase_history": resume_phase_history,
                    "phase": snapshot.phase,
                    "error": snapshot.error,
                },
                "runtime_lock_id": ready.as_ref().map(|ready| ready.runtime_lock_id.clone()),
                "worker_build_id": ready.as_ref().map(|ready| ready.worker_build_id.clone()),
                "model_snapshot_id": ready.as_ref().map(|ready| ready.model_snapshot_id.clone()),
                "runtime_inventory_entries": record.as_ref().map(|record| record.runtime_files.len()),
                "probe": record.as_ref().map(|record| record.probe.clone()),
            }),
        )
        .unwrap();

        assert!(!timed_out, "Windows empty-cache installation timed out");
        assert_eq!(snapshot.phase, "ready", "{:?}", snapshot.error);
        assert!(snapshot.runtime_ready && snapshot.model_ready && snapshot.device_verified);
        let ready = ready.unwrap();
        assert_eq!(ready.worker_build_id, WINDOWS_ZIMAGE_WORKER_BUILD_ID);
        assert_eq!(ready.runtime_lock_id, installer.inner.runtime.id);
        assert_eq!(ready.model_snapshot_id, WINDOWS_ZIMAGE_INSTALL_ID);
        assert!(ready.worker_program.is_file());
        assert!(ready.model_path.join("model_index.json").is_file());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "explicit acceptance: cancels runtime preparation and the probe, then retries from the verified cache"]
    fn actual_windows_product_cancels_runtime_preparation_and_probe_and_retries() {
        fn wait_for_phase(
            manager: &Installations,
            target: &str,
            deadline: std::time::Instant,
            history: &mut Vec<String>,
        ) -> std::time::Instant {
            loop {
                let snapshot = manager.snapshot(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
                if history.last() != Some(&snapshot.phase) {
                    history.push(snapshot.phase.clone());
                }
                if snapshot.phase == target {
                    return std::time::Instant::now();
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "installation did not reach {target}: phase={} error={:?}",
                    snapshot.phase,
                    snapshot.error
                );
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        fn wait_terminal(
            manager: &Installations,
            deadline: std::time::Instant,
            history: &mut Vec<String>,
        ) -> InstallationSnapshot {
            loop {
                let snapshot = manager.snapshot(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
                if history.last() != Some(&snapshot.phase) {
                    history.push(snapshot.phase.clone());
                }
                if !matches!(
                    snapshot.phase.as_str(),
                    "preparing" | "downloading" | "verifying" | "installing_runtime" | "probing"
                ) {
                    return snapshot;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "installation did not stop: phase={} error={:?}",
                    snapshot.phase,
                    snapshot.error
                );
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }

        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::resolve(Some(root.path())).unwrap();
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let manager = Installations::new(paths.clone()).unwrap();
        let installer = manager.for_id(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
        seed_windows_validation_cache(installer);
        let ready_record = paths.data.join(installer.inner.profile.ready_relative);
        let started_at_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let started = std::time::Instant::now();

        // Cancel while the private interpreter and wheels are being prepared.
        let mut runtime_phases = vec![
            manager
                .snapshot(WINDOWS_ZIMAGE_INSTALL_ID)
                .unwrap()
                .phase
                .clone(),
        ];
        manager.start(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
        let runtime_reached = wait_for_phase(
            &manager,
            "installing_runtime",
            std::time::Instant::now() + std::time::Duration::from_secs(900),
            &mut runtime_phases,
        );
        let runtime_cancel_requested_seconds =
            runtime_reached.duration_since(started).as_secs_f64();
        manager.cancel(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
        let cancelled_runtime = wait_terminal(
            &manager,
            std::time::Instant::now() + std::time::Duration::from_secs(60),
            &mut runtime_phases,
        );
        let runtime_cancel_seconds = started.elapsed().as_secs_f64();
        let activated_after_runtime_cancel = ready_record.exists();
        let busy_after_runtime_cancel = manager.is_busy();

        // Cancel again while the real GPU/NF4 probe is running.
        let mut probe_phases = vec![cancelled_runtime.phase.clone()];
        manager.start(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
        let probe_reached = wait_for_phase(
            &manager,
            "probing",
            std::time::Instant::now() + std::time::Duration::from_secs(900),
            &mut probe_phases,
        );
        let probe_cancel_requested_seconds = probe_reached.duration_since(started).as_secs_f64();
        manager.cancel(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
        let cancelled_probe = wait_terminal(
            &manager,
            std::time::Instant::now() + std::time::Duration::from_secs(120),
            &mut probe_phases,
        );
        let probe_cancel_seconds = started.elapsed().as_secs_f64();
        let activated_after_probe_cancel = ready_record.exists();
        let busy_after_probe_cancel = manager.is_busy();

        // A third start must activate only after the complete runtime is verified again.
        let mut retry_phases = vec![cancelled_probe.phase.clone()];
        manager.start(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
        let retry_started = std::time::Instant::now();
        let final_snapshot = loop {
            let snapshot = manager.snapshot(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
            if retry_phases.last() != Some(&snapshot.phase) {
                retry_phases.push(snapshot.phase.clone());
            }
            if !matches!(
                snapshot.phase.as_str(),
                "preparing" | "downloading" | "verifying" | "installing_runtime" | "probing"
            ) {
                break snapshot;
            }
            assert!(
                retry_started.elapsed() < std::time::Duration::from_secs(900),
                "retried installation did not finish: phase={} error={:?}",
                snapshot.phase,
                snapshot.error
            );
            std::thread::sleep(std::time::Duration::from_millis(250));
        };
        let retry_elapsed_seconds = retry_started.elapsed().as_secs_f64();
        let passed = final_snapshot.phase == "ready";
        let ready = passed.then(|| installer.ready_paths().unwrap());
        let record = passed.then(|| {
            serde_json::from_slice::<ReadyRecord>(&fs::read(ready_record.clone()).unwrap()).unwrap()
        });
        atomic_json(
            &repo.join("verification/windows-gate0/product-runtime-cancel-retry.json"),
            &serde_json::json!({
                "schema_version": 1,
                "kind": "windows-product-runtime-cancel-retry",
                "status": if passed { "passed" } else { "failed" },
                "started_at_unix": started_at_unix,
                "elapsed_seconds": started.elapsed().as_secs_f64(),
                "validation_cache_seeded": true,
                "cache_entries_at_start": fs::read_dir(installer.cache())
                    .map(|entries| entries.count())
                    .unwrap_or(0),
                "runtime_cancel": {
                    "phase_history": runtime_phases,
                    "cancel_requested_seconds": runtime_cancel_requested_seconds,
                    "cancelled_seconds": runtime_cancel_seconds,
                    "phase": cancelled_runtime.phase,
                    "error": cancelled_runtime.error,
                    "activated": activated_after_runtime_cancel,
                    "busy_after_cancel": busy_after_runtime_cancel,
                },
                "probe_cancel": {
                    "phase_history": probe_phases,
                    "cancel_requested_seconds": probe_cancel_requested_seconds,
                    "cancelled_seconds": probe_cancel_seconds,
                    "phase": cancelled_probe.phase,
                    "error": cancelled_probe.error,
                    "activated": activated_after_probe_cancel,
                    "busy_after_cancel": busy_after_probe_cancel,
                },
                "retry": {
                    "phase_history": retry_phases,
                    "elapsed_seconds": retry_elapsed_seconds,
                    "phase": final_snapshot.phase,
                    "error": final_snapshot.error,
                },
                "runtime_lock_id": ready.as_ref().map(|ready| ready.runtime_lock_id.clone()),
                "worker_build_id": ready.as_ref().map(|ready| ready.worker_build_id.clone()),
                "model_snapshot_id": ready.as_ref().map(|ready| ready.model_snapshot_id.clone()),
                "runtime_inventory_entries": record.as_ref().map(|record| record.runtime_files.len()),
                "probe": record.as_ref().map(|record| record.probe.clone()),
            }),
        )
        .unwrap();

        assert_eq!(
            cancelled_runtime.phase, "cancelled",
            "{:?}",
            cancelled_runtime.error
        );
        assert_eq!(
            cancelled_probe.phase, "cancelled",
            "{:?}",
            cancelled_probe.error
        );
        assert!(!activated_after_runtime_cancel);
        assert!(!activated_after_probe_cancel);
        assert!(!busy_after_runtime_cancel);
        assert!(!busy_after_probe_cancel);
        assert!(passed, "{:?}", final_snapshot.error);
        let ready = ready.unwrap();
        assert_eq!(ready.worker_build_id, WINDOWS_ZIMAGE_WORKER_BUILD_ID);
        assert_eq!(ready.runtime_lock_id, installer.inner.runtime.id);
        assert_eq!(ready.model_snapshot_id, WINDOWS_ZIMAGE_INSTALL_ID);
        assert!(ready.worker_program.is_file());
        assert!(ready.model_path.join("model_index.json").is_file());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "explicit acceptance: activation rejects corrupt and missing model components"]
    fn actual_windows_product_rejects_corrupt_and_missing_model_components() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::resolve(Some(root.path())).unwrap();
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let manager = Installations::new(paths.clone()).unwrap();
        let installer = manager.for_id(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
        seed_windows_validation_cache(installer);
        let ready_record = paths.data.join(installer.inner.profile.ready_relative);
        let started_at_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let started = std::time::Instant::now();
        let mut install_phase_history = vec![
            manager
                .snapshot(WINDOWS_ZIMAGE_INSTALL_ID)
                .unwrap()
                .phase
                .clone(),
        ];
        manager.start(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(900);
        loop {
            let snapshot = manager.snapshot(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
            if install_phase_history.last() != Some(&snapshot.phase) {
                install_phase_history.push(snapshot.phase.clone());
            }
            if !matches!(
                snapshot.phase.as_str(),
                "preparing" | "downloading" | "verifying" | "installing_runtime" | "probing"
            ) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "installation timed out in {}: {}",
                snapshot.phase,
                snapshot.error.as_deref().unwrap_or("no error")
            );
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        let installed = manager.snapshot(WINDOWS_ZIMAGE_INSTALL_ID).unwrap();
        assert_eq!(installed.phase, "ready", "{:?}", installed.error);
        assert!(installer.ready_paths().is_ok());
        let install_elapsed_seconds = started.elapsed().as_secs_f64();
        let cache_entries_before = fs::read_dir(installer.cache())
            .map(|entries| entries.count())
            .unwrap_or(0);
        let scheduler = installer
            .model_dir()
            .join("scheduler/scheduler_config.json");
        let model_index = installer.model_dir().join("model_index.json");
        let original_scheduler = fs::read(&scheduler).unwrap();
        assert!(!original_scheduler.is_empty());

        // Same-length corruption is reported as a digest failure and never silently replaced.
        fs::write(&scheduler, vec![b'x'; original_scheduler.len()]).unwrap();
        let corrupt_error = installer.ready_paths().unwrap_err();
        let corrupt_phase = manager.snapshot(WINDOWS_ZIMAGE_INSTALL_ID).unwrap().phase;
        let cache_entries_after_corrupt = fs::read_dir(installer.cache())
            .map(|entries| entries.count())
            .unwrap_or(0);
        let corrupt_file_still_present = scheduler.is_file();

        // Restoring the exact bytes recovers readiness without reinstalling or downloading.
        fs::write(&scheduler, &original_scheduler).unwrap();
        let recovered = installer.ready_paths().is_ok();

        // A missing component keeps the installation unavailable and planning never re-fetches it.
        fs::remove_file(&model_index).unwrap();
        let missing_error = installer.ready_paths().unwrap_err();
        let missing_phase = manager.snapshot(WINDOWS_ZIMAGE_INSTALL_ID).unwrap().phase;
        let cache_entries_after_missing = fs::read_dir(installer.cache())
            .map(|entries| entries.count())
            .unwrap_or(0);
        let model_files_present = installer
            .inner
            .model
            .files
            .iter()
            .filter(|file| installer.model_dir().join(&file.path).is_file())
            .count();
        let missing_still_absent = !model_index.exists();

        atomic_json(
            &repo.join("verification/windows-gate0/product-activation-failures.json"),
            &serde_json::json!({
                "schema_version": 1,
                "kind": "windows-product-activation-failures",
                "status": "passed",
                "started_at_unix": started_at_unix,
                "install_elapsed_seconds": install_elapsed_seconds,
                "validation_cache_seeded": true,
                "install_phase_history": install_phase_history,
                "model_files": installer.inner.model.files.len(),
                "cache_entries_before_checks": cache_entries_before,
                "corrupt_component": {
                    "component": "scheduler/scheduler_config.json",
                    "same_length_corruption": true,
                    "error": corrupt_error,
                    "phase": corrupt_phase,
                    "cache_entries": cache_entries_after_corrupt,
                    "file_still_present": corrupt_file_still_present,
                    "recovered_after_exact_restore": recovered,
                },
                "missing_component": {
                    "component": "model_index.json",
                    "error": missing_error,
                    "phase": missing_phase,
                    "cache_entries": cache_entries_after_missing,
                    "model_files_present": model_files_present,
                    "still_absent": missing_still_absent,
                },
            }),
        )
        .unwrap();

        assert!(
            corrupt_error.contains("SHA-256 校验失败"),
            "unexpected corrupt error: {corrupt_error}"
        );
        assert_eq!(corrupt_phase, "unavailable");
        assert_eq!(cache_entries_after_corrupt, cache_entries_before);
        assert!(corrupt_file_still_present);
        assert!(recovered);
        assert!(
            missing_error.contains("文件缺失"),
            "unexpected missing error: {missing_error}"
        );
        assert_eq!(missing_phase, "unavailable");
        assert_eq!(cache_entries_after_missing, cache_entries_before);
        assert_eq!(model_files_present, installer.inner.model.files.len() - 1);
        assert!(missing_still_absent);
        assert!(ready_record.is_file());
    }

    #[test]
    #[ignore = "explicit acceptance: requires all fixed runtime artifacts in validation cache"]
    fn actual_private_runtime_installs_offline_without_activating_missing_model() {
        let (_temp, i) = fixture();
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let source = repo.join("target/first-generation-validation/cache/downloads");
        for a in i
            .inner
            .runtime
            .wheels
            .iter()
            .chain(std::iter::once(i.inner.runtime.interpreter().unwrap()))
        {
            let from = source.join(&a.sha256);
            verify(&from, a.bytes, &a.sha256).unwrap();
            fs::hard_link(from, i.cache().join(&a.sha256)).unwrap();
        }
        let started = std::time::Instant::now();
        let record = i.prepare_runtime().unwrap();
        assert_eq!(record.probe["device"], "gpu");
        assert!(!record.runtime_files.is_empty());
        for entry in &record.runtime_files {
            inventory_metadata_matches(&i.runtime_dir(), entry).unwrap();
        }
        assert!(!i.inner.paths.data.join("installation-ready.json").exists());
        assert!(!i.snapshot().model_ready);
        assert!(i.ready_paths().is_err());
        atomic_json(&repo.join("verification/first-generation-runtime-install.json"),&serde_json::json!({"kind":"production-installer-offline-runtime-acceptance","is_full_model_inference":false,"installation_activated":false,"network_denied":std::env::var("STUDIO_ACCEPTANCE_NETWORK_DENIED").as_deref()==Ok("1"),"elapsed_seconds":started.elapsed().as_secs_f64(),"runtime_lock_id":record.runtime_lock_id,"model_snapshot_id":record.model_snapshot_id,"worker_wheel_sha256":i.inner.runtime.wheels.iter().find(|a|a.url=="bundled:worker-wheel").unwrap().sha256,"inventory_entries":record.runtime_files.len(),"probe":record.probe})).unwrap();
    }

    #[test]
    #[ignore = "explicit acceptance: requires the pinned MFLUX wheel cache"]
    fn klein_private_runtime_installs_offline_without_model_weights() {
        let (_temp, i) = fixture_for(Installation::new_klein);
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let source = repo.join("target/model-usability-offline-cache-20261031/cache/downloads");
        for artifact in i
            .inner
            .runtime
            .wheels
            .iter()
            .filter(|wheel| wheel.url != "bundled:worker-wheel")
            .chain(std::iter::once(i.inner.runtime.interpreter().unwrap()))
        {
            let from = source.join(&artifact.sha256);
            verify(&from, artifact.bytes, &artifact.sha256).unwrap();
            fs::hard_link(from, i.cache().join(&artifact.sha256)).unwrap();
        }
        let worker = i
            .inner
            .runtime
            .wheels
            .iter()
            .find(|wheel| wheel.url == "bundled:worker-wheel")
            .unwrap();
        i.download(worker).unwrap();
        let started = std::time::Instant::now();
        let record = i.prepare_runtime().unwrap();
        assert_eq!(record.probe["identity"]["build_id"], KLEIN_WORKER_BUILD_ID);
        assert_eq!(record.probe["device"], "gpu");
        assert!(!i.snapshot().model_ready);
        assert!(i.ready_paths().is_err());
        atomic_json(&repo.join("verification/klein-runtime-install.json"),&serde_json::json!({"kind":"production-installer-offline-runtime-acceptance","is_full_model_inference":false,"installation_activated":false,"elapsed_seconds":started.elapsed().as_secs_f64(),"runtime_lock_id":record.runtime_lock_id,"model_snapshot_id":record.model_snapshot_id,"worker_wheel_sha256":i.inner.runtime.wheels.iter().find(|a|a.url=="bundled:worker-wheel").unwrap().sha256,"inventory_entries":record.runtime_files.len(),"probe":record.probe})).unwrap();
    }
    fn fixture() -> (tempfile::TempDir, Installation) {
        fixture_for(Installation::new)
    }
    fn fixture_for(
        create: impl FnOnce(AppPaths) -> Result<Installation, String>,
    ) -> (tempfile::TempDir, Installation) {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::resolve(Some(temp.path())).unwrap();
        let installer = create(paths).unwrap();
        ensure_child_dir(&installer.inner.paths.cache, &installer.cache()).unwrap();
        (temp, installer)
    }
    const TEST_LICENSES: &[(&str, &[u8])] = &[
        ("LICENSE-A.txt", b"license A text\n"),
        ("nested/LICENSE-B.txt", b"license B text\n"),
    ];
    fn license_profile() -> InstallProfile {
        InstallProfile {
            install_id: "license-test-install-v1",
            bundle_licenses: TEST_LICENSES,
            ..Z_PROFILE
        }
    }
    #[test]
    fn bundled_license_texts_are_archived_at_their_fixed_identity() {
        let profile = license_profile();
        let (_temp, install) = fixture_for(|paths| Installation::new_for(paths, profile));
        let model_bytes = install
            .inner
            .model
            .downloads()
            .iter()
            .map(|artifact| artifact.bytes)
            .sum::<u64>();
        let runtime_bytes = install
            .inner
            .runtime
            .downloads()
            .iter()
            .map(|artifact| artifact.bytes)
            .sum::<u64>();
        let license_bytes = TEST_LICENSES
            .iter()
            .map(|(_, bytes)| bytes.len() as u64)
            .sum::<u64>();
        assert_eq!(
            install.snapshot().required_bytes,
            model_bytes * 2 + runtime_bytes * 7 + license_bytes + RESERVE
        );
        let records = install.install_licenses().unwrap();
        assert_eq!(records, license_records(profile));
        let root = install
            .inner
            .paths
            .data
            .join("licenses")
            .join(profile.install_id);
        assert_eq!(
            fs::read(root.join("LICENSE-A.txt")).unwrap(),
            b"license A text\n"
        );
        assert_eq!(
            fs::read(root.join("nested/LICENSE-B.txt")).unwrap(),
            b"license B text\n"
        );
        for record in &records {
            verify(&root.join(&record.path), record.bytes, &record.sha256).unwrap();
        }
        assert_eq!(install.install_licenses().unwrap(), records);
        licenses_healthy(&install.inner.paths.data, profile).unwrap();
        fs::write(root.join("LICENSE-A.txt"), b"tampered").unwrap();
        assert!(licenses_healthy(&install.inner.paths.data, profile).is_err());
        assert!(
            verify(
                &root.join("LICENSE-A.txt"),
                records[0].bytes,
                &records[0].sha256
            )
            .is_err()
        );
        assert!(licenses_match_profile(profile, &records));
    }
    #[test]
    fn license_archive_failure_cannot_activate_silently() {
        let profile = license_profile();
        let (_temp, install) = fixture_for(|paths| Installation::new_for(paths, profile));
        let licenses = install.inner.paths.data.join("licenses");
        fs::write(&licenses, b"blocked by a file").unwrap();
        let error = install.install_licenses().unwrap_err();
        assert!(error.contains("管理目录"), "{error}");
    }
    #[test]
    fn runtime_activation_reuse_requires_identity_wheels_and_unchanged_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtimes/windows-v2");
        fs::create_dir_all(runtime.join("Lib/site-packages/worker")).unwrap();
        fs::write(runtime.join("python.exe"), b"interpreter").unwrap();
        fs::write(runtime.join("Lib/site-packages/worker/__init__.py"), b"").unwrap();
        // The installer canonicalizes its data root, so the activation checks compare
        // against canonical paths exactly like production does.
        let runtime = runtime.canonicalize().unwrap();
        let manifest = "a".repeat(64);
        let wheels = vec![WorkerWheelRecord {
            path: "studio_diffusers_worker-0.1.0-py3-none-any.whl".into(),
            bytes: 4,
            sha256: "c".repeat(64),
        }];
        assert!(!runtime_activation_matches(
            &runtime,
            "windows-v2",
            &manifest,
            &wheels
        ));
        write_runtime_activation(&runtime, "windows-v2", &manifest, &wheels).unwrap();
        assert!(runtime_activation_matches(
            &runtime,
            "windows-v2",
            &manifest,
            &wheels
        ));
        assert!(!runtime_activation_matches(
            &runtime,
            "other-runtime",
            &manifest,
            &wheels
        ));
        assert!(!runtime_activation_matches(
            &runtime,
            "windows-v2",
            &"b".repeat(64),
            &wheels
        ));
        // A future app build with different embedded worker wheels must not reuse
        // the old runtime; the staging rebuild installs the new wheels.
        let newer_wheels = vec![WorkerWheelRecord {
            path: "studio_diffusers_worker-0.1.0-py3-none-any.whl".into(),
            bytes: 4,
            sha256: "d".repeat(64),
        }];
        assert!(!runtime_activation_matches(
            &runtime,
            "windows-v2",
            &manifest,
            &newer_wheels
        ));
        assert!(!runtime_activation_matches(
            &runtime,
            "windows-v2",
            &manifest,
            &[]
        ));
        fs::write(runtime.join("python.exe"), b"interpreter-changed").unwrap();
        assert!(!runtime_activation_matches(
            &runtime,
            "windows-v2",
            &manifest,
            &wheels
        ));
    }
    #[test]
    fn legacy_profiles_have_no_worker_wheel_identity() {
        assert!(worker_wheels_match(Z_PROFILE, &[]));
        assert!(!worker_wheels_match(
            Z_PROFILE,
            &[WorkerWheelRecord {
                path: "studio_diffusers_worker-0.1.0-py3-none-any.whl".into(),
                bytes: 1,
                sha256: "a".repeat(64),
            }]
        ));
    }
    #[test]
    fn staged_runtime_activation_swaps_the_complete_tree() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtimes/v2");
        let staging = temp.path().join("runtimes/.v2.staging-1");
        fs::create_dir_all(runtime.join("Lib")).unwrap();
        fs::write(runtime.join("stale.txt"), b"stale").unwrap();
        fs::create_dir_all(staging.join("Lib")).unwrap();
        fs::write(staging.join("python.exe"), b"new").unwrap();
        activate_staged_runtime(&staging, &runtime).unwrap();
        assert!(!runtime.join("stale.txt").exists());
        assert_eq!(fs::read(runtime.join("python.exe")).unwrap(), b"new");
        assert!(!staging.exists());
    }
    fn staged_profile() -> InstallProfile {
        InstallProfile {
            install_id: "staged-test-install-v1",
            prepare_with_model: true,
            prepared_cache_reserve: 123_456,
            ..Z_PROFILE
        }
    }
    fn memory_facts(available_physical_gib: u64, available_commit_gib: u64) -> MemoryFacts {
        MemoryFacts {
            available_physical_bytes: available_physical_gib * GIB_BYTES,
            available_commit_bytes: available_commit_gib * GIB_BYTES,
        }
    }
    fn write_prepared_cache(dir: &Path) -> (Vec<PreparedFileRecord>, String) {
        for (relative, bytes) in [
            ("text_encoder/config.json", &b"{}"[..]),
            ("text_encoder/model.safetensors", &b"encoder-weights"[..]),
            ("transformer/config.json", &b"{}"[..]),
            ("transformer/model.safetensors", &b"dit-weights"[..]),
        ] {
            let target = dir.join(relative);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(&target, bytes).unwrap();
        }
        let files = prepared_cache_inventory(dir).unwrap();
        let digest = prepared_inventory_digest(&files).unwrap();
        (files, digest)
    }
    fn staged_ready_record(
        install: &Installation,
        profile: InstallProfile,
        model_id: &str,
    ) -> ReadyRecord {
        ReadyRecord {
            runtime_lock_id: install.inner.runtime.id.clone(),
            model_snapshot_id: model_id.into(),
            runtime_manifest_sha256: manifest_digest(profile.runtime_manifest),
            model_manifest_sha256: manifest_digest(profile.model_manifest),
            licenses: vec![],
            prepared_dir: None,
            prepared_files: Vec::new(),
            prepared_digest: None,
            worker_wheels: bundled_worker_wheel_records(profile),
            prepare_preflight: None,
            runtime_files: vec![],
            probe: serde_json::Value::Null,
        }
    }
    #[test]
    fn staged_prepare_command_matches_the_worker_contract() {
        let args = prepare_worker_args(
            staged_profile(),
            "model-v1",
            Path::new("C:/data/models/model-v1"),
            Path::new("C:/data/models/model-v1.prepared"),
            false,
        )
        .unwrap();
        assert_eq!(
            args,
            vec![
                "-I",
                "-m",
                WORKER_MODULE,
                "--model-snapshot-id",
                "model-v1",
                "--prepare",
                "--model-dir",
                "C:/data/models/model-v1",
                "--prepared-dir",
                "C:/data/models/model-v1.prepared",
            ]
        );
        let rebuild = prepare_worker_args(
            staged_profile(),
            "model-v1",
            Path::new("C:/data/models/model-v1"),
            Path::new("C:/data/models/model-v1.prepared"),
            true,
        )
        .unwrap();
        assert_eq!(
            rebuild.last().map(String::as_str),
            Some("--no-reuse-prepared")
        );
    }
    #[test]
    fn staged_prepared_cache_is_digest_bound_and_space_reserved() {
        let profile = staged_profile();
        let (_temp, install) = fixture_for(|paths| Installation::new_for(paths, profile));
        let model_bytes = install
            .inner
            .model
            .downloads()
            .iter()
            .map(|artifact| artifact.bytes)
            .sum::<u64>();
        let runtime_bytes = install
            .inner
            .runtime
            .downloads()
            .iter()
            .map(|artifact| artifact.bytes)
            .sum::<u64>();
        assert_eq!(
            install.snapshot().required_bytes,
            model_bytes * 2 + runtime_bytes * 7 + 123_456 + RESERVE
        );
        let model_id = install.inner.model.id.clone();
        let prepared = prepared_cache_dir(&install.inner.paths.data, &model_id);
        let mut record = staged_ready_record(&install, profile, &model_id);
        // An old/absent record is stale: metadata and digest checks fail.
        assert!(
            prepared_cache_matches(&install.inner.paths.data, &model_id, profile, &record).is_err()
        );
        assert!(!prepared_cache_metadata_matches(
            &install.inner.paths.data,
            &model_id,
            profile,
            &record
        ));
        record.prepared_dir = Some(prepared.to_string_lossy().into_owned());
        assert!(
            prepared_cache_matches(&install.inner.paths.data, &model_id, profile, &record).is_err()
        );
        let (files, digest) = write_prepared_cache(&prepared);
        record.prepared_files = files.clone();
        record.prepared_digest = Some(digest.clone());
        prepared_cache_matches(&install.inner.paths.data, &model_id, profile, &record).unwrap();
        assert!(prepared_cache_metadata_matches(
            &install.inner.paths.data,
            &model_id,
            profile,
            &record
        ));
        // Same-size tampering keeps the metadata check passing but the digest
        // binding catches it before execution.
        let tampered = prepared.join("transformer/model.safetensors");
        fs::write(&tampered, b"dit-weightZ").unwrap();
        assert!(prepared_cache_metadata_matches(
            &install.inner.paths.data,
            &model_id,
            profile,
            &record
        ));
        let error = prepared_cache_matches(&install.inner.paths.data, &model_id, profile, &record)
            .unwrap_err();
        assert!(error.contains("量化缓存内容"), "{error}");
        write_prepared_cache(&prepared);
        // A record from before the digest binding (serde defaults) is stale.
        let mut legacy = staged_ready_record(&install, profile, &model_id);
        legacy.prepared_dir = Some(prepared.to_string_lossy().into_owned());
        assert!(
            prepared_cache_matches(&install.inner.paths.data, &model_id, profile, &legacy).is_err()
        );
        assert!(!prepared_cache_metadata_matches(
            &install.inner.paths.data,
            &model_id,
            profile,
            &legacy
        ));
        // A foreign prepared directory or a non-staged profile is handled safely.
        record.prepared_dir = Some(
            install
                .inner
                .paths
                .data
                .join("models/other.prepared")
                .to_string_lossy()
                .into_owned(),
        );
        assert!(
            prepared_cache_matches(&install.inner.paths.data, &model_id, profile, &record).is_err()
        );
        record.prepared_dir = Some(prepared.to_string_lossy().into_owned());
        assert!(
            prepared_cache_matches(&install.inner.paths.data, &model_id, Z_PROFILE, &record)
                .is_ok()
        );
    }
    #[test]
    fn prepared_reuse_requires_same_model_manifest_and_matching_digest() {
        let profile = staged_profile();
        let (_temp, install) = fixture_for(|paths| Installation::new_for(paths, profile));
        let model_id = install.inner.model.id.clone();
        let prepared = prepared_cache_dir(&install.inner.paths.data, &model_id);
        fs::create_dir_all(&prepared).unwrap();
        let (files, digest) = write_prepared_cache(&prepared);
        let mut record = staged_ready_record(&install, profile, &model_id);
        record.prepared_dir = Some(prepared.to_string_lossy().into_owned());
        record.prepared_files = files;
        record.prepared_digest = Some(digest);
        let ready_path = install.inner.paths.data.join(profile.ready_relative);
        ensure_child_dir(&install.inner.paths.data, ready_path.parent().unwrap()).unwrap();
        atomic_json(&ready_path, &record).unwrap();
        assert!(install.prepared_cache_reuse_allowed(&prepared));
        // A changed model manifest digest must force a clean rebuild.
        record.model_manifest_sha256 = "a".repeat(64);
        atomic_json(&ready_path, &record).unwrap();
        assert!(!install.prepared_cache_reuse_allowed(&prepared));
        record.model_manifest_sha256 = manifest_digest(profile.model_manifest);
        atomic_json(&ready_path, &record).unwrap();
        assert!(install.prepared_cache_reuse_allowed(&prepared));
        // Tampered cache content invalidates reuse even at the same size.
        fs::write(
            prepared.join("transformer/model.safetensors"),
            b"dit-weightZ",
        )
        .unwrap();
        assert!(!install.prepared_cache_reuse_allowed(&prepared));
        write_prepared_cache(&prepared);
        // A record without the digest binding (old build) is not reusable.
        record.prepared_files.clear();
        record.prepared_digest = None;
        atomic_json(&ready_path, &record).unwrap();
        assert!(!install.prepared_cache_reuse_allowed(&prepared));
        // A missing ready record is not reusable.
        fs::remove_file(&ready_path).unwrap();
        assert!(!install.prepared_cache_reuse_allowed(&prepared));
    }
    #[test]
    fn prepare_memory_preflight_boundaries_and_actionable_message() {
        assert!(prepare_memory_preflight(memory_facts(28, 0)).is_ok());
        assert!(prepare_memory_preflight(memory_facts(0, 26)).is_ok());
        assert!(prepare_memory_preflight(memory_facts(28, 26)).is_ok());
        assert!(prepare_memory_preflight(memory_facts(31, 27)).is_ok());
        let below = memory_facts(27, 25);
        let error = prepare_memory_preflight(below).unwrap_err();
        assert!(error.contains("内存不足"), "{error}");
        assert!(error.contains("未开始构建"), "{error}");
        assert!(error.contains("未激活"), "{error}");
        assert!(error.contains("27.0 GiB"), "{error}");
        assert!(error.contains("25.0 GiB"), "{error}");
        assert!(error.contains("28 GiB"), "{error}");
        assert!(error.contains("26 GiB"), "{error}");
        // Exact byte boundaries: one byte below both minima fails, one minimum met
        // passes.
        let just_below = MemoryFacts {
            available_physical_bytes: PREPARE_MIN_AVAILABLE_PHYSICAL_BYTES - 1,
            available_commit_bytes: PREPARE_MIN_COMMIT_HEADROOM_BYTES - 1,
        };
        assert!(prepare_memory_preflight(just_below).is_err());
        let at_commit = MemoryFacts {
            available_physical_bytes: 0,
            available_commit_bytes: PREPARE_MIN_COMMIT_HEADROOM_BYTES,
        };
        assert!(prepare_memory_preflight(at_commit).is_ok());
        let record = at_commit.preflight_record();
        assert_eq!(
            record.minimum_available_physical_bytes,
            PREPARE_MIN_AVAILABLE_PHYSICAL_BYTES
        );
        assert_eq!(
            record.minimum_commit_headroom_bytes,
            PREPARE_MIN_COMMIT_HEADROOM_BYTES
        );
        assert_eq!(
            record.available_commit_bytes,
            PREPARE_MIN_COMMIT_HEADROOM_BYTES
        );
    }
    #[test]
    fn prepare_quantization_is_skipped_when_the_memory_preflight_fails() {
        let profile = staged_profile();
        let (_temp, install) = fixture_for(|paths| Installation::new_for(paths, profile));
        let prepared = prepared_cache_dir(&install.inner.paths.data, &install.inner.model.id);
        let error = install
            .prepare_quantized_cache_with_facts(memory_facts(6, 5))
            .unwrap_err();
        assert!(error.contains("内存不足"), "{error}");
        assert!(error.contains("6.0 GiB"), "{error}");
        // Nothing was created, activated or recorded by the prepare step.
        assert!(!prepared.exists());
        assert!(!install.inner.paths.data.join("models").exists());
        assert!(
            !install
                .inner
                .paths
                .data
                .join("installation-ready.json")
                .exists()
        );
        assert!(
            !install
                .inner
                .paths
                .data
                .join("installation-installing.json")
                .exists()
        );
        assert_eq!(install.snapshot().phase, "not_installed");
        assert!(!install.snapshot().runtime_ready);
        assert!(!install.snapshot().model_ready);
        // The measured values are persisted as evidence and in the install log.
        let preflight: serde_json::Value = serde_json::from_slice(
            &fs::read(install.inner.paths.logs.join("prepare-preflight.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(preflight["available_physical_bytes"], 6 * GIB_BYTES);
        assert_eq!(preflight["available_commit_bytes"], 5 * GIB_BYTES);
        assert_eq!(
            preflight["minimum_available_physical_bytes"],
            PREPARE_MIN_AVAILABLE_PHYSICAL_BYTES
        );
        let log = fs::read_to_string(install.inner.paths.logs.join("installation-process.log"))
            .unwrap_or_default();
        assert!(log.contains("[prepare-preflight]"), "{log}");
        assert!(log.contains("result=blocked"), "{log}");
    }
    #[cfg(windows)]
    #[test]
    fn windows_prepare_ram_preflight_evidence_is_recorded() {
        let example_facts = memory_facts(12, 8);
        let boundaries = [
            (28, 0, "pass_available_ram"),
            (0, 26, "pass_commit_headroom"),
            (28, 26, "pass_both"),
            (27, 25, "fail_both_below"),
            (28, 25, "pass_available_ram_with_low_commit"),
            (27, 26, "pass_commit_headroom_with_low_ram"),
        ]
        .map(|(ram, commit, label)| {
            serde_json::json!({
                "label": label,
                "available_physical_gib": ram,
                "available_commit_gib": commit,
                "passed": prepare_memory_preflight(memory_facts(ram, commit)).is_ok(),
            })
        });
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let directory = repo.join("verification/windows-parity-app");
        fs::create_dir_all(&directory).unwrap();
        atomic_json(
            &directory.join("prepare-ram-preflight.json"),
            &serde_json::json!({
                "schema_version": 1,
                "kind": "windows-parity-prepare-ram-preflight",
                "status": "unit_verified",
                "thresholds": {
                    "minimum_available_physical_bytes": PREPARE_MIN_AVAILABLE_PHYSICAL_BYTES,
                    "minimum_available_physical_gib": 28,
                    "minimum_commit_headroom_bytes": PREPARE_MIN_COMMIT_HEADROOM_BYTES,
                    "minimum_commit_headroom_gib": 26,
                },
                "decision_rule": "available physical RAM >= 28 GiB OR available commit headroom >= 26 GiB",
                "measurement_source": "installer-private GlobalMemoryStatusEx (ullAvailPhys / ullAvailPageFile); no new crate and no cross-stream helper",
                "measured_values_recorded_in": [
                    "ReadyRecord.prepare_preflight",
                    "logs/installation-process.log line '[prepare-preflight] available_physical_bytes=... available_commit_bytes=... minimum_available_physical_bytes=... minimum_commit_headroom_bytes=...'"
                ],
                "failure_message_example_for_12GiB_available_ram_8GiB_commit": prepare_memory_error(example_facts),
                "boundary_cases": boundaries,
                "when_it_runs": "after model+runtime install and runtime probe, before any quantization work; a failure leaves no ready record and does not activate the installation",
                "real_install_note": "actual measured values are produced by the Lead's installed-app acceptance installs; this file documents the implemented gate and its unit-tested boundaries",
            }),
        )
        .unwrap();
    }
    fn server(
        response: Vec<u8>,
    ) -> (
        String,
        std::sync::mpsc::Receiver<String>,
        std::thread::JoinHandle<()>,
    ) {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/fixture", listener.local_addr().unwrap());
        let (tx, rx) = std::sync::mpsc::channel();
        let task = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                assert!(request.len() < 8192);
            }
            tx.send(String::from_utf8(request).unwrap()).unwrap();
            socket.write_all(&response).unwrap();
        });
        (url, rx, task)
    }
    fn test_artifact(url: String, bytes: &[u8]) -> Artifact {
        Artifact {
            path: "fixture.bin".into(),
            url,
            bytes: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
        }
    }
    #[test]
    fn interrupted_transfer_keeps_a_resumable_part_but_never_a_final_file() {
        let (_temp, i) = fixture();
        let (url, request, task) = server(
            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nETag: \"one\"\r\nConnection: close\r\n\r\nhe"
                .to_vec(),
        );
        let a = test_artifact(url, b"hello");
        assert!(i.download(&a).is_err());
        request.recv().unwrap();
        task.join().unwrap();
        let target = i.cache().join(a.sha256);
        assert!(!target.exists());
        assert_eq!(fs::read(target.with_extension("part")).unwrap(), b"he");
        assert!(target.with_extension("resume.json").exists());
    }
    #[test]
    fn stalled_transfer_cancellation_does_not_wait_for_a_file_or_read_deadline() {
        use std::net::TcpListener;
        let (_temp, i) = fixture();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/stall", listener.local_addr().unwrap());
        let (tx, ready) = std::sync::mpsc::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\n")
                .unwrap();
            tx.send(()).unwrap();
            let _ = wait.recv_timeout(Duration::from_secs(5));
        });
        let worker = i.clone();
        let task = std::thread::spawn(move || worker.download(&test_artifact(url, b"hello")));
        ready.recv_timeout(Duration::from_secs(3)).unwrap();
        let start = std::time::Instant::now();
        i.cancel();
        assert!(task.join().unwrap().is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
        release.send(()).unwrap();
        server.join().unwrap();
    }
    #[test]
    fn tar_links_cannot_escape_the_private_runtime() {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("unsafe.tar.gz");
        let out = File::create(&archive).unwrap();
        let zip = flate2::write::GzEncoder::new(out, flate2::Compression::default());
        let mut builder = tar::Builder::new(zip);
        let mut header = tar::Header::new_gnu();
        header.set_path("python/escape").unwrap();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_link_name("../../outside").unwrap();
        header.set_size(0);
        header.set_mode(0o777);
        header.set_cksum();
        builder.append(&header, std::io::empty()).unwrap();
        builder.into_inner().unwrap().finish().unwrap();
        let dest = temp.path().join("runtime");
        fs::create_dir(&dest).unwrap();
        assert!(extract(&archive, &dest).is_err());
        assert!(!temp.path().join("outside").exists());
    }
    #[test]
    fn range_resume_requires_identity_and_preserves_exact_bytes() {
        let (_temp, i) = fixture();
        let data = b"hello world";
        let(url,request,task)=server(b"HTTP/1.1 206 Partial Content\r\nContent-Length: 6\r\nContent-Range: bytes 5-10/11\r\nETag: \"one\"\r\nConnection: close\r\n\r\n world".to_vec());
        let a = test_artifact(url, data);
        let dest = i.cache().join(&a.sha256);
        fs::write(dest.with_extension("part"), b"hello").unwrap();
        atomic_json(
            &dest.with_extension("resume.json"),
            &serde_json::json!({"etag":"\"one\"","sha256":a.sha256}),
        )
        .unwrap();
        i.download(&a).unwrap();
        let headers = request.recv().unwrap().to_ascii_lowercase();
        assert!(headers.contains("range: bytes=5-"));
        assert!(headers.contains("if-range: \"one\""));
        task.join().unwrap();
        assert_eq!(fs::read(dest).unwrap(), data);
        assert_eq!(i.snapshot().received_bytes, 11);
    }
    #[test]
    fn segmented_range_resume_preserves_the_existing_prefix() {
        let (_temp, i) = fixture();
        let data = b"hello world";
        let(url,request,task)=server(b"HTTP/1.1 206 Partial Content\r\nContent-Length: 6\r\nContent-Range: bytes 5-10/11\r\nETag: \"one\"\r\nConnection: close\r\n\r\n world".to_vec());
        let path = i.cache().join("segment.part");
        let metadata_path = path.with_extension("part.json");
        fs::write(&path, b"hello").unwrap();
        atomic_json(
            &metadata_path,
            &serde_json::json!({
                "sha256": format!("{:x}", Sha256::digest(data)),
                "etag": "\"one\"",
                "start": 0,
                "end": 11,
                "total": 11,
                "offset": 5
            }),
        )
        .unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime
            .block_on(i.download_segment(
                &http_client().unwrap(),
                SegmentDownload {
                    url: &url,
                    artifact_path: "fixture.bin",
                    path: &path,
                    metadata_path: &metadata_path,
                    sha256: &format!("{:x}", Sha256::digest(data)),
                    etag: "\"one\"",
                    start: 0,
                    end: 11,
                    total: 11,
                },
            ))
            .unwrap();
        let headers = request.recv().unwrap().to_ascii_lowercase();
        assert!(headers.contains("range: bytes=5-"));
        assert!(headers.contains("if-range: \"one\""));
        task.join().unwrap();
        assert_eq!(fs::read(path).unwrap(), data);
    }
    #[test]
    fn only_segments_matching_the_current_source_identity_count_as_progress() {
        let temp = tempfile::tempdir().unwrap();
        let paths = (0..3)
            .map(|index| temp.path().join(format!("segment-{index}")))
            .collect::<Vec<_>>();
        let metadata_paths = paths
            .iter()
            .map(|path| path.with_extension("json"))
            .collect::<Vec<_>>();
        let ranges = [(0, 4), (4, 7), (7, 9)];
        for (index, bytes) in [4_u64, 3, 2].into_iter().enumerate() {
            fs::write(&paths[index], vec![b'x'; bytes as usize]).unwrap();
            atomic_json(
                &metadata_paths[index],
                &serde_json::json!({
                    "sha256": "a".repeat(64),
                    "etag": if index == 1 { "\"old\"" } else { "\"current\"" },
                    "start": ranges[index].0,
                    "end": ranges[index].1,
                    "offset": bytes,
                    "total": 9,
                }),
            )
            .unwrap();
        }
        assert_eq!(
            resumable_segment_bytes(
                &paths,
                &metadata_paths,
                &ranges,
                &"a".repeat(64),
                "\"current\"",
            ),
            6
        );
    }
    #[test]
    fn full_response_replaces_a_partial_without_appending() {
        let (_temp, i) = fixture();
        let(url,request,task)=server(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nETag: \"new\"\r\nConnection: close\r\n\r\nhello".to_vec());
        let a = test_artifact(url, b"hello");
        let dest = i.cache().join(&a.sha256);
        fs::write(dest.with_extension("part"), b"xx").unwrap();
        atomic_json(
            &dest.with_extension("resume.json"),
            &serde_json::json!({"etag":"\"old\"","sha256":a.sha256}),
        )
        .unwrap();
        i.download(&a).unwrap();
        request.recv().unwrap();
        task.join().unwrap();
        assert_eq!(fs::read(dest).unwrap(), b"hello");
        assert_eq!(i.snapshot().received_bytes, 5);
    }
    #[test]
    fn wrong_digest_never_becomes_a_completed_cache_file() {
        let (_temp, i) = fixture();
        let (url, request, task) = server(
            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nwrong".to_vec(),
        );
        let a = test_artifact(url, b"hello");
        assert!(i.download(&a).unwrap_err().contains("SHA-256"));
        request.recv().unwrap();
        task.join().unwrap();
        assert!(!i.cache().join(a.sha256).exists());
        assert!(!i.snapshot().model_ready);
    }
    #[cfg(unix)]
    #[test]
    fn cached_part_and_managed_parent_links_cannot_write_outside() {
        use std::os::unix::fs::symlink;
        let (temp, i) = fixture();
        let outside = temp.path().join("outside");
        fs::write(&outside, b"preserve").unwrap();
        let part = i.cache().join("part");
        symlink(&outside, &part).unwrap();
        assert!(open_regular(&part, true, true).is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"preserve");
        symlink(temp.path(), i.inner.paths.data.join("models")).unwrap();
        assert!(ensure_child_dir(&i.inner.paths.data, &i.model_dir()).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn runtime_inventory_detects_changed_link_targets() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        fs::write(root.join("python"), b"trusted").unwrap();
        symlink("python", root.join("alias")).unwrap();
        let mut entries = vec![];
        inventory(&root, &root, &mut entries).unwrap();
        assert_eq!(entries.len(), 2);
        for entry in &entries {
            inventory_metadata_matches(&root, entry).unwrap();
        }
        fs::remove_file(root.join("alias")).unwrap();
        symlink("/bin/sh", root.join("alias")).unwrap();
        assert!(
            inventory_metadata_matches(&root, entries.iter().find(|e| e.path == "alias").unwrap())
                .is_err()
        );
    }
    #[test]
    fn plan_metadata_keeps_all_seventeen_pinned_components() {
        let (_temp, i) = fixture();
        atomic_json(
            &i.inner.paths.data.join("installation-ready.json"),
            &ReadyRecord {
                runtime_lock_id: i.inner.runtime.id.clone(),
                model_snapshot_id: i.inner.model.id.clone(),
                runtime_manifest_sha256: manifest_digest(RUNTIME_MANIFEST),
                model_manifest_sha256: manifest_digest(MODEL_MANIFEST),
                licenses: vec![],
                prepared_dir: None,
                prepared_files: vec![],
                prepared_digest: None,
                worker_wheels: vec![],
                prepare_preflight: None,
                runtime_files: vec![],
                probe: serde_json::json!({"device":"gpu","device_name":"fixture-device"}),
            },
        )
        .unwrap();
        let plan = i.plan_environment().unwrap();
        assert_eq!(plan.components.len(), 17);
        assert_eq!(plan.scheduler, "linear");
        assert_eq!(
            plan.runtime.source.revision,
            manifest_digest(RUNTIME_MANIFEST)
        );
        assert_eq!(
            plan.runtime.source.source,
            format!("image-studio:runtime-manifest:{}", i.inner.runtime.id)
        );
        assert_eq!(
            plan.device.precision,
            "native;compute_precision=None;float32=False"
        );
        assert!(
            plan.components
                .iter()
                .all(|c| c.source.revision == i.inner.model.revision)
        );
        assert!(
            !Installation::new(i.inner.paths.clone())
                .unwrap()
                .snapshot()
                .runtime_ready
        );
    }
    #[test]
    fn fixed_manifests_are_complete() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::resolve(Some(temp.path())).unwrap();
        let installer = Installation::new(paths).unwrap();
        assert_eq!(
            installer
                .inner
                .model
                .files
                .iter()
                .filter(|a| a.path != "README.md")
                .map(|a| a.bytes)
                .sum::<u64>(),
            32_848_304_893
        );
        assert_eq!(installer.snapshot().phase, "not_installed");
        assert!(installer.snapshot().required_bytes > installer.snapshot().total_bytes * 2);
        assert!(installer.ready_paths().is_err());
        assert!(installer.start("https://untrusted").is_err());
    }

    #[test]
    fn windows_gate_zero_manifest_is_an_independent_complete_closure() {
        const WINDOWS_GATE_ZERO_MANIFEST: &str = include_str!(
            "../../../../resources/windows-foundation/z-image-diffusers/model-manifest.json"
        );
        let manifest: ModelManifest = serde_json::from_str(WINDOWS_GATE_ZERO_MANIFEST).unwrap();
        assert_eq!(manifest.id, "z-image-turbo-diffusers-cuda-nf4-v1");
        assert_eq!(
            manifest.revision,
            "f332072aa78be7aecdf3ee76d5c247082da564a6"
        );
        let manifest_json: serde_json::Value =
            serde_json::from_str(WINDOWS_GATE_ZERO_MANIFEST).unwrap();
        assert_eq!(manifest_json["download_source"]["provider"], "modelscope");
        assert_eq!(
            manifest_json["download_source"]["repository"],
            "Tongyi-MAI/Z-Image-Turbo"
        );
        assert_eq!(manifest_json["download_source"]["revision"], "master");
        assert_eq!(manifest.component_bytes, 32_848_305_533);
        assert_eq!(manifest.files.len(), 20);
        assert_eq!(
            manifest
                .files
                .iter()
                .filter(|artifact| artifact.path != "README.md")
                .map(|artifact| artifact.bytes)
                .sum::<u64>(),
            manifest.component_bytes
        );
        assert_eq!(
            manifest
                .files
                .iter()
                .filter(|artifact| !artifact.path.ends_with("README.md"))
                .count(),
            19
        );
        for path in [
            "model_index.json",
            "scheduler/scheduler_config.json",
            "transformer/config.json",
            "vae/config.json",
        ] {
            assert!(manifest.files.iter().any(|artifact| artifact.path == path));
        }
        let mut paths = manifest
            .files
            .iter()
            .map(|artifact| artifact.path.as_str())
            .collect::<Vec<_>>();
        paths.sort_unstable();
        paths.dedup();
        assert_eq!(paths.len(), manifest.files.len());
        let manifest_files = manifest_json["files"].as_array().unwrap();
        assert_eq!(manifest_files.len(), manifest.files.len());
        for (artifact, raw) in manifest.files.iter().zip(manifest_files) {
            assert!(artifact.url.starts_with(
                "https://www.modelscope.cn/models/Tongyi-MAI/Z-Image-Turbo/resolve/master/"
            ));
            assert!(raw["upstream_url"].as_str().unwrap().starts_with(&format!(
                "https://huggingface.co/Tongyi-MAI/Z-Image-Turbo/resolve/{}/",
                manifest.revision
            )));
            assert_eq!(artifact.sha256.len(), 64);
            assert!(artifact.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()));
            assert!(artifact.bytes > 0);
        }
    }

    #[test]
    fn requirements_lock_identity_normalizes_checkout_line_endings() {
        assert_eq!(
            canonical_requirements_lock("alpha==1\r\nbeta==2\r\n"),
            "alpha==1\nbeta==2\n"
        );
    }

    #[test]
    fn windows_gate_zero_runtime_manifest_is_fixed_and_binary_only() {
        const RUNTIME_MANIFEST: &str = include_str!(
            "../../../../resources/windows-foundation/python-runtime/runtime-manifest.json"
        );
        const RUNTIME_LOCK: &str = include_str!(
            "../../../../resources/windows-foundation/python-runtime/requirements.lock"
        );
        let manifest: serde_json::Value = serde_json::from_str(RUNTIME_MANIFEST).unwrap();
        assert_eq!(manifest["schema_version"], 1);
        assert_eq!(
            manifest["id"],
            "windows-gate0-python31210-torch2100-cu126-bnb0482-v1"
        );
        assert_eq!(manifest["platform"], "windows-amd64");
        assert_eq!(manifest["python"], "3.12.10");
        assert_eq!(manifest["torch"], "2.10.0+cu126");
        assert_eq!(manifest["cuda"], "12.6");
        assert_eq!(manifest["artifact_count"], 38);
        assert_eq!(manifest["total_bytes"], 2_713_854_538_u64);
        let lock = &manifest["requirements_lock"];
        let runtime_lock = canonical_requirements_lock(RUNTIME_LOCK);
        assert_eq!(lock["bytes"], runtime_lock.len() as u64);
        assert_eq!(
            lock["sha256"],
            format!("{:x}", Sha256::digest(runtime_lock.as_bytes()))
        );

        let interpreter = &manifest["interpreter"];
        assert_eq!(interpreter["version"], "3.12.10");
        assert_eq!(
            interpreter["url"],
            "https://www.python.org/ftp/python/3.12.10/python-3.12.10-embed-amd64.zip"
        );
        assert!(interpreter["path"].as_str().unwrap().ends_with(".zip"));

        let wheels = manifest["wheels"].as_array().unwrap();
        assert_eq!(wheels.len(), 37);
        let mut paths = std::collections::BTreeSet::new();
        for wheel in wheels {
            let path = wheel["path"].as_str().unwrap();
            assert!(path.ends_with(".whl"), "{path}");
            assert!(!path.contains('/') && !path.contains('\\'), "{path}");
            assert!(paths.insert(path), "duplicate wheel path: {path}");
            assert!(wheel["bytes"].as_u64().unwrap() > 0);
            assert_eq!(wheel["sha256"].as_str().unwrap().len(), 64);
            let url = wheel["url"].as_str().unwrap();
            assert!(url.starts_with("https://"), "{path}");
            if wheel["package"] != "torch" {
                assert!(url.starts_with("https://files.pythonhosted.org/"), "{path}");
            }
        }
        for (package, version, path_fragment) in [
            ("torch", "2.10.0+cu126", "cp312-cp312-win_amd64"),
            ("bitsandbytes", "0.48.2", "win_amd64"),
            ("diffusers", "0.36.0", "py3-none-any"),
            ("transformers", "4.57.3", "py3-none-any"),
            ("accelerate", "1.12.0", "py3-none-any"),
        ] {
            let wheel = wheels
                .iter()
                .find(|wheel| wheel["package"] == package)
                .unwrap();
            assert_eq!(wheel["version"], version);
            assert!(wheel["path"].as_str().unwrap().contains(path_fragment));
        }
        assert_eq!(manifest["source_policy"]["binary_only"], true);
        assert_eq!(manifest["source_policy"]["system_python_modified"], false);
    }

    #[test]
    fn windows_gate_zero_runtime_probe_remains_explicitly_partial() {
        const PROBE: &str =
            include_str!("../../../../verification/windows-gate0/runtime-probe.json");
        let probe: serde_json::Value = serde_json::from_str(PROBE).unwrap();
        assert_eq!(probe["status"], "passed");
        assert_eq!(probe["is_full_model_inference"], false);
        assert_eq!(probe["is_installer_activation"], false);
        assert_eq!(probe["versions"]["torch"], "2.10.0+cu126");
        assert_eq!(probe["versions"]["bitsandbytes"], "0.48.2");
        assert_eq!(
            probe["cuda"]["selected_device"]["name"],
            "NVIDIA GeForce RTX 3080 Ti"
        );
        assert_eq!(probe["cuda"]["selected_device"]["major"], 8);
        assert_eq!(probe["cuda"]["selected_device"]["minor"], 6);
        assert_eq!(
            probe["cuda"]["external_uuid_match"]["pci_bus_id"],
            "00000000:01:00.0"
        );
        assert_eq!(
            probe["cuda"]["external_uuid_match"]["driver_version"],
            "596.49"
        );
        assert_eq!(probe["cuda"]["bf16_matmul"], true);
        assert_eq!(probe["cuda"]["nf4_linear_forward_backward"], true);
        assert!(
            probe["platform"]["python_executable"]
                .as_str()
                .unwrap()
                .starts_with("target/windows-gate0/")
        );
    }

    #[test]
    fn windows_gate_zero_result_records_three_budget_passed_runs() {
        const RESULT: &str =
            include_str!("../../../../verification/windows-gate0/gate0-results.json");
        const MODEL_SCOPE: &str =
            include_str!("../../../../verification/windows-gate0/modelscope-equivalence.json");
        let result: serde_json::Value = serde_json::from_str(RESULT).unwrap();
        assert_eq!(result["status"], "passed");
        assert_eq!(result["product_activation"], "not_run");
        assert_eq!(result["selected_candidate"]["id"], "a");
        assert_eq!(result["model"]["verified_files"], 20);
        assert_eq!(result["model"]["modelscope_verified_files"], 20);
        assert_eq!(result["budget"]["passed"], true);
        assert!(
            result["budget"]["cuda_peak_reserved_max_bytes"]
                .as_u64()
                .unwrap()
                <= result["budget"]["cuda_peak_reserved_limit_bytes"]
                    .as_u64()
                    .unwrap()
        );
        assert!(
            result["budget"]["private_commit_peak_max_bytes"]
                .as_u64()
                .unwrap()
                <= result["budget"]["private_commit_limit_bytes"]
                    .as_u64()
                    .unwrap()
        );
        assert!(
            result["budget"]["available_ram_min_bytes"]
                .as_u64()
                .unwrap()
                >= result["budget"]["available_ram_minimum_bytes"]
                    .as_u64()
                    .unwrap()
        );
        let runs = result["runs"].as_array().unwrap();
        assert_eq!(runs.len(), 3);
        for run in runs {
            assert_eq!(run["callback_total"], 9);
            assert!(run["load_seconds"].as_f64().unwrap() <= 600.0);
            assert!(run["generation_seconds"].as_f64().unwrap() <= 900.0);
            assert_eq!(
                run["visual_checks"]["size"],
                serde_json::json!([1024, 1024])
            );
            assert_eq!(run["visual_checks"]["mode"], "RGB");
            assert_eq!(run["visual_checks"]["non_black"], true);
            assert_eq!(run["image_sha256"].as_str().unwrap().len(), 64);
        }
        let modelscope: serde_json::Value = serde_json::from_str(MODEL_SCOPE).unwrap();
        assert_eq!(modelscope["status"], "passed");
        assert_eq!(modelscope["source_revision"], "master");
        assert_eq!(modelscope["verified_file_count"], 20);
    }

    #[cfg(windows)]
    #[test]
    fn windows_probe_cleanup_reaps_pipe_holding_descendants() {
        let (temp, installer) = fixture();
        let script = temp.path().join("probe-with-descendant.cmd");
        fs::write(
            &script,
            concat!(
                "@start \"\" /B \"%COMSPEC%\" /D /C \"ping -n 60 127.0.0.1 >NUL\"\r\n",
                "@echo probe-ok\r\n",
                "@exit /B 0\r\n"
            ),
        )
        .unwrap();
        let output = installer
            .command(
                Command::new(std::env::var_os("COMSPEC").unwrap())
                    .args(["/D", "/C"])
                    .arg(script),
            )
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&output).trim(), "probe-ok");
    }

    #[test]
    fn klein_has_an_independent_manifest_runtime_and_worker() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::resolve(Some(temp.path())).unwrap();
        let installer = Installation::new_klein(paths).unwrap();
        assert_eq!(installer.snapshot().install_id, KLEIN_INSTALL_ID);
        assert_eq!(
            installer
                .inner
                .model
                .files
                .iter()
                .map(|file| file.bytes)
                .sum::<u64>(),
            15_980_131_745
        );
        let worker = installer
            .inner
            .runtime
            .wheels
            .iter()
            .find(|wheel| wheel.url == "bundled:worker-wheel")
            .unwrap();
        assert_eq!(worker.path, "studio_mflux_worker_v2-0.2.0-py3-none-any.whl");
        assert_eq!(
            installer
                .inner
                .runtime
                .packages
                .get("studio-mflux-worker-v2")
                .map(String::as_str),
            Some("0.2.0")
        );
        assert_eq!(worker.bytes, KLEIN_WORKER_WHEEL.len() as u64);
        assert_eq!(
            worker.sha256,
            format!("{:x}", Sha256::digest(KLEIN_WORKER_WHEEL))
        );
        assert!(installer.ready_paths().is_err());
        assert_ne!(
            installer.inner.runtime.id,
            "mflux-0.24.0-py3.13.12-macos-arm64-v1"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn qwen_and_moss_share_v3_runtime_but_keep_model_identity_and_worker_arguments() {
        for (profile, expected_model_bytes, expected_ready) in [
            (
                QWEN_IMAGE_21_PROFILE,
                33_131_609_424,
                "installations/models/qwen-image-2.1-mflux-v1.json",
            ),
            (
                QWEN_IMAGE_21_TURBO_PROFILE,
                32_451_607_098,
                "installations/models/qwen-image-2.1-turbo-mflux-v1.json",
            ),
            (
                MOSS_IMAGE_SFT_PROFILE,
                27_219_783_878,
                "installations/models/moss-image-sft-mflux-v1.json",
            ),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let paths = AppPaths::resolve(Some(temp.path())).unwrap();
            let installer = Installation::new_for(paths, profile).unwrap();
            assert_eq!(installer.snapshot().install_id, profile.install_id);
            assert_eq!(installer.inner.model.component_bytes, expected_model_bytes);
            assert_eq!(profile.ready_relative, expected_ready);
            assert_eq!(
                installer.inner.runtime.id,
                "mflux-0.24.0-py3.13.12-macos-arm64-recipes-v3"
            );
            assert_eq!(
                installer.inner.profile.worker_module,
                RECIPES_V3_WORKER_MODULE
            );
            assert!(installer.inner.profile.worker_model_argument);
            assert!(installer.inner.model.files.iter().all(|file| {
                file.sha256.len() == 64 && file.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            }));
            let worker = installer
                .inner
                .runtime
                .wheels
                .iter()
                .find(|wheel| wheel.url == "bundled:worker-wheel")
                .unwrap();
            assert_eq!(
                worker.sha256,
                format!("{:x}", Sha256::digest(RECIPES_V3_WORKER_WHEEL))
            );
            assert!(installer.ready_paths().is_err());
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn qwen_and_moss_manifests_use_pinned_modelscope_files() {
        for (profile, repository) in [
            (QWEN_IMAGE_21_PROFILE, "Qwen/Qwen-Image-2.1"),
            (QWEN_IMAGE_21_TURBO_PROFILE, "Qwen/Qwen-Image-2.1-Turbo"),
            (MOSS_IMAGE_SFT_PROFILE, "OpenMOSS/MOSS-Image-SFT"),
        ] {
            let manifest: serde_json::Value = serde_json::from_str(profile.model_manifest).unwrap();
            assert_eq!(manifest["download_source"]["provider"], "modelscope");
            assert_eq!(manifest["download_source"]["repository"], repository);
            assert_eq!(manifest["download_source"]["revision_policy"], "per-file");
            let files = manifest["files"].as_array().unwrap();
            assert!(!files.is_empty());
            for file in files {
                let revision = file["source_revision"].as_str().unwrap();
                assert_eq!(revision.len(), 40);
                assert!(revision.bytes().all(|byte| byte.is_ascii_hexdigit()));
                let url = file["url"].as_str().unwrap();
                assert!(url.starts_with(&format!(
                    "https://www.modelscope.cn/models/{repository}/resolve/{revision}/"
                )));
                assert!(!url.contains("/master/"));
                assert!(allowed_url(&reqwest::Url::parse(url).unwrap()));
            }
        }
    }

    #[cfg(not(windows))]
    #[test]
    #[ignore = "explicit acceptance: requires ModelScope network access"]
    fn actual_modelscope_fixed_range_probe_returns_partial_content() {
        let manifest: serde_json::Value =
            serde_json::from_str(QWEN_IMAGE_21_MODEL_MANIFEST).unwrap();
        let file = manifest["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|file| {
                file["path"] == "transformer/diffusion_pytorch_model-00001-of-00002.safetensors"
            })
            .unwrap();
        let url = file["url"].as_str().unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let response = http_client()
                .unwrap()
                .get(url)
                .header(reqwest::header::RANGE, "bytes=0-0")
                .send()
                .await
                .unwrap();
            let status = response.status();
            let headers = response.headers().clone();
            let body = response.text().await.unwrap();
            assert_eq!(
                status,
                reqwest::StatusCode::PARTIAL_CONTENT,
                "status={status} headers={headers:?} body={body}"
            );
            assert_eq!(
                headers
                    .get(reqwest::header::CONTENT_RANGE)
                    .and_then(|value| value.to_str().ok()),
                Some("bytes 0-0/9968332504")
            );
            assert!(headers.contains_key(reqwest::header::ETAG));
        });
    }

    #[test]
    fn birefnet_has_an_independent_onnx_runtime_and_worker() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::resolve(Some(temp.path())).unwrap();
        let installer = Installation::new_birefnet(paths).unwrap();
        assert_eq!(installer.snapshot().install_id, BIREFNET_INSTALL_ID);
        assert_eq!(installer.inner.model.files.len(), 1);
        assert_eq!(installer.inner.model.component_bytes, 972_666_916);
        assert_eq!(
            installer.inner.model.files[0].sha256,
            "58f621f00f5d756097615970a88a791584600dcf7c45b18a0a6267535a1ebd3c"
        );
        let worker = installer
            .inner
            .runtime
            .wheels
            .iter()
            .find(|wheel| wheel.url == "bundled:worker-wheel")
            .unwrap();
        assert_eq!(worker.path, "studio_onnx_worker-0.1.0-py3-none-any.whl");
        assert_eq!(
            installer
                .inner
                .runtime
                .packages
                .get("studio-onnx-worker")
                .map(String::as_str),
            Some("0.1.0")
        );
        assert_eq!(worker.bytes, BIREFNET_WORKER_WHEEL.len() as u64);
        assert_eq!(
            worker.sha256,
            format!("{:x}", Sha256::digest(BIREFNET_WORKER_WHEEL))
        );
        assert_eq!(installer.inner.profile.probe_device, "cpu");
        assert!(installer.inner.profile.probe_with_model);
        assert!(installer.ready_paths().is_err());
    }

    #[test]
    fn native_runtime_platform_is_the_current_operating_system() {
        assert_eq!(
            native_runtime_platform(),
            if cfg!(windows) {
                "windows-amd64"
            } else {
                "macos-universal2"
            }
        );
    }

    #[cfg(windows)]
    #[test]
    fn native_runtime_platform_gate_rejects_a_foreign_native_manifest() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::resolve(Some(temp.path())).unwrap();
        let error = match Installation::new_for(paths, REALESRGAN_PROFILE) {
            Ok(_) => panic!("a macOS native manifest must be rejected on Windows"),
            Err(error) => error,
        };
        assert!(error.contains("平台"), "{error}");
    }

    #[cfg(not(windows))]
    #[test]
    fn realesrgan_has_a_native_archive_runtime_and_fixed_model_pair() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::resolve(Some(temp.path())).unwrap();
        let installer = Installation::new_realesrgan(paths).unwrap();
        assert_eq!(installer.snapshot().install_id, REALESRGAN_INSTALL_ID);
        assert_eq!(installer.inner.model.component_bytes, 33_540_549);
        assert!(installer.inner.runtime.is_native());
        assert_eq!(
            installer.inner.runtime.id,
            "realesrgan-x4plus-ncnn-macos-v0.2.0-v1"
        );
        assert_eq!(
            installer
                .inner
                .model
                .installed_files()
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            vec!["realesrgan-x4plus.bin", "realesrgan-x4plus.param"]
        );
        assert_eq!(
            installer.inner.runtime.executable.as_deref(),
            Some("bin/realesrgan-ncnn-vulkan")
        );
        assert!(installer.ready_paths().is_err());
    }

    fn zip_fixture(i: &Installation, name: &str, symlink: bool) -> ArchiveArtifact {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();
        if symlink {
            writer.add_symlink(name, "../../outside", options).unwrap();
        } else {
            writer.start_file(name, options).unwrap();
            writer.write_all(b"trusted").unwrap();
        }
        let bytes = writer.finish().unwrap().into_inner();
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        fs::write(i.cache().join(&sha256), &bytes).unwrap();
        ArchiveArtifact {
            path: "fixture.zip".into(),
            url: "https://github.com/fixture/fixture.zip".into(),
            bytes: bytes.len() as u64,
            sha256,
            members: vec![ArchiveMember {
                archive_path: name.into(),
                path: "bin/tool".into(),
                bytes: 7,
                sha256: format!("{:x}", Sha256::digest(b"trusted")),
                executable: true,
            }],
        }
    }

    #[test]
    fn zip_extraction_rejects_links_and_paths_before_writing_members() {
        let (temp, installer) = fixture();
        let destination = temp.path().join("out");
        fs::create_dir(&destination).unwrap();
        for (name, symlink) in [
            ("../outside", false),
            ("/outside", false),
            ("C:\\outside", false),
            ("nested/../../outside", false),
            ("link", true),
        ] {
            let archive = zip_fixture(&installer, name, symlink);
            assert!(
                installer
                    .install_archive_members(&archive, &destination)
                    .is_err(),
                "{name}"
            );
            assert!(!destination.join("bin/tool").exists());
        }
        assert!(!temp.path().join("outside").exists());
    }

    #[test]
    fn zip_extraction_requires_member_size_hash_and_presence() {
        let (temp, installer) = fixture();
        let destination = temp.path().join("out");
        fs::create_dir(&destination).unwrap();
        let mut archive = zip_fixture(&installer, "tool", false);
        archive.members[0].bytes = 8;
        assert!(
            installer
                .install_archive_members(&archive, &destination)
                .is_err()
        );
        archive.members[0].bytes = 7;
        archive.members[0].sha256 = "0".repeat(64);
        assert!(
            installer
                .install_archive_members(&archive, &destination)
                .is_err()
        );
        archive.members[0].archive_path = "missing".into();
        assert!(
            installer
                .install_archive_members(&archive, &destination)
                .is_err()
        );
        assert!(!destination.join("bin/tool").exists());
    }

    #[cfg(unix)]
    #[test]
    fn zip_extraction_sets_native_mode_and_refuses_linked_parents() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let (temp, installer) = fixture_for(Installation::new_realesrgan);
        let destination = temp.path().join("out");
        fs::create_dir(&destination).unwrap();
        let archive = zip_fixture(&installer, "tool", false);
        installer
            .install_archive_members(&archive, &destination)
            .unwrap();
        let target = destination.join("bin/tool");
        assert_eq!(fs::read(&target).unwrap(), b"trusted");
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o755
        );
        let mut entries = vec![];
        inventory(&destination, &destination, &mut entries).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(inventory_metadata_matches(&destination, &entries[0]).is_err());
        fs::remove_dir_all(destination.join("bin")).unwrap();
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        symlink(&outside, destination.join("bin")).unwrap();
        assert!(
            installer
                .install_archive_members(&archive, &destination)
                .is_err()
        );
        assert!(!outside.join("tool").exists());
    }

    #[cfg(not(windows))]
    #[test]
    fn installation_manager_keeps_model_states_and_actions_independent() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::resolve(Some(temp.path())).unwrap();
        let manager = Installations::new(paths).unwrap();
        let snapshots = manager.snapshots();
        assert_eq!(snapshots.len(), 7);
        assert!(
            snapshots
                .iter()
                .all(|snapshot| snapshot.phase == "not_installed")
        );
        assert_eq!(
            manager.snapshot(KLEIN_INSTALL_ID).unwrap().install_id,
            KLEIN_INSTALL_ID
        );
        assert_eq!(
            manager.snapshot(BIREFNET_INSTALL_ID).unwrap().install_id,
            BIREFNET_INSTALL_ID
        );
        assert_eq!(
            manager.snapshot(REALESRGAN_INSTALL_ID).unwrap().install_id,
            REALESRGAN_INSTALL_ID
        );
        for install_id in [
            QWEN_IMAGE_21_INSTALL_ID,
            QWEN_IMAGE_21_TURBO_INSTALL_ID,
            MOSS_IMAGE_SFT_INSTALL_ID,
        ] {
            assert_eq!(manager.snapshot(install_id).unwrap().install_id, install_id);
        }
        assert!(manager.start("https://untrusted").is_err());
        assert!(manager.cancel("unregistered-model").is_err());
        assert!(
            manager
                .snapshot(INSTALL_ID)
                .is_some_and(|snapshot| snapshot.phase == "not_installed")
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn native_install_admission_is_exclusive_and_wrong_row_stop_is_isolated() {
        let temp = tempfile::tempdir().unwrap();
        let manager = Installations::new(AppPaths::resolve(Some(temp.path())).unwrap()).unwrap();
        manager
            .realesrgan
            .update(|state| state.phase = "downloading".into());
        assert!(
            manager
                .start(BIREFNET_INSTALL_ID)
                .unwrap_err()
                .contains("另一个型号")
        );
        manager.cancel(BIREFNET_INSTALL_ID).unwrap();
        assert!(!manager.realesrgan.inner.cancelled.load(Ordering::SeqCst));
        assert_eq!(
            manager.snapshot(REALESRGAN_INSTALL_ID).unwrap().phase,
            "downloading"
        );
        manager.cancel(REALESRGAN_INSTALL_ID).unwrap();
        assert!(manager.realesrgan.inner.cancelled.load(Ordering::SeqCst));
        assert_eq!(manager.snapshot(INSTALL_ID).unwrap().phase, "not_installed");
    }

    #[cfg(windows)]
    const WINDOWS_MODEL_BYTES: &[(&str, u64)] = &[
        (WINDOWS_ZIMAGE_INSTALL_ID, 32_848_305_533),
        (WINDOWS_KLEIN_INSTALL_ID, 15_980_141_329),
        (WINDOWS_BIREFNET_INSTALL_ID, 972_666_916),
        (WINDOWS_REALESRGAN_INSTALL_ID, 33_540_549),
        (WINDOWS_QWEN_IMAGE_21_INSTALL_ID, 33_131_609_424),
        (WINDOWS_QWEN_IMAGE_21_TURBO_INSTALL_ID, 32_451_607_098),
        (WINDOWS_MOSS_IMAGE_SFT_INSTALL_ID, 27_219_783_878),
    ];
    #[cfg(windows)]
    fn windows_profile(install_id: &str) -> InstallProfile {
        match install_id {
            WINDOWS_ZIMAGE_INSTALL_ID => WINDOWS_ZIMAGE_PROFILE,
            WINDOWS_KLEIN_INSTALL_ID => WINDOWS_KLEIN_PROFILE,
            WINDOWS_BIREFNET_INSTALL_ID => WINDOWS_BIREFNET_PROFILE,
            WINDOWS_REALESRGAN_INSTALL_ID => WINDOWS_REALESRGAN_PROFILE,
            WINDOWS_QWEN_IMAGE_21_INSTALL_ID => WINDOWS_QWEN_IMAGE_21_PROFILE,
            WINDOWS_QWEN_IMAGE_21_TURBO_INSTALL_ID => WINDOWS_QWEN_IMAGE_21_TURBO_PROFILE,
            WINDOWS_MOSS_IMAGE_SFT_INSTALL_ID => WINDOWS_MOSS_IMAGE_SFT_PROFILE,
            _ => panic!("unregistered Windows install id"),
        }
    }
    #[cfg(windows)]
    #[test]
    fn windows_installation_manager_exposes_all_seven_profiles_in_fixed_order() {
        let temp = tempfile::tempdir().unwrap();
        let manager = Installations::new(AppPaths::resolve(Some(temp.path())).unwrap()).unwrap();
        let snapshots = manager.snapshots();
        assert_eq!(
            snapshots
                .iter()
                .map(|snapshot| snapshot.install_id.as_str())
                .collect::<Vec<_>>(),
            WINDOWS_MODEL_BYTES
                .iter()
                .map(|(install_id, _)| *install_id)
                .collect::<Vec<_>>()
        );
        for &(install_id, model_bytes) in WINDOWS_MODEL_BYTES {
            let snapshot = manager.snapshot(install_id).unwrap();
            assert_eq!(snapshot.phase, "not_installed");
            assert_eq!(snapshot.model_bytes, model_bytes);
            assert!(!snapshot.runtime_ready);
            assert!(!snapshot.model_ready);
            assert!(!snapshot.device_verified);
        }
        assert!(manager.snapshot(INSTALL_ID).is_none());
        assert!(manager.snapshot(KLEIN_INSTALL_ID).is_none());
        assert!(manager.start(INSTALL_ID).is_err());
        assert!(manager.start("https://untrusted").is_err());
        assert!(manager.cancel("unregistered-model").is_err());
        let profiles = WINDOWS_MODEL_BYTES
            .iter()
            .map(|&(install_id, _)| {
                let profile = windows_profile(install_id);
                let runtime: RuntimeManifest =
                    serde_json::from_str(profile.runtime_manifest).unwrap();
                serde_json::json!({
                    "install_id": install_id,
                    "model_snapshot_id": execution_snapshot_id(install_id)
                        .map_or_else(|| install_id.to_owned(), str::to_owned),
                    "runtime_lock_id": runtime.id,
                    "worker_module": profile.worker_module,
                    "worker_build_id": profile.worker_build_id,
                    "engine": profile.engine,
                    "scheduler": profile.scheduler,
                    "precision": profile.precision,
                    "probe_device": profile.probe_device,
                    "prepare_with_model": profile.prepare_with_model,
                    "prepared_cache_reserve": profile.prepared_cache_reserve,
                    "bundled_license_count": profile.bundle_licenses.len(),
                    "shared_worker_wheels": profile
                        .shared_worker_wheels
                        .iter()
                        .map(|(path, _)| *path)
                        .collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>();
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        atomic_json(
            &repo.join("verification/windows-installer/windows-parity-wiring.json"),
            &serde_json::json!({
                "schema_version": 1,
                "kind": "windows-parity-installer-wiring",
                "status": "unit_verified",
                "snapshot_order": WINDOWS_MODEL_BYTES
                    .iter()
                    .map(|(install_id, _)| *install_id)
                    .collect::<Vec<_>>(),
                "prepared_cache_layout": "<data>/models/<install_id>.prepared",
                "license_layout": "<data>/licenses/<install_id>/<relative path>",
                "note": "Deterministic wiring evidence from cfg(windows) unit tests; real install and generation acceptance is run separately.",
                "profiles": profiles,
            }),
        )
        .unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn windows_profiles_parse_embedded_manifests_and_frozen_identities() {
        for &(install_id, model_bytes) in WINDOWS_MODEL_BYTES {
            let profile = windows_profile(install_id);
            let temp = tempfile::tempdir().unwrap();
            let installer =
                Installation::new_for(AppPaths::resolve(Some(temp.path())).unwrap(), profile)
                    .unwrap();
            assert_eq!(installer.snapshot().install_id, install_id);
            // new_for already enforced component_bytes and URL allowlist; assert the
            // dimensions wired into the host identity here.
            assert_eq!(installer.inner.model.component_bytes, model_bytes);
            let expected_snapshot = execution_snapshot_id(install_id).unwrap_or(install_id);
            assert_eq!(installer.snapshot_id(), expected_snapshot);
            assert_eq!(
                install_device_expectation(install_id),
                installer.inner.profile.probe_device
            );
            assert!(installer.inner.profile.probe_with_model);
            assert_eq!(
                installer.inner.profile.worker_model_argument,
                install_id != WINDOWS_ZIMAGE_INSTALL_ID
            );
            assert!(plan_device_id(install_id).is_some());
            if install_id == WINDOWS_BIREFNET_INSTALL_ID {
                assert_eq!(installer.inner.profile.probe_device, "windows-cpu");
                assert_eq!(
                    installer.inner.profile.engine,
                    studio_core::generation::WINDOWS_ONNX_ENGINE
                );
                assert!(!installer.inner.profile.bundle_licenses.is_empty());
                assert!(installer.inner.profile.shared_worker_wheels.len() == 2);
            } else if install_id == WINDOWS_REALESRGAN_INSTALL_ID {
                assert!(installer.inner.runtime.is_native());
                assert_eq!(
                    installer.inner.runtime.id,
                    studio_core::generation::REALESRGAN_WINDOWS_RUNTIME
                );
                assert_eq!(
                    installer.inner.runtime.executable.as_deref(),
                    Some("realesrgan-ncnn-vulkan.exe")
                );
                assert_eq!(
                    installer.inner.profile.engine,
                    studio_core::generation::WINDOWS_NCNN_ENGINE
                );
                assert_eq!(installer.inner.profile.bundle_licenses.len(), 2);
                assert!(installer.inner.profile.shared_worker_wheels.is_empty());
            } else if install_id == WINDOWS_ZIMAGE_INSTALL_ID {
                assert_eq!(
                    installer.inner.runtime.id,
                    studio_core::generation::Z_IMAGE_WINDOWS_RUNTIME
                );
                assert_eq!(
                    installer.inner.profile.engine,
                    studio_core::generation::Z_IMAGE_WINDOWS_ENGINE
                );
            } else {
                assert_eq!(
                    installer.inner.runtime.id,
                    studio_core::generation::WINDOWS_V2_RUNTIME
                );
                assert_eq!(
                    installer.inner.profile.engine,
                    studio_core::generation::WINDOWS_DIFFUSERS_ENGINE
                );
                assert_eq!(
                    installer.inner.profile.shared_worker_wheels.len(),
                    2,
                    "{install_id} shares the v2 runtime"
                );
                let staged = matches!(
                    install_id,
                    WINDOWS_QWEN_IMAGE_21_INSTALL_ID
                        | WINDOWS_QWEN_IMAGE_21_TURBO_INSTALL_ID
                        | WINDOWS_MOSS_IMAGE_SFT_INSTALL_ID
                );
                assert_eq!(installer.inner.profile.prepare_with_model, staged);
                assert_eq!(installer.inner.profile.prepared_cache_reserve != 0, staged);
            }
        }
    }
    #[cfg(windows)]
    fn install_device_expectation(install_id: &str) -> &'static str {
        if install_id == WINDOWS_BIREFNET_INSTALL_ID {
            "windows-cpu"
        } else {
            "gpu"
        }
    }
    #[cfg(windows)]
    #[test]
    fn windows_shared_runtime_embeds_both_frozen_worker_wheels() {
        for install_id in [
            WINDOWS_KLEIN_INSTALL_ID,
            WINDOWS_BIREFNET_INSTALL_ID,
            WINDOWS_QWEN_IMAGE_21_INSTALL_ID,
            WINDOWS_QWEN_IMAGE_21_TURBO_INSTALL_ID,
            WINDOWS_MOSS_IMAGE_SFT_INSTALL_ID,
        ] {
            let profile = windows_profile(install_id);
            let temp = tempfile::tempdir().unwrap();
            let installer =
                Installation::new_for(AppPaths::resolve(Some(temp.path())).unwrap(), profile)
                    .unwrap();
            let wheels = installer.runtime_wheels();
            let expected = [
                (
                    "studio_diffusers_worker-0.1.0-py3-none-any.whl",
                    WINDOWS_DIFFUSERS_WORKER_WHEEL,
                ),
                (
                    "studio_onnx_worker-0.1.0-py3-none-any.whl",
                    WINDOWS_ONNX_WORKER_WHEEL,
                ),
            ];
            for (path, bytes) in expected {
                let wheel = wheels
                    .iter()
                    .find(|wheel| wheel.path == path)
                    .unwrap_or_else(|| panic!("{install_id} misses {path}"));
                assert_eq!(wheel.url, "bundled:worker-wheel");
                assert_eq!(wheel.bytes, bytes.len() as u64);
                assert_eq!(wheel.sha256, format!("{:x}", Sha256::digest(bytes)));
                assert_eq!(installer.bundled_worker_wheel_bytes(path), Some(bytes));
            }
            // Bundled wheels are handled exactly once even though every model shares them.
            let downloads = installer.runtime_downloads();
            for (path, _) in expected {
                assert_eq!(
                    downloads
                        .iter()
                        .filter(|artifact| artifact.path == path)
                        .count(),
                    1
                );
            }
        }
    }
    #[cfg(windows)]
    #[test]
    fn windows_embedded_worker_wheel_identity_is_bound_to_the_ready_record() {
        let profile = WINDOWS_KLEIN_PROFILE;
        let temp = tempfile::tempdir().unwrap();
        let install =
            Installation::new_for(AppPaths::resolve(Some(temp.path())).unwrap(), profile).unwrap();
        let snapshot_id = install.snapshot_id();
        let mut record = ReadyRecord {
            runtime_lock_id: install.inner.runtime.id.clone(),
            model_snapshot_id: snapshot_id.clone(),
            runtime_manifest_sha256: manifest_digest(profile.runtime_manifest),
            model_manifest_sha256: manifest_digest(profile.model_manifest),
            licenses: license_records(profile),
            prepared_dir: None,
            prepared_files: Vec::new(),
            prepared_digest: None,
            worker_wheels: bundled_worker_wheel_records(profile),
            prepare_preflight: None,
            runtime_files: vec![],
            probe: serde_json::Value::Null,
        };
        assert!(
            ready_identity_error(&record, profile, &install.inner.runtime.id, &snapshot_id)
                .is_none()
        );
        assert!(worker_wheels_match(profile, &record.worker_wheels));
        // A record from an older app build (serde default: no wheel identity) must
        // mark the installation stale instead of reusing the runtime loop.
        record.worker_wheels.clear();
        assert!(!worker_wheels_match(profile, &record.worker_wheels));
        let error = ready_identity_error(&record, profile, &install.inner.runtime.id, &snapshot_id)
            .unwrap();
        assert!(error.contains("worker wheel"), "{error}");
        // A different wheel build (changed sha) is stale too.
        record.worker_wheels = bundled_worker_wheel_records(profile);
        record.worker_wheels[0].sha256 = "0".repeat(64);
        assert!(!worker_wheels_match(profile, &record.worker_wheels));
        // Startup reports unavailable with the actionable reinstall message.
        let ready_path = install.inner.paths.data.join(profile.ready_relative);
        ensure_child_dir(&install.inner.paths.data, ready_path.parent().unwrap()).unwrap();
        atomic_json(&ready_path, &record).unwrap();
        let reloaded =
            Installation::new_for(AppPaths::resolve(Some(temp.path())).unwrap(), profile).unwrap();
        let snapshot = reloaded.snapshot();
        assert_eq!(snapshot.phase, "unavailable");
        assert!(snapshot.error.unwrap().contains("worker wheel"));
        // Restoring the correct identity returns the installation to the normal
        // metadata-only startup path (not ready yet because no runtime is installed).
        record.worker_wheels = bundled_worker_wheel_records(profile);
        atomic_json(&ready_path, &record).unwrap();
        let reloaded =
            Installation::new_for(AppPaths::resolve(Some(temp.path())).unwrap(), profile).unwrap();
        assert_eq!(reloaded.snapshot().phase, "unavailable");
        assert!(
            reloaded
                .snapshot()
                .error
                .as_deref()
                .is_some_and(|error| !error.contains("worker wheel"))
        );
    }
    #[cfg(windows)]
    #[test]
    fn windows_embedded_worker_wheels_match_the_frozen_manifest_identities() {
        let manifest: serde_json::Value =
            serde_json::from_str(WINDOWS_V2_RUNTIME_MANIFEST).unwrap();
        let embedded = manifest["worker_wheels_embedded_by_installer"]["wheels"]
            .as_array()
            .unwrap();
        assert_eq!(embedded.len(), WINDOWS_SHARED_WORKER_WHEELS.len());
        for wheel in embedded {
            let resource_path = wheel["path"].as_str().unwrap();
            let file_name = resource_path.rsplit('/').next().unwrap();
            let (_, bytes) = WINDOWS_SHARED_WORKER_WHEELS
                .iter()
                .find(|(candidate, _)| *candidate == file_name)
                .unwrap_or_else(|| panic!("manifest wheel is not embedded: {resource_path}"));
            assert_eq!(
                wheel["bytes"].as_u64().unwrap(),
                bytes.len() as u64,
                "{file_name}"
            );
            assert_eq!(
                wheel["sha256"].as_str().unwrap(),
                format!("{:x}", Sha256::digest(bytes)),
                "{file_name}"
            );
        }
    }
    #[cfg(windows)]
    #[test]
    fn windows_plan_environment_binds_the_frozen_runtime_engine_and_device() {
        for &(install_id, _) in WINDOWS_MODEL_BYTES {
            let profile = windows_profile(install_id);
            let temp = tempfile::tempdir().unwrap();
            let installer =
                Installation::new_for(AppPaths::resolve(Some(temp.path())).unwrap(), profile)
                    .unwrap();
            let snapshot_id = installer.snapshot_id();
            let record = ReadyRecord {
                runtime_lock_id: installer.inner.runtime.id.clone(),
                model_snapshot_id: snapshot_id,
                runtime_manifest_sha256: manifest_digest(profile.runtime_manifest),
                model_manifest_sha256: manifest_digest(profile.model_manifest),
                licenses: license_records(profile),
                prepared_dir: None,
                prepared_files: vec![],
                prepared_digest: None,
                worker_wheels: bundled_worker_wheel_records(profile),
                prepare_preflight: None,
                runtime_files: vec![],
                probe: serde_json::json!({
                    "device": profile.probe_device,
                    "device_name": "fixture-device",
                }),
            };
            let ready_path = installer.inner.paths.data.join(profile.ready_relative);
            ensure_child_dir(&installer.inner.paths.data, ready_path.parent().unwrap()).unwrap();
            atomic_json(&ready_path, &record).unwrap();
            let plan = installer.plan_environment().unwrap();
            assert_eq!(plan.runtime.package_id, installer.inner.runtime.id);
            assert_eq!(plan.runtime.engine, profile.engine);
            assert_eq!(plan.device.device_id, plan_device_id(install_id).unwrap());
            assert_eq!(plan.device.precision, profile.precision);
            assert_eq!(plan.scheduler, profile.scheduler);
            assert!(!plan.components.is_empty());
        }
    }
    #[cfg(windows)]
    #[test]
    fn windows_birefnet_and_realesrgan_archive_their_licenses() {
        for (install_id, expected_paths) in [
            (WINDOWS_BIREFNET_INSTALL_ID, vec!["LICENSE-BiRefNet.txt"]),
            (
                WINDOWS_REALESRGAN_INSTALL_ID,
                vec!["LICENSE-RealESRGAN.txt", "LICENSE-ncnn-app.txt"],
            ),
        ] {
            let profile = windows_profile(install_id);
            let temp = tempfile::tempdir().unwrap();
            let installer =
                Installation::new_for(AppPaths::resolve(Some(temp.path())).unwrap(), profile)
                    .unwrap();
            let records = installer.install_licenses().unwrap();
            assert_eq!(
                records
                    .iter()
                    .map(|record| record.path.as_str())
                    .collect::<Vec<_>>(),
                expected_paths
            );
            let root = license_root(&installer.inner.paths.data, profile);
            for record in &records {
                verify(&root.join(&record.path), record.bytes, &record.sha256).unwrap();
            }
            assert!(licenses_healthy(&installer.inner.paths.data, profile).is_ok());
            // License bytes are small but counted in the install plan.
            assert!(installer.snapshot().required_bytes > installer.snapshot().total_bytes * 2);
        }
    }
    #[test]
    fn range_requires_exact_start_end_and_total() {
        assert!(valid_range(Some("bytes 5-9/10"), 5, 10, 5));
        assert!(!valid_range(Some("bytes 0-9/10"), 5, 10, 5));
        assert!(!valid_range(Some("bytes 5-8/9"), 5, 10, 4));
    }

    #[test]
    fn segmented_ranges_are_complete_disjoint_and_resumeable() {
        assert_eq!(
            segment_ranges(10, 3).unwrap(),
            vec![(0, 4), (4, 8), (8, 10)]
        );
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.part");
        fs::write(&source, b"0123456").unwrap();
        let segments = vec![
            root.path().join("segment-0"),
            root.path().join("segment-1"),
            root.path().join("segment-2"),
        ];
        assert_eq!(
            seed_segments(&source, &segment_ranges(10, 3).unwrap(), &segments).unwrap(),
            7
        );
        assert_eq!(fs::read(&segments[0]).unwrap(), b"0123");
        assert_eq!(fs::read(&segments[1]).unwrap(), b"456");
        assert_eq!(fs::read(&segments[2]).unwrap(), b"");
    }
    #[test]
    fn corrupt_segment_state_is_cleared_for_an_explicit_redownload() {
        let root = tempfile::tempdir().unwrap();
        let part = root.path().join("artifact.part");
        let seed_marker = root.path().join("artifact.segments-seeded");
        let segments = vec![
            root.path().join("artifact.segment-00"),
            root.path().join("artifact.segment-01"),
        ];
        let metadata = vec![
            root.path().join("artifact.segment-00.json"),
            root.path().join("artifact.segment-01.json"),
        ];
        fs::write(&part, b"corrupt").unwrap();
        fs::write(&seed_marker, b"seeded").unwrap();
        for path in segments.iter().chain(&metadata) {
            fs::write(path, b"partial").unwrap();
        }
        clear_segment_state(&part, &segments, &metadata, &seed_marker).unwrap();
        assert!(!part.exists());
        assert!(!seed_marker.exists());
        assert!(segments.iter().chain(&metadata).all(|path| !path.exists()));
    }
    #[test]
    fn installation_space_accounts_for_separate_and_shared_volumes() {
        assert!(installation_space_available(100, 100, 60, 70, 20, 5, false));
        assert!(!installation_space_available(100, 100, 60, 70, 20, 5, true));
        assert!(installation_space_available(200, 100, 60, 70, 20, 5, true));
        assert!(!installation_space_available(60, 100, 60, 70, 20, 5, false));
        assert!(!installation_space_available(100, 20, 60, 70, 20, 5, false));
    }
    #[test]
    fn activation_records_bind_bytes_even_when_version_labels_are_reused() {
        let (_temp, i) = fixture();
        let mut record = ReadyRecord {
            runtime_lock_id: i.inner.runtime.id.clone(),
            model_snapshot_id: i.inner.model.id.clone(),
            runtime_manifest_sha256: manifest_digest(RUNTIME_MANIFEST),
            model_manifest_sha256: manifest_digest(MODEL_MANIFEST),
            licenses: vec![],
            prepared_dir: None,
            prepared_files: vec![],
            prepared_digest: None,
            worker_wheels: vec![],
            prepare_preflight: None,
            runtime_files: vec![],
            probe: serde_json::json!({"device":"gpu","device_name":"fixture-device"}),
        };
        let ready = i.inner.paths.data.join("installation-ready.json");
        assert!(ready_record_matches(
            &record,
            Z_PROFILE,
            &i.inner.runtime.id,
            &i.inner.model.id
        ));
        record.runtime_manifest_sha256 = "a".repeat(64);
        atomic_json(&ready, &record).unwrap();
        assert!(i.plan_environment().is_err());
        assert!(i.ready_paths().unwrap_err().contains("安装身份已变化"));
        assert_eq!(
            Installation::new(i.inner.paths.clone())
                .unwrap()
                .snapshot()
                .phase,
            "unavailable"
        );
        record.runtime_manifest_sha256 = manifest_digest(RUNTIME_MANIFEST);
        record.model_manifest_sha256.clear();
        atomic_json(&ready, &record).unwrap();
        assert!(i.plan_environment().is_err());
        assert!(i.ready_paths().is_err());
    }
    #[test]
    fn paths_urls_and_hashes_fail_closed() {
        for p in ["/tmp/escape", "../escape", "a/../../escape", ""] {
            assert!(safe_relative(p).is_err());
        }
        assert!(!allowed_url(
            &reqwest::Url::parse("https://huggingface.co.evil.test/a").unwrap()
        ));
        assert!(!allowed_url(
            &reqwest::Url::parse("http://huggingface.co/a").unwrap()
        ));
        assert!(allowed_url(
            &reqwest::Url::parse("https://us.aws.cdn.hf.co/official-object").unwrap()
        ));
        assert!(allowed_url(
            &reqwest::Url::parse("https://us.gcp.cdn.hf.co/official-object").unwrap()
        ));
        assert!(allowed_url(
            &reqwest::Url::parse("https://www.modelscope.cn/models/Qwen/model").unwrap()
        ));
        assert!(allowed_url(
            &reqwest::Url::parse("https://cdn-lfs-cn-1.modelscope.cn/prod/object").unwrap()
        ));
        assert!(!allowed_url(
            &reqwest::Url::parse("https://us.aws.cdn.hf.co.evil.test/object").unwrap()
        ));
        assert!(!allowed_url(
            &reqwest::Url::parse("https://evilcdn.hf.co.evil.test/object").unwrap()
        ));
        assert!(!allowed_url(
            &reqwest::Url::parse("https://modelscope.cn.evil.test/model").unwrap()
        ));
        assert!(!allowed_url(
            &reqwest::Url::parse("https://evilmodelscope.cn/model").unwrap()
        ));
        assert!(!allowed_url(
            &reqwest::Url::parse("http://www.modelscope.cn/model").unwrap()
        ));
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a");
        fs::write(&path, b"hello").unwrap();
        assert!(verify(&path, 5, &format!("{:x}", Sha256::digest(b"hello"))).is_ok());
        assert!(verify(&path, 5, &"0".repeat(64)).is_err());
    }
}
