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
const QWEN_IMAGE_21_MODEL_MANIFEST: &str =
    include_str!("../../../../resources/planned-models/qwen21/model-manifest.json");
const QWEN_IMAGE_21_TURBO_MODEL_MANIFEST: &str =
    include_str!("../../../../resources/planned-models/qwen21-turbo/model-manifest.json");
const MOSS_IMAGE_SFT_MODEL_MANIFEST: &str =
    include_str!("../../../../resources/planned-models/moss-image-sft/model-manifest.json");
const RECIPES_V3_RUNTIME_MANIFEST: &str =
    include_str!("../../../../resources/planned-models/qwen21/runtime-manifest.json");
const RECIPES_V3_LOCK: &str =
    include_str!("../../../../resources/planned-models/qwen21/requirements.lock");
const RECIPES_V3_WORKER_WHEEL: &[u8] = include_bytes!(
    "../../../../resources/planned-models/qwen21/studio_mflux_worker_v3-0.3.0-py3-none-any.whl"
);
const RESERVE: u64 = 4 * 1024 * 1024 * 1024;
const SEGMENTED_DOWNLOAD_THRESHOLD: u64 = 2 * 1024 * 1024 * 1024;
const DOWNLOAD_SEGMENTS: usize = 8;

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
}

struct SegmentDownload<'a> {
    url: &'a str,
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
};

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
};

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
};

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
    legacy: Installation,
    klein: Installation,
    birefnet: Installation,
    realesrgan: Installation,
    qwen_image_21: Installation,
    qwen_image_21_turbo: Installation,
    moss_image_sft: Installation,
}

impl Installations {
    pub fn new(paths: AppPaths) -> Result<Self, String> {
        Ok(Self {
            legacy: Installation::new(paths.clone())?,
            klein: Installation::new_klein(paths.clone())?,
            birefnet: Installation::new_birefnet(paths.clone())?,
            realesrgan: Installation::new_realesrgan(paths.clone())?,
            qwen_image_21: Installation::new_for(paths.clone(), QWEN_IMAGE_21_PROFILE)?,
            qwen_image_21_turbo: Installation::new_for(paths.clone(), QWEN_IMAGE_21_TURBO_PROFILE)?,
            moss_image_sft: Installation::new_for(paths, MOSS_IMAGE_SFT_PROFILE)?,
        })
    }

    pub fn snapshots(&self) -> Vec<InstallationSnapshot> {
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
                "preparing" | "downloading" | "verifying" | "installing_runtime" | "probing"
            )
        })
    }

    pub fn shutdown(&self) {
        self.legacy.cancel();
        self.klein.cancel();
        self.birefnet.cancel();
        self.realesrgan.cancel();
        self.qwen_image_21.cancel();
        self.qwen_image_21_turbo.cancel();
        self.moss_image_sft.cancel();
    }

    fn for_id(&self, install_id: &str) -> Option<&Installation> {
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
#[derive(Serialize, Deserialize)]
struct ReadyRecord {
    runtime_lock_id: String,
    model_snapshot_id: String,
    #[serde(default)]
    runtime_manifest_sha256: String,
    #[serde(default)]
    model_manifest_sha256: String,
    runtime_files: Vec<InventoryEntry>,
    probe: serde_json::Value,
}
impl ReadyRecord {
    fn matches_profile(&self, profile: InstallProfile) -> bool {
        self.runtime_manifest_sha256 == manifest_digest(profile.runtime_manifest)
            && self.model_manifest_sha256 == manifest_digest(profile.model_manifest)
    }
}
fn ready_record_matches(
    record: &ReadyRecord,
    profile: InstallProfile,
    runtime_id: &str,
    model_id: &str,
) -> bool {
    record.runtime_lock_id == runtime_id
        && record.model_snapshot_id == model_id
        && record.matches_profile(profile)
}
fn manifest_digest(manifest: &str) -> String {
    format!("{:x}", Sha256::digest(manifest.as_bytes()))
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
        let runtime_downloads = runtime.downloads();
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
            if runtime.platform != "macos-universal2"
                || runtime.executable.is_none()
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
        // Initial upper bound is available before any network or large hashing:
        // full download cache + target model copy + runtime expansion + reserve.
        // start() refines missing cache bytes and checks both volumes separately.
        let initial_required_bytes = model_file_bytes * 2 + runtime_bytes * 7 + RESERVE;
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
            if ready_record_matches(&record, profile, &runtime.id, &model.id) {
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
                    [
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
                    ]
                    .iter()
                    .all(|path| {
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
                    && record.probe["identity"]["model_snapshot_id"] == model.id
                    && record.probe["device_name"]
                        .as_str()
                        .is_some_and(|s| !s.is_empty())
                    && !record.runtime_files.is_empty()
                    && model.installed_files().iter().all(|file| {
                        fs::symlink_metadata(model_root.join(&file.path)).is_ok_and(|metadata| {
                            metadata.is_file() && metadata.len() == file.bytes
                        })
                    })
                    && runtime_critical_ok;
                if healthy {
                    state.phase = "ready".into();
                    state.runtime_ready = true;
                    state.model_ready = true;
                    state.device_verified = true;
                } else {
                    state.phase = "unavailable".into();
                    state.error = Some("安装文件缺失或身份异常，请修复安装。".into());
                }
            } else {
                state.phase = "unavailable".into();
                state.error = Some("安装清单已变化或记录缺少字节身份，请修复安装。".into());
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
            &self.inner.model.id,
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
                device_id: device.into(),
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
        if !ready_record_matches(
            &record,
            self.inner.profile,
            &self.inner.runtime.id,
            &self.inner.model.id,
        ) {
            return Err("安装身份已变化，请重新安装。".into());
        }
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
                arguments.push(self.inner.model.id.clone());
            }
            (self.runtime_dir().join("venv/bin/python3"), arguments)
        };
        Ok(ReadyPaths {
            worker_program,
            worker_args,
            model_path: self.model_dir(),
            worker_module: self.inner.profile.worker_module.into(),
            worker_build_id: self.inner.profile.worker_build_id.into(),
            runtime_lock_id: self.inner.runtime.id.clone(),
            model_snapshot_id: self.inner.model.id.clone(),
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
        all.extend(self.inner.runtime.downloads());
        let missing: u64 = all
            .iter()
            .filter(|a| !self.cache().join(&a.sha256).exists())
            .map(|a| a.bytes)
            .sum();
        let target = self.inner.model.component_bytes;
        // Conservative full target copy and private expansion budget. Cache and data
        // may share a volume, in which case both peaks must fit simultaneously.
        let runtime_download_bytes = self
            .inner
            .runtime
            .downloads()
            .iter()
            .map(|artifact| artifact.bytes)
            .sum::<u64>();
        let runtime_budget =
            runtime_download_bytes * if self.inner.runtime.is_native() { 3 } else { 6 };
        let required = missing + target + runtime_budget + RESERVE;
        self.update(|s| s.required_bytes = required);
        // Check both volumes independently; never rely on cross-volume rename.
        let data_free = fs2::available_space(&self.inner.paths.data).map_err(err)?;
        let cache_free = fs2::available_space(&self.inner.paths.cache).map_err(err)?;
        if !installation_space_available(
            data_free,
            cache_free,
            missing,
            target,
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
            &serde_json::json!({"runtime_lock_id":self.inner.runtime.id,"model_snapshot_id":self.inner.model.id}),
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
        let record = self.prepare_runtime()?;
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
        // Never execute a leftover, unverified interpreter from an interrupted install.
        // Only this fixed private identity is rebuilt; other activated versions survive.
        fs::remove_dir_all(&runtime).map_err(err)?;
        ensure_child_dir(&self.inner.paths.data, &runtime)?;
        extract(&self.cache().join(&interpreter.sha256), &runtime)?;
        let base_python = runtime.join("python/bin/python3");
        self.check_cancel()?;
        self.command(
            Command::new(&base_python)
                .args(["-I", "-m", "venv", "--clear"])
                .arg(runtime.join("venv")),
        )?;
        let python = runtime.join("venv/bin/python3");
        let wheelhouse = runtime.join("wheelhouse");
        ensure_child_dir(&self.inner.paths.data, &wheelhouse)?;
        for wheel in &self.inner.runtime.wheels {
            copy_verified(
                &self.cache().join(&wheel.sha256),
                &wheelhouse.join(&wheel.path),
                wheel,
                &self.inner.paths.data,
            )?;
        }
        fs::write(
            runtime.join("requirements.lock"),
            self.inner.profile.requirements_lock,
        )
        .map_err(err)?;
        self.command(
            Command::new(&python)
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
                .arg(runtime.join("requirements.lock")),
        )?;
        self.check_cancel()?;
        self.update(|s| s.phase = "probing".into());
        let mut probe = Command::new(&python);
        probe.args(["-I", "-m", self.inner.profile.worker_module]);
        if self.inner.profile.worker_model_argument {
            probe.arg("--model-snapshot-id").arg(&self.inner.model.id);
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
            || probe["identity"]["model_snapshot_id"] != self.inner.model.id
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
        let mut runtime_files = Vec::new();
        inventory(&runtime, &runtime, &mut runtime_files)?;
        self.check_cancel()?;
        let record = ReadyRecord {
            runtime_lock_id: self.inner.runtime.id.clone(),
            model_snapshot_id: self.inner.model.id.clone(),
            runtime_manifest_sha256: manifest_digest(self.inner.profile.runtime_manifest),
            model_manifest_sha256: manifest_digest(self.inner.profile.model_manifest),
            runtime_files,
            probe,
        };
        Ok(record)
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
        self.command(
            Command::new(&ncnn)
                .args([
                    "-i",
                    input.to_str().ok_or("探针输入路径不是 UTF-8")?,
                    "-o",
                    output.to_str().ok_or("探针输出路径不是 UTF-8")?,
                    "-s",
                    "4",
                    "-t",
                    "128",
                    "-m",
                    model.to_str().ok_or("模型路径不是 UTF-8")?,
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
            model_snapshot_id: self.inner.model.id.clone(),
            runtime_manifest_sha256: manifest_digest(self.inner.profile.runtime_manifest),
            model_manifest_sha256: manifest_digest(self.inner.profile.model_manifest),
            runtime_files,
            probe: serde_json::json!({
                "identity": {
                    "build_id": self.inner.profile.worker_build_id,
                    "runtime_lock_id": self.inner.runtime.id,
                    "model_snapshot_id": self.inner.model.id,
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
        let deadline = std::time::Instant::now() + Duration::from_secs(600);
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
            let mut file = open_regular(&final_path, true, true)?;
            file.write_all(self.inner.profile.worker_wheel)
                .map_err(err)?;
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
            self.download_segment(
                &client,
                SegmentDownload {
                    url: &a.url,
                    path: &path,
                    metadata_path: &metadata_path,
                    sha256: &a.sha256,
                    etag: &etag,
                    start,
                    end,
                    total: a.bytes,
                },
            )
            .await?;
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
        if response.status() != reqwest::StatusCode::PARTIAL_CONTENT
            || response
                .headers()
                .get(reqwest::header::ETAG)
                .and_then(|value| value.to_str().ok())
                != Some(etag)
            || !valid_range(
                response
                    .headers()
                    .get(reqwest::header::CONTENT_RANGE)
                    .and_then(|value| value.to_str().ok()),
                request_start,
                total,
                end - request_start,
            )
        {
            return Err("分段 Range 响应身份或范围不符。".into());
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
            let Some(chunk) = self.cancellable(response.chunk()).await? else {
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
                    .into_owned(),
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
                    .into_owned(),
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
        for artifact in &manifest.files {
            assert!(artifact.url.starts_with(&format!(
                "https://huggingface.co/Tongyi-MAI/Z-Image-Turbo/resolve/{}/",
                manifest.revision
            )));
            assert_eq!(artifact.sha256.len(), 64);
            assert!(artifact.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()));
            assert!(artifact.bytes > 0);
        }
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
        assert_eq!(lock["bytes"], RUNTIME_LOCK.len() as u64);
        assert_eq!(
            lock["sha256"],
            format!("{:x}", Sha256::digest(RUNTIME_LOCK.as_bytes()))
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
        let (temp, installer) = fixture_for(Installation::new_realesrgan);
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
        let (temp, installer) = fixture_for(Installation::new_realesrgan);
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
