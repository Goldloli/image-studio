//! Business facade for the local webview. Blocking work is called through spawn_blocking.
use crate::adapters::{
    AppPaths,
    catalog::ModelCatalog,
    device::DeviceFacts,
    feature_catalog::FeatureCatalog,
    generation::GenerationController,
    installation::{
        BIREFNET_INSTALL_ID, INSTALL_ID, InstallationSnapshot, Installations, KLEIN_INSTALL_ID,
        MOSS_IMAGE_SFT_INSTALL_ID, QWEN_IMAGE_21_INSTALL_ID, QWEN_IMAGE_21_TURBO_INSTALL_ID,
        REALESRGAN_INSTALL_ID, WINDOWS_ZIMAGE_INSTALL_ID,
    },
    library::{
        AcceptTurnInput, AssetLibraryQuery, DEFAULT_MODEL_ID, DraftRecord, JobRecord, Library,
        TaskCenterQuery, TurnRecord, TurnStatus,
    },
    preferences::{AppearancePreferences, CreationDefaults, PreferencesStore, valid_seed},
    storage,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use studio_core::{
    domain::*,
    generation::{
        GenerationInput, Z_IMAGE_WINDOWS_CAPABILITY, freeze_windows_z_image,
        freeze_windows_z_image_compact, freeze_z_image,
    },
    recipes::{
        BiRefNetInput, KleinInput, MOSS_IMAGE_SFT_CAPABILITY, MOSS_IMAGE_SFT_MODEL,
        QWEN_IMAGE_21_CAPABILITY, QWEN_IMAGE_21_MODEL, QWEN_IMAGE_21_TURBO_CAPABILITY,
        QWEN_IMAGE_21_TURBO_MODEL, RealEsrganInput, Recipe, SourceAsset, SourceFacts,
        TextModelInput, freeze_birefnet, freeze_klein, freeze_realesrgan, freeze_text_model,
        freeze_windows_birefnet, freeze_windows_klein, freeze_windows_klein_compact,
        freeze_windows_realesrgan, freeze_windows_text_model,
    },
};
use uuid::Uuid;

/// Windows parity model ids from design.md section 11. Each id is also its install id. These are
/// plain frozen strings: `prepare_turn` references them on every host (they are unreachable on
/// macOS, which keeps its own recipe ids), while the UI/validation projections are cfg-gated.
const WINDOWS_KLEIN_MODEL: &str = studio_core::generation::KLEIN_WINDOWS_MODEL;
const WINDOWS_BIREFNET_MODEL: &str = studio_core::generation::BIREFNET_WINDOWS_MODEL;
const WINDOWS_REALESRGAN_MODEL: &str = studio_core::generation::REALESRGAN_WINDOWS_MODEL;
const WINDOWS_QWEN_MODEL: &str = studio_core::generation::QWEN_IMAGE_21_WINDOWS_MODEL;
const WINDOWS_QWEN_TURBO_MODEL: &str = studio_core::generation::QWEN_IMAGE_21_TURBO_WINDOWS_MODEL;
const WINDOWS_MOSS_MODEL: &str = studio_core::generation::MOSS_IMAGE_SFT_WINDOWS_MODEL;

pub struct StudioService {
    pub paths: AppPaths,
    pub catalog: Result<ModelCatalog, String>,
    pub features: Result<FeatureCatalog, String>,
    pub preferences: PreferencesStore,
    pub library: Option<Arc<Mutex<Library>>>,
    pub installation: Option<Installations>,
    pub generation: Option<GenerationController>,
    pub startup_error: Option<String>,
    pub closing: AtomicBool,
    /// Local device facts used by every hardware gate (catalog projection, install start and
    /// generation send). Probed once per host process via `DeviceFacts::cached`; tests inject
    /// deterministic values here instead of depending on the CI machine.
    device_facts: DeviceFacts,
    scheduling: Mutex<()>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftInput {
    pub id: u64,
    pub name: String,
    pub prompt: String,
    pub params: Value,
    pub collapsed: bool,
    pub prompt_source_id: Option<String>,
}

pub struct SubmitInput {
    pub draft_id: u64,
    pub prompt: String,
    pub aspect: String,
    pub count: u32,
    pub seed: String,
    pub submission_key: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationSendInput {
    pub conversation_id: String,
    pub draft_id: u64,
    pub expected_version: u64,
    pub prompt: String,
    pub model_id: String,
    #[serde(default)]
    pub capability_id: Option<String>,
    #[serde(default)]
    pub source_asset_id: Option<String>,
    pub aspect: String,
    pub count: u32,
    pub seed: String,
    pub submission_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftParams {
    pub model_id: String,
    pub aspect: String,
    pub count: u32,
    pub seed: String,
    #[serde(default = "default_capability_id")]
    pub capability_id: String,
    #[serde(default)]
    pub source_asset_id: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub capability_drafts: std::collections::BTreeMap<String, CapabilityDraft>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityDraft {
    pub prompt: String,
    pub aspect: String,
    pub seed: String,
    pub source_asset_id: Option<String>,
    pub prompt_source_id: Option<String>,
}

fn default_capability_id() -> String {
    #[cfg(windows)]
    return studio_core::generation::Z_IMAGE_WINDOWS_CAPABILITY.into();
    #[cfg(not(windows))]
    studio_core::generation::Z_IMAGE_CAPABILITY.into()
}

fn capability_for_model(model_id: &str) -> &'static str {
    #[cfg(windows)]
    match model_id {
        WINDOWS_KLEIN_MODEL => return studio_core::generation::KLEIN_WINDOWS_TEXT_CAPABILITY,
        WINDOWS_BIREFNET_MODEL => return studio_core::generation::BIREFNET_WINDOWS_CAPABILITY,
        WINDOWS_REALESRGAN_MODEL => return studio_core::generation::REALESRGAN_WINDOWS_CAPABILITY,
        WINDOWS_QWEN_MODEL => return studio_core::generation::QWEN_IMAGE_21_WINDOWS_CAPABILITY,
        WINDOWS_QWEN_TURBO_MODEL => {
            return studio_core::generation::QWEN_IMAGE_21_TURBO_WINDOWS_CAPABILITY;
        }
        WINDOWS_MOSS_MODEL => return studio_core::generation::MOSS_IMAGE_SFT_WINDOWS_CAPABILITY,
        _ => {}
    }
    match model_id {
        WINDOWS_ZIMAGE_INSTALL_ID => Z_IMAGE_WINDOWS_CAPABILITY,
        INSTALL_ID => studio_core::generation::Z_IMAGE_CAPABILITY,
        KLEIN_INSTALL_ID => studio_core::recipes::KLEIN_TEXT_CAPABILITY,
        BIREFNET_INSTALL_ID => studio_core::recipes::BIREFNET_CAPABILITY,
        REALESRGAN_INSTALL_ID => studio_core::recipes::REALESRGAN_CAPABILITY,
        QWEN_IMAGE_21_INSTALL_ID => QWEN_IMAGE_21_CAPABILITY,
        QWEN_IMAGE_21_TURBO_INSTALL_ID => QWEN_IMAGE_21_TURBO_CAPABILITY,
        MOSS_IMAGE_SFT_INSTALL_ID => MOSS_IMAGE_SFT_CAPABILITY,
        _ => {
            #[cfg(windows)]
            return Z_IMAGE_WINDOWS_CAPABILITY;
            #[cfg(not(windows))]
            studio_core::generation::Z_IMAGE_CAPABILITY
        }
    }
}

/// The single-image edit capability when the model has one; used to keep the composer's
/// capability consistent with an attached source image.
fn klein_edit_capability(model_id: &str) -> Option<&'static str> {
    if model_id == KLEIN_INSTALL_ID {
        return Some(studio_core::recipes::KLEIN_EDIT_CAPABILITY);
    }
    #[cfg(windows)]
    if model_id == WINDOWS_KLEIN_MODEL {
        return Some(studio_core::generation::KLEIN_WINDOWS_EDIT_CAPABILITY);
    }
    None
}

/// Tool capabilities consume a source image only: no instruction and no seed.
fn is_source_only_tool_capability(capability: &str) -> bool {
    if matches!(
        capability,
        studio_core::recipes::BIREFNET_CAPABILITY | studio_core::recipes::REALESRGAN_CAPABILITY
    ) {
        return true;
    }
    #[cfg(windows)]
    if matches!(
        capability,
        studio_core::generation::BIREFNET_WINDOWS_CAPABILITY
            | studio_core::generation::REALESRGAN_WINDOWS_CAPABILITY
    ) {
        return true;
    }
    false
}

fn is_source_only_tool_model(model_id: &str) -> bool {
    if matches!(
        model_id,
        studio_core::recipes::BIREFNET_MODEL | studio_core::recipes::REALESRGAN_MODEL
    ) {
        return true;
    }
    #[cfg(windows)]
    if matches!(model_id, WINDOWS_BIREFNET_MODEL | WINDOWS_REALESRGAN_MODEL) {
        return true;
    }
    false
}

fn model_accepts_aspect(model_id: &str, aspect: &str) -> bool {
    match model_id {
        INSTALL_ID => matches!(aspect, "1:1" | "4:3" | "3:4"),
        WINDOWS_ZIMAGE_INSTALL_ID
        | KLEIN_INSTALL_ID
        | BIREFNET_INSTALL_ID
        | REALESRGAN_INSTALL_ID
        | QWEN_IMAGE_21_INSTALL_ID
        | QWEN_IMAGE_21_TURBO_INSTALL_ID
        | MOSS_IMAGE_SFT_INSTALL_ID => aspect == "1:1",
        #[cfg(windows)]
        WINDOWS_KLEIN_MODEL
        | WINDOWS_BIREFNET_MODEL
        | WINDOWS_REALESRGAN_MODEL
        | WINDOWS_QWEN_MODEL
        | WINDOWS_QWEN_TURBO_MODEL
        | WINDOWS_MOSS_MODEL => aspect == "1:1",
        _ => false,
    }
}

fn install_display_name(install_id: &str) -> &'static str {
    #[cfg(windows)]
    match install_id {
        WINDOWS_KLEIN_MODEL => return "FLUX.2 Klein 4B distilled",
        WINDOWS_BIREFNET_MODEL => return "BiRefNet general epoch_244",
        WINDOWS_REALESRGAN_MODEL => return "Real-ESRGAN x4plus",
        WINDOWS_QWEN_MODEL => return "Qwen-Image 2.1",
        WINDOWS_QWEN_TURBO_MODEL => return "Qwen-Image 2.1 Turbo",
        WINDOWS_MOSS_MODEL => return "MOSS-Image-SFT",
        _ => {}
    }
    match install_id {
        KLEIN_INSTALL_ID => "FLUX.2 Klein 4B distilled",
        BIREFNET_INSTALL_ID => "BiRefNet general epoch_244",
        REALESRGAN_INSTALL_ID => "Real-ESRGAN x4plus",
        QWEN_IMAGE_21_INSTALL_ID => "Qwen-Image 2.1",
        QWEN_IMAGE_21_TURBO_INSTALL_ID => "Qwen-Image 2.1 Turbo",
        MOSS_IMAGE_SFT_INSTALL_ID => "MOSS-Image-SFT",
        _ => "Z-Image Turbo",
    }
}

fn fallback_model_bytes(install_id: &str) -> u64 {
    #[cfg(windows)]
    match install_id {
        WINDOWS_KLEIN_MODEL => return 15_980_131_745,
        WINDOWS_BIREFNET_MODEL => return 972_666_916,
        WINDOWS_REALESRGAN_MODEL => return 33_540_549,
        WINDOWS_QWEN_MODEL => return 33_131_609_424,
        WINDOWS_QWEN_TURBO_MODEL => return 32_451_607_098,
        WINDOWS_MOSS_MODEL => return 27_219_783_878,
        _ => {}
    }
    match install_id {
        WINDOWS_ZIMAGE_INSTALL_ID => 32_848_305_533,
        KLEIN_INSTALL_ID => 15_980_131_745,
        BIREFNET_INSTALL_ID => 972_666_916,
        REALESRGAN_INSTALL_ID => 33_540_549,
        QWEN_IMAGE_21_INSTALL_ID => 33_131_609_424,
        QWEN_IMAGE_21_TURBO_INSTALL_ID => 32_451_607_098,
        MOSS_IMAGE_SFT_INSTALL_ID => 27_219_783_878,
        _ => 32_848_304_893,
    }
}

/// The registered runtime lock id for an install profile. Windows shows the v1 and v2 runtime
/// rows separately in the settings page; macOS keeps its per-profile runtime.
fn runtime_lock_id(install_id: &str) -> Option<&'static str> {
    #[cfg(windows)]
    match install_id {
        WINDOWS_ZIMAGE_INSTALL_ID => {
            return Some(studio_core::generation::Z_IMAGE_WINDOWS_RUNTIME);
        }
        WINDOWS_REALESRGAN_MODEL => {
            return Some(studio_core::generation::REALESRGAN_WINDOWS_RUNTIME);
        }
        WINDOWS_KLEIN_MODEL
        | WINDOWS_BIREFNET_MODEL
        | WINDOWS_QWEN_MODEL
        | WINDOWS_QWEN_TURBO_MODEL
        | WINDOWS_MOSS_MODEL => return Some(studio_core::generation::WINDOWS_V2_RUNTIME),
        _ => {}
    }
    #[cfg(not(windows))]
    let _ = install_id;
    None
}

/// Host-side hardware gate shared by the install and generation entry points.
///
/// The webview disables below-threshold entry points, but the host must not rely on that: a direct
/// IPC call (or a frontend bug) could still ask to install or generate for a model whose
/// `minVramGb`/`minRamGb` the local device does not meet. This mirrors the catalog projection:
/// threshold `None`/`0` means "not required", unknown device facts fail closed inside
/// `DeviceFacts::gate`, and the returned text is the exact actionable Chinese reason the catalog
/// exposes (`hardwareReason`), so errors stay consistent between the model page and the host.
///
/// Unknown model ids are not judged here: they are not registered in the catalog and the install
/// or turn path reports its own unknown-id error.
/// The output size for one aspect at the selected tier. The compact tier renders a 768 square
/// and has no other registered aspect, so a 4:3 or 3:4 request must fail instead of silently
/// falling back to the 1024 standard recipe.
fn compact_aspect_size(aspect: &str, compact: bool) -> Result<(u32, u32), String> {
    match (aspect, compact) {
        ("1:1", true) => Ok((768, 768)),
        (_, true) => {
            Err("当前设备使用紧凑档（8 GB 显存），仅支持 1:1 方图；请把画幅改为 1:1。".into())
        }
        ("1:1", false) => Ok((1024, 1024)),
        ("4:3", false) => Ok((1024, 768)),
        ("3:4", false) => Ok((768, 1024)),
        _ => Err("当前模型不支持此画幅，请明确修改参数".into()),
    }
}

fn enforce_hardware_gate(
    catalog: &ModelCatalog,
    model_id: &str,
    device: &DeviceFacts,
) -> Result<(), String> {
    let Some(facts) = catalog.model_facts(model_id, device) else {
        return Ok(());
    };
    if facts.hardware.eligible {
        return Ok(());
    }
    Err(facts
        .hardware
        .reason
        .unwrap_or_else(|| "当前设备不满足该模型的硬件门槛，安装与生成入口已禁用。".into()))
}

impl Default for DraftParams {
    fn default() -> Self {
        Self {
            model_id: DEFAULT_MODEL_ID.into(),
            aspect: "1:1".into(),
            count: 1,
            seed: String::new(),
            capability_id: default_capability_id(),
            source_asset_id: None,
            capability_drafts: Default::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationCreateInput {
    pub model_id: Option<String>,
    pub aspect: Option<String>,
    pub count: Option<u32>,
    pub seed: Option<String>,
    pub initial_title: Option<String>,
    pub initial_prompt: Option<String>,
    pub prompt_source_id: Option<String>,
}

fn valid_prompt_source(value: &str) -> bool {
    value
        .strip_prefix("awesome-gpt-image-2:case:")
        .is_some_and(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()))
        && value.len() <= 128
}

impl StudioService {
    pub fn new(paths: AppPaths, supervisor: PathBuf) -> Self {
        let preferences = PreferencesStore::load(&paths.data);
        let catalog = ModelCatalog::load();
        let features = FeatureCatalog::load();
        let library = Library::open(
            &paths.data,
            vec![paths.cache.clone(), paths.logs.clone(), paths.work.clone()],
        );
        match library {
            Err(error) => Self {
                paths,
                catalog,
                features,
                preferences,
                library: None,
                installation: None,
                generation: None,
                startup_error: Some(error.to_string()),
                closing: AtomicBool::new(false),
                device_facts: DeviceFacts::cached(),
                scheduling: Mutex::new(()),
            },
            Ok(library) => {
                let library = Arc::new(Mutex::new(library));
                let installation = Installations::new(paths.clone());
                let startup_error = installation.as_ref().err().cloned();
                let service_paths = paths.clone();
                let generation = GenerationController::new(paths, library.clone(), supervisor);
                Self {
                    paths: service_paths,
                    catalog,
                    features,
                    preferences,
                    library: Some(library),
                    installation: installation.ok(),
                    generation: Some(generation),
                    startup_error,
                    closing: AtomicBool::new(false),
                    device_facts: DeviceFacts::cached(),
                    scheduling: Mutex::new(()),
                }
            }
        }
    }
    pub fn with_library<T>(
        &self,
        f: impl FnOnce(&mut Library) -> crate::adapters::library::Result<T>,
    ) -> Result<T, String> {
        let library = self.library.as_ref().ok_or_else(|| {
            self.startup_error
                .clone()
                .unwrap_or_else(|| "本地库不可用".into())
        })?;
        let mut guard = library
            .lock()
            .map_err(|_| "本地库锁异常，请重启并检查数据".to_string())?;
        f(&mut guard).map_err(|error| error.to_string())
    }
    /// Non-blocking variant for the watchdog: never waits for the library mutex.
    pub fn try_with_library<T>(
        &self,
        f: impl FnOnce(&mut Library) -> crate::adapters::library::Result<T>,
    ) -> Option<Result<T, String>> {
        let library = self.library.as_ref()?;
        let mut guard = library.try_lock().ok()?;
        Some(f(&mut guard).map_err(|error| error.to_string()))
    }

    /// Fail turns that were accepted but never reached a job.
    ///
    /// Returns how many were reaped. Safe to call from a watchdog thread at any time: it never
    /// blocks on the library lock and never touches an already-bound turn.
    pub fn reap_stale_pending_turns(&self, older_than_ms: i64) -> usize {
        let Some(Ok(ids)) =
            self.try_with_library(|library| library.stale_pending_turns(older_than_ms))
        else {
            return 0;
        };
        let mut reaped = 0;
        for id in ids {
            let Ok(turn_id) = TurnId::new(&id) else {
                continue;
            };
            let detail = format!(
                "发送超时：{} 秒内未进入执行（等待提交阶段），请重试；若反复出现请重启应用",
                older_than_ms / 1000
            );
            if matches!(
                self.try_with_library(|library| library.reject_turn(&turn_id, &detail)),
                Some(Ok(_))
            ) {
                reaped += 1;
            }
        }
        reaped
    }

    pub fn model_view(&self) -> Value {
        #[cfg(windows)]
        return self.model_view_for(WINDOWS_ZIMAGE_INSTALL_ID);
        #[cfg(not(windows))]
        self.model_view_for(INSTALL_ID)
    }
    pub fn model_views(&self) -> Vec<Value> {
        #[cfg(windows)]
        {
            [
                WINDOWS_ZIMAGE_INSTALL_ID,
                WINDOWS_KLEIN_MODEL,
                WINDOWS_BIREFNET_MODEL,
                WINDOWS_REALESRGAN_MODEL,
                WINDOWS_QWEN_MODEL,
                WINDOWS_QWEN_TURBO_MODEL,
                WINDOWS_MOSS_MODEL,
            ]
            .into_iter()
            .map(|id| self.model_view_for(id))
            .collect()
        }
        #[cfg(not(windows))]
        {
            [
                INSTALL_ID,
                KLEIN_INSTALL_ID,
                BIREFNET_INSTALL_ID,
                REALESRGAN_INSTALL_ID,
                QWEN_IMAGE_21_INSTALL_ID,
                QWEN_IMAGE_21_TURBO_INSTALL_ID,
                MOSS_IMAGE_SFT_INSTALL_ID,
            ]
            .into_iter()
            .map(|id| self.model_view_for(id))
            .collect()
        }
    }
    fn model_view_for(&self, install_id: &str) -> Value {
        let name = install_display_name(install_id);
        let fallback_model_bytes = fallback_model_bytes(install_id);
        let device = self.device_facts;
        let facts = self
            .catalog
            .as_ref()
            .ok()
            .and_then(|catalog| catalog.model_facts(install_id, &device));
        let hardware_eligible = facts.as_ref().is_none_or(|facts| facts.hardware.eligible);
        let hardware_reason = facts
            .as_ref()
            .and_then(|facts| facts.hardware.reason.clone());
        let metadata = facts
            .as_ref()
            .map(|facts| facts.metadata.clone())
            .unwrap_or_default();
        let tier = facts
            .as_ref()
            .map(|facts| facts.tier.clone())
            .unwrap_or_default();
        let static_facts = json!({
            "hardwareEligible": hardware_eligible,
            "hardwareReason": hardware_reason,
            "minVramGb": metadata.min_vram_gb,
            "minRamGb": metadata.min_ram_gb,
            "standardMinVramGb": tier.standard_min_vram_gb,
            "tier": tier.id,
            "resolution": tier.resolution,
            "computeDtype": tier.compute_dtype,
            "tierReason": tier.reason,
            "quantization": metadata.quantization,
            "licenseNotice": metadata.license_notice,
            "windowsStatus": metadata.windows_status,
            "runtimeLockId": runtime_lock_id(install_id),
            "deviceFacts": device.label()
        });
        let Some(s) = self
            .installation
            .as_ref()
            .and_then(|installation| installation.snapshot(install_id))
        else {
            return json!({"id":install_id,"name":name,"phase":"failed","summary":"安装清单不可用","ready":false,"runtimeReady":false,"modelReady":false,"deviceVerified":false,"receivedBytes":0,"totalBytes":0,"modelBytes":fallback_model_bytes,"runtimeBytes":0,"requiredBytes":0,"error":self.startup_error,"busy":false,"staticFacts":static_facts});
        };
        let busy = matches!(
            s.phase.as_str(),
            "preparing" | "downloading" | "verifying" | "installing_runtime" | "probing"
        );
        let summary = match s.phase.as_str() {
            "not_installed" => "尚未安装",
            "preparing" => "准备安装",
            "downloading" => "正在下载",
            "verifying" => "正在校验模型",
            "installing_runtime" => "准备隔离运行环境",
            "probing" => "检查运行环境与设备",
            "ready" => "模型已就绪",
            "interrupted" => "安装已中断，可继续",
            "cancelled" => "安装已停止，可继续",
            "unavailable" => "安装文件缺失或损坏",
            "failed" => "安装失败，可重试",
            _ => "检查安装状态",
        };
        let ready = s.phase == "ready" && s.runtime_ready && s.model_ready && s.device_verified;
        json!({"id":s.install_id,"name":name,"phase":s.phase,"summary":summary,"ready":ready,"available":ready&&hardware_eligible,"runtimeReady":s.runtime_ready,"modelReady":s.model_ready,"deviceVerified":s.device_verified,"receivedBytes":s.received_bytes,"totalBytes":s.total_bytes,"modelBytes":s.model_bytes,"runtimeBytes":s.runtime_bytes,"requiredBytes":s.required_bytes,"error":s.error,"busy":busy,"staticFacts":static_facts})
    }
    pub fn model_catalog(&self) -> Result<Value, String> {
        let catalog = self.catalog.as_ref().map_err(|error| error.clone())?;
        let installations = self
            .installation
            .as_ref()
            .map(Installations::snapshots)
            .unwrap_or_default();
        let installation_error = self.installation.is_none().then(|| {
            self.startup_error
                .clone()
                .unwrap_or_else(|| "安装器不可用".into())
        });
        catalog.query(
            &installations,
            installation_error.as_deref(),
            &self.device_facts,
        )
    }
    pub fn feature_catalog(&self) -> Result<Value, String> {
        let catalog = self.features.as_ref().map_err(|error| error.clone())?;
        let models = self.model_catalog()?;
        let busy = self
            .generation
            .as_ref()
            .is_some_and(GenerationController::is_busy);
        Ok(catalog.query(&models, busy))
    }
    pub fn asset_library_query(&self, query: AssetLibraryQuery) -> Result<Value, String> {
        self.with_library(|library| library.asset_library_query(query))
            .and_then(|value| serde_json::to_value(value).map_err(|error| error.to_string()))
    }
    pub fn asset_library_item(&self, id: &str) -> Result<Value, String> {
        let id = AssetId::new(id).map_err(|error| error.to_string())?;
        self.with_library(|library| library.asset_library_item(&id))
            .and_then(|value| serde_json::to_value(value).map_err(|error| error.to_string()))
    }
    /// Explicit favourite assignment; only an active primary work qualifies and
    /// the host returns the confirmed persisted value.
    pub fn asset_set_favorite(&self, asset_id: &str, favorite: bool) -> Result<Value, String> {
        let id = AssetId::new(asset_id).map_err(|error| error.to_string())?;
        let confirmed = self.with_library(|library| library.set_asset_favorite(&id, favorite))?;
        Ok(json!({"assetId": asset_id, "favorite": confirmed}))
    }
    /// Confirmed work deletion: reference check, persisted intent, controlled
    /// file cleanup and completion, serialized on the library boundary so a
    /// concurrent export or read cannot interleave with the removal.
    pub fn asset_delete(&self, asset_id: &str) -> Result<Value, String> {
        let id = AssetId::new(asset_id).map_err(|error| error.to_string())?;
        self.with_library(|library| {
            let group = library.asset_deletion_group(&id)?;
            if group.all_deleted() {
                return Ok(json!({"assetId": asset_id, "state": "deleted"}));
            }
            let conflicts = library.asset_reference_conflicts(&id)?;
            if !conflicts.is_empty() {
                return Ok(json!({
                    "assetId": asset_id,
                    "state": "conflict",
                    "conflicts": conflicts
                }));
            }
            library.begin_asset_deletion(&id)?;
            if let Err(error) = library.remove_asset_deletion_files(&id) {
                library.fail_asset_deletion(&id, &error.to_string())?;
                return Ok(json!({
                    "assetId": asset_id,
                    "state": "failed",
                    "error": error.to_string()
                }));
            }
            library.finish_asset_deletion(&id)?;
            Ok(json!({"assetId": asset_id, "state": "deleted"}))
        })
    }
    pub fn import_source(&self, path: &Path) -> Result<Value, String> {
        let record = self.with_library(|library| library.import_source(path))?;
        serde_json::to_value(record).map_err(|error| error.to_string())
    }
    pub fn imported_sources(&self) -> Result<Value, String> {
        let records = self.with_library(|library| library.imported_sources())?;
        serde_json::to_value(records).map_err(|error| error.to_string())
    }
    pub fn task_center_query(&self, query: TaskCenterQuery) -> Result<Value, String> {
        self.with_library(|library| library.task_center_query(query))
            .and_then(|value| serde_json::to_value(value).map_err(|error| error.to_string()))
    }
    pub fn task_center_detail(&self, id: &str) -> Result<Value, String> {
        let id = JobId::new(id).map_err(|error| error.to_string())?;
        self.with_library(|library| library.task_center_detail(&id))
            .and_then(|value| serde_json::to_value(value).map_err(|error| error.to_string()))
    }
    pub fn preferences_snapshot(&self) -> Value {
        let snapshot = self.preferences.snapshot();
        let models = self
            .catalog
            .as_ref()
            .map(ModelCatalog::entries)
            .unwrap_or_default();
        json!({
            "preferences": snapshot,
            "models": models
        })
    }
    pub fn reload_preferences(&self) -> Value {
        self.preferences.reload();
        self.preferences_snapshot()
    }
    pub fn save_appearance(&self, appearance: AppearancePreferences) -> Result<Value, String> {
        self.preferences.save_appearance(appearance)?;
        Ok(self.preferences_snapshot())
    }
    pub fn save_creation_defaults(&self, defaults: CreationDefaults) -> Result<Value, String> {
        let catalog = self.catalog.as_ref().map_err(|error| error.clone())?;
        if !catalog.supports(&defaults.model_id, "text_to_image") {
            return Err("默认模型必须支持已接入的文生图能力".into());
        }
        enforce_hardware_gate(catalog, &defaults.model_id, &self.device_facts)?;
        #[cfg(windows)]
        if defaults.aspect != "1:1" {
            return Err("当前 Windows 型号仅支持 1:1 默认画幅".into());
        }
        if matches!(
            defaults.model_id.as_str(),
            QWEN_IMAGE_21_MODEL
                | QWEN_IMAGE_21_TURBO_MODEL
                | MOSS_IMAGE_SFT_MODEL
                | studio_core::generation::QWEN_IMAGE_21_WINDOWS_MODEL
                | studio_core::generation::QWEN_IMAGE_21_TURBO_WINDOWS_MODEL
                | studio_core::generation::MOSS_IMAGE_SFT_WINDOWS_MODEL
        ) && defaults.aspect != "1:1"
        {
            return Err("所选模型仅支持 1:1；请明确修改默认画幅后再保存".into());
        }
        self.preferences.save_creation_defaults(defaults)?;
        Ok(self.preferences_snapshot())
    }
    pub fn settings_snapshot(&self, diagnostics_enabled: bool) -> Result<Value, String> {
        let preferences = self.preferences.snapshot();
        let catalog = self.model_catalog()?;
        let storage = storage::summary(&self.paths);
        let installations = self
            .installation
            .as_ref()
            .map(Installations::snapshots)
            .unwrap_or_default();
        let model = self.model_view();
        let device = self.device_facts;
        let runtime_list = installations
            .iter()
            .map(|snapshot| {
                let facts = self
                    .catalog
                    .as_ref()
                    .ok()
                    .and_then(|catalog| catalog.model_facts(&snapshot.install_id, &device));
                let hardware_eligible = facts.as_ref().is_none_or(|facts| facts.hardware.eligible);
                let hardware_reason = facts
                    .as_ref()
                    .and_then(|facts| facts.hardware.reason.clone());
                let metadata = facts
                    .as_ref()
                    .map(|facts| facts.metadata.clone())
                    .unwrap_or_default();
                let tier = facts
                    .as_ref()
                    .map(|facts| facts.tier.clone())
                    .unwrap_or_default();
                json!({
                    "installId": snapshot.install_id.clone(),
                    "label": install_display_name(&snapshot.install_id),
                    "runtimeLockId": runtime_lock_id(&snapshot.install_id),
                    "phase": snapshot.phase.clone(),
                    "runtimeReady": snapshot.runtime_ready,
                    "modelReady": snapshot.model_ready,
                    "deviceVerified": snapshot.device_verified,
                    "lastError": snapshot.error.clone(),
                    "hardwareEligible": hardware_eligible,
                    "hardwareReason": hardware_reason,
                    "minVramGb": metadata.min_vram_gb,
                    "minRamGb": metadata.min_ram_gb,
                    "standardMinVramGb": tier.standard_min_vram_gb,
                    "tier": tier.id,
                    "resolution": tier.resolution,
                    "computeDtype": tier.compute_dtype,
                    "tierReason": tier.reason
                })
            })
            .collect::<Vec<_>>();
        let runtime = installations.first().map_or_else(
            || {
                json!({
                    "phase": "unavailable",
                    "summary": self.startup_error.clone().unwrap_or_else(|| "运行环境不可用".into()),
                    "runtimeReady": false,
                    "deviceVerified": false,
                    "lastError": self.startup_error
                })
            },
            |snapshot: &InstallationSnapshot| {
                json!({
                    "phase": snapshot.phase,
                    "summary": model["summary"],
                    "runtimeReady": snapshot.runtime_ready,
                    "modelReady": snapshot.model_ready,
                    "deviceVerified": snapshot.device_verified,
                    "lastError": snapshot.error
                })
            },
        );
        #[cfg(windows)]
        let runtime_name = "Diffusers CUDA v1 + v2 共享运行时";
        #[cfg(not(windows))]
        let runtime_name = "MFLUX / MLX + ONNX Runtime CPU + ncnn Vulkan";
        #[cfg(windows)]
        let runtime_version = "v1: Diffusers 0.36.0 / PyTorch 2.10.0 / CUDA 12.6 · v2: diffusers 0.41 / transformers 5.3 / torch 2.10.0+cu126 · ncnn Vulkan v0.2.0";
        #[cfg(not(windows))]
        let runtime_version = "0.24.0 / 1.31.1 / 0.2.0";
        #[cfg(windows)]
        let python_version = "3.12.10";
        #[cfg(not(windows))]
        let python_version = "3.13.12";
        #[cfg(windows)]
        let model_sources: Vec<String> = vec![
            "Z-Image Turbo · ModelScope Tongyi-MAI/Z-Image-Turbo · 逐文件等价核验 · Apache-2.0"
                .into(),
            "FLUX.2 Klein 4B distilled · black-forest-labs/FLUX.2-klein-4B · Apache-2.0".into(),
            "BiRefNet general epoch_244 · ZhengPeng7/BiRefNet v1 · MIT".into(),
            "Real-ESRGAN x4plus · 官方 Windows ncnn 包 v0.2.5.0 · BSD-3-Clause / MIT".into(),
            "Qwen-Image 2.1 · Qwen/Qwen-Image-2.1 · Qwen Research License（非商用）".into(),
            "Qwen-Image 2.1 Turbo · Qwen/Qwen-Image-2.1-Turbo · Qwen Research License（非商用）"
                .into(),
            "MOSS-Image-SFT · OpenMOSS-Team/MOSS-Image-SFT · Apache-2.0".into(),
        ];
        #[cfg(not(windows))]
        let model_sources: Vec<String> = vec![
            "Z-Image Turbo · Tongyi-MAI/Z-Image-Turbo · Apache-2.0".into(),
            "FLUX.2 Klein 4B distilled · black-forest-labs/FLUX.2-klein-4B · Apache-2.0".into(),
            "BiRefNet general epoch_244 · ZhengPeng7/BiRefNet v1 · MIT".into(),
            "Real-ESRGAN x4plus · official ncnn package v0.2.5.0 · BSD-3-Clause".into(),
            "Qwen-Image 2.1 · Qwen/Qwen-Image-2.1 · qwen-research".into(),
            "Qwen-Image 2.1 Turbo · Qwen/Qwen-Image-2.1-Turbo · qwen-research".into(),
            "MOSS-Image-SFT · OpenMOSS-Team/MOSS-Image-SFT · Apache-2.0".into(),
        ];
        #[cfg(windows)]
        let capability_scope = "Windows 已接入七个型号与八项能力：Z-Image Turbo（v1 运行时）、FLUX.2 Klein（文生图/单图编辑）、BiRefNet 去背景、Real-ESRGAN 4× 放大、Qwen-Image 2.1 基础版/Turbo（非商用）与 MOSS-Image-SFT；低于硬件门槛时禁用安装与生成入口。";
        #[cfg(not(windows))]
        let capability_scope = "Z-Image Turbo、FLUX.2 Klein 4B、BiRefNet epoch_244、Real-ESRGAN x4plus、Qwen-Image 2.1 基础版/Turbo 与 MOSS-Image-SFT 已接入本地安装与执行链路。";
        Ok(json!({
            "preferences": preferences,
            "catalog": catalog,
            "storage": storage,
            "environment": {
                "platform": std::env::consts::OS,
                "architecture": std::env::consts::ARCH,
                "runtimeName": runtime_name,
                "runtimeVersion": runtime_version,
                "pythonVersion": python_version,
                "diagnosticsEnabled": diagnostics_enabled,
                "runtime": runtime,
                "runtimes": runtime_list,
                "deviceFacts": device.label(),
                "deviceVramGb": device.vram_gb,
                "deviceRamGb": device.ram_gb
            },
            "about": {
                "version": env!("CARGO_PKG_VERSION"),
                "productName": "Image Studio",
                "localProcessing": true,
                "autoUpdateSupported": false,
                "onlineProviderSupported": false,
                "storageMigrationSupported": false,
                "examples": "风景、静物、插画为应用内嵌原创示例，不是本机生成结果。",
                "fontLicense": "Noto Sans CJK SC · SIL Open Font License 1.1",
                "modelSources": model_sources,
                "capabilityScope": capability_scope
            }
        }))
    }
    pub fn open_storage_location(&self, id: &str) -> Result<(), String> {
        storage::open_location(&self.paths, id)
    }
    pub fn apply_model_to_conversation(
        &self,
        conversation_id: &str,
        model_id: &str,
    ) -> Result<Value, String> {
        let catalog = self.catalog.as_ref().map_err(|error| error.clone())?;
        if !catalog.executable(model_id) {
            return Err("该模型当前不可执行，不能用于会话".into());
        }
        enforce_hardware_gate(catalog, model_id, &self.device_facts)?;
        let installation = self.installation.as_ref().ok_or("安装器不可用")?;
        let snapshot = installation
            .snapshot(model_id)
            .ok_or("该模型没有登记的安装状态")?;
        if snapshot.phase != "ready"
            || !snapshot.model_ready
            || !snapshot.runtime_ready
            || !snapshot.device_verified
        {
            return Err("模型尚未完成安装、运行时和设备检查".into());
        }
        let id = ConversationId::new(conversation_id).map_err(|error| error.to_string())?;
        let draft =
            self.with_library(|library| library.apply_model_to_conversation(&id, model_id))?;
        Ok(json!({
            "conversationId": conversation_id,
            "draft": draft
        }))
    }
    fn validate_registered_draft_params(&self, value: &Value) -> Result<DraftParams, String> {
        let params: DraftParams =
            serde_json::from_value(value.clone()).map_err(|_| "创作参数结构无效".to_string())?;
        let catalog = self.catalog.as_ref().map_err(|error| error.clone())?;
        for (capability, saved) in &params.capability_drafts {
            let aspect_valid = match capability.as_str() {
                studio_core::generation::Z_IMAGE_CAPABILITY => {
                    matches!(saved.aspect.as_str(), "1:1" | "4:3" | "3:4")
                }
                studio_core::generation::Z_IMAGE_WINDOWS_CAPABILITY
                | studio_core::recipes::KLEIN_TEXT_CAPABILITY
                | studio_core::recipes::KLEIN_EDIT_CAPABILITY
                | studio_core::recipes::BIREFNET_CAPABILITY
                | studio_core::recipes::REALESRGAN_CAPABILITY
                | QWEN_IMAGE_21_CAPABILITY
                | QWEN_IMAGE_21_TURBO_CAPABILITY
                | MOSS_IMAGE_SFT_CAPABILITY => saved.aspect == "1:1",
                #[cfg(windows)]
                studio_core::generation::KLEIN_WINDOWS_TEXT_CAPABILITY
                | studio_core::generation::KLEIN_WINDOWS_EDIT_CAPABILITY
                | studio_core::generation::BIREFNET_WINDOWS_CAPABILITY
                | studio_core::generation::REALESRGAN_WINDOWS_CAPABILITY
                | studio_core::generation::QWEN_IMAGE_21_WINDOWS_CAPABILITY
                | studio_core::generation::QWEN_IMAGE_21_TURBO_WINDOWS_CAPABILITY
                | studio_core::generation::MOSS_IMAGE_SFT_WINDOWS_CAPABILITY => {
                    saved.aspect == "1:1"
                }
                _ => false,
            };
            let tool = is_source_only_tool_capability(capability);
            if !aspect_valid
                || saved.prompt.len() > 1024 * 1024
                || !valid_seed(&saved.seed)
                || tool && (!saved.prompt.is_empty() || !saved.seed.is_empty())
                || saved
                    .prompt_source_id
                    .as_ref()
                    .is_some_and(|id| id.is_empty() || id.len() > 128)
            {
                return Err("能力草稿结构或范围无效".into());
            }
            if let Some(source) = &saved.source_asset_id {
                AssetId::new(source).map_err(|error| error.to_string())?;
            }
        }
        if !catalog.contains(&params.model_id) {
            ModelVariantId::new(&params.model_id).map_err(|error| error.to_string())?;
            if params.capability_id.is_empty()
                || params.capability_id.len() > 128
                || !params
                    .capability_id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
                || !matches!(params.aspect.as_str(), "1:1" | "4:3" | "3:4")
                || params.count != 1
                || !valid_seed(&params.seed)
            {
                return Err("历史草稿参数结构或范围无效".into());
            }
            if let Some(source) = &params.source_asset_id {
                let source = AssetId::new(source).map_err(|error| error.to_string())?;
                self.with_library(|library| library.resolve_source(&source))?;
            }
            return Ok(params);
        }
        let supported_combo = match (
            params.model_id.as_str(),
            params.capability_id.as_str(),
            params.source_asset_id.is_some(),
        ) {
            #[cfg(windows)]
            (
                WINDOWS_ZIMAGE_INSTALL_ID,
                studio_core::generation::Z_IMAGE_WINDOWS_CAPABILITY,
                false,
            ) => params.aspect == "1:1",
            (DEFAULT_MODEL_ID, studio_core::generation::Z_IMAGE_CAPABILITY, false) => {
                matches!(params.aspect.as_str(), "1:1" | "4:3" | "3:4")
            }
            (
                studio_core::recipes::KLEIN_MODEL,
                studio_core::recipes::KLEIN_TEXT_CAPABILITY,
                false,
            )
            | (studio_core::recipes::KLEIN_MODEL, studio_core::recipes::KLEIN_EDIT_CAPABILITY, _) => {
                params.aspect == "1:1"
            }
            (
                studio_core::recipes::BIREFNET_MODEL,
                studio_core::recipes::BIREFNET_CAPABILITY,
                _,
            )
            | (
                studio_core::recipes::REALESRGAN_MODEL,
                studio_core::recipes::REALESRGAN_CAPABILITY,
                _,
            ) => params.aspect == "1:1",
            (QWEN_IMAGE_21_MODEL, QWEN_IMAGE_21_CAPABILITY, false)
            | (QWEN_IMAGE_21_TURBO_MODEL, QWEN_IMAGE_21_TURBO_CAPABILITY, false)
            | (MOSS_IMAGE_SFT_MODEL, MOSS_IMAGE_SFT_CAPABILITY, false) => params.aspect == "1:1",
            #[cfg(windows)]
            (
                WINDOWS_KLEIN_MODEL,
                studio_core::generation::KLEIN_WINDOWS_TEXT_CAPABILITY,
                false,
            )
            | (WINDOWS_KLEIN_MODEL, studio_core::generation::KLEIN_WINDOWS_EDIT_CAPABILITY, _)
            | (WINDOWS_BIREFNET_MODEL, studio_core::generation::BIREFNET_WINDOWS_CAPABILITY, _)
            | (
                WINDOWS_REALESRGAN_MODEL,
                studio_core::generation::REALESRGAN_WINDOWS_CAPABILITY,
                _,
            ) => params.aspect == "1:1",
            #[cfg(windows)]
            (
                WINDOWS_QWEN_MODEL,
                studio_core::generation::QWEN_IMAGE_21_WINDOWS_CAPABILITY,
                false,
            )
            | (
                WINDOWS_QWEN_TURBO_MODEL,
                studio_core::generation::QWEN_IMAGE_21_TURBO_WINDOWS_CAPABILITY,
                false,
            )
            | (
                WINDOWS_MOSS_MODEL,
                studio_core::generation::MOSS_IMAGE_SFT_WINDOWS_CAPABILITY,
                false,
            ) => params.aspect == "1:1",
            _ => false,
        };
        if !supported_combo {
            return Err("模型、能力、源图与画幅组合不受支持".into());
        }
        if is_source_only_tool_model(&params.model_id) && !params.seed.is_empty() {
            return Err("当前工具能力不接受种子".into());
        }
        if let Some(source) = &params.source_asset_id {
            let source = AssetId::new(source).map_err(|error| error.to_string())?;
            self.with_library(|library| library.resolve_source(&source))?;
        }
        if params.count != 1 {
            return Err("当前产品每轮只接受 1 张图片".into());
        }
        if !valid_seed(&params.seed) {
            return Err("种子须为 0–4294967295 的整数或留空".into());
        }
        Ok(params)
    }
    pub fn resolve_new_conversation_defaults(
        &self,
        input: Option<&ConversationCreateInput>,
    ) -> Result<(DraftParams, Vec<String>), String> {
        let catalog = self.catalog.as_ref().map_err(|error| error.clone())?;
        let snapshot = self.preferences.snapshot();
        let mut warnings = snapshot.warnings;
        if let Some(error) = snapshot.load_error {
            warnings.push(format!("应用偏好未能恢复，本次使用内置回退：{error}"));
        }
        let mut params = DraftParams {
            model_id: snapshot.creation_defaults.model_id,
            aspect: snapshot.creation_defaults.aspect,
            count: snapshot.creation_defaults.count,
            seed: snapshot.creation_defaults.seed,
            capability_id: default_capability_id(),
            source_asset_id: None,
            capability_drafts: Default::default(),
        };
        let explicit_model = input.and_then(|value| value.model_id.as_deref());
        let explicit_aspect = input.and_then(|value| value.aspect.as_deref());
        if let Some(input) = input {
            if let Some(model_id) = &input.model_id {
                if !catalog.supports(model_id, "text_to_image") {
                    return Err("创建入口覆盖的模型不支持文生图".into());
                }
                params.model_id = model_id.clone();
            }
            if let Some(aspect) = &input.aspect {
                if !matches!(aspect.as_str(), "1:1" | "4:3" | "3:4") {
                    return Err("创建入口覆盖的画幅不受支持".into());
                }
                params.aspect = aspect.clone();
            }
            if let Some(count) = input.count {
                if count != 1 {
                    return Err("创建入口覆盖的数量不受支持".into());
                }
                params.count = count;
            }
            if let Some(seed) = &input.seed {
                if !valid_seed(seed) {
                    return Err("创建入口覆盖的种子无效".into());
                }
                params.seed = seed.clone();
            }
        }
        if !catalog.supports(&params.model_id, "text_to_image") && explicit_model.is_none() {
            warnings.push(format!(
                "已保存的默认模型 {} 在当前平台不可执行；原选择已保留，请明确选择可用模型",
                params.model_id
            ));
        }
        if !model_accepts_aspect(&params.model_id, &params.aspect) {
            if explicit_aspect.is_some() {
                return Err("所选模型不支持该画幅；当前输入和默认画幅未被改写".into());
            }
            warnings.push(format!(
                "已保存的默认画幅 {} 不适用于 {}；原画幅已保留，请明确修改",
                params.aspect, params.model_id
            ));
        }
        params.capability_id = capability_for_model(&params.model_id).into();
        params.source_asset_id = None;
        Ok((params, warnings))
    }
    pub fn snapshot(&self) -> Result<Value, String> {
        let model = self.model_view();
        let models = self.model_views();
        let execution = self.execution_view();
        if self.library.is_none() {
            return Ok(
                json!({"model":model,"models":models,"drafts":[],"activeId":null,"activeConversationId":null,"conversations":[],"turns":[],"turnsHasMore":false,"turnsNextCursor":null,"selectedAssetId":null,"jobs":[],"assets":[],"jobCounts":[0,0,0],"execution":execution,"error":self.startup_error}),
            );
        }
        let (lib, jobs, assets, turns, counts) = self.with_library(|library| {
            let lib = library.snapshot()?;
            let counts = library.job_counts()?;
            let jobs = lib.jobs.iter().map(job_view).collect::<Vec<_>>();
            let assets = lib
                .assets
                .iter()
                .map(|asset| {
                    let job = lib
                        .jobs
                        .iter()
                        .find(|job| job.job_id == asset.job_id)
                        .cloned()
                        .or_else(|| library.job(&asset.job_id).ok());
                    json!({
                        "id": asset.asset.asset_id,
                        "jobId": asset.job_id,
                        "conversationId": job.as_ref().and_then(|job| job.conversation_id.clone()),
                        "turnId": job.as_ref().and_then(|job| job.turn_id.clone()),
                        "prompt": job.as_ref().map(|job| job.request.instruction.as_str()).unwrap_or("生成图片"),
                        "width": asset.width,
                        "height": asset.height,
                        "seed": job.as_ref().and_then(|job| job.plan.effective_parameters.get("seed")).cloned().unwrap_or(Value::Null),
                        "bytes": asset.byte_count,
                        "outputRole": asset.output_role,
                        "dataType": match asset.asset.data_type {
                            AssetKind::ImageRgb => "image_rgb",
                            AssetKind::ImageRgba => "image_rgba",
                            AssetKind::Mask => "mask",
                            AssetKind::Depth | AssetKind::StructuredData => "other",
                        },
                        "available": asset.available,
                        "error": asset.availability_error,
                        "isFavorite": asset.is_favorite,
                        "deletionState": asset.deletion_state,
                        "deletionError": asset.deletion_error
                    })
                })
                .collect::<Vec<_>>();
            let turns = lib
                .turns
                .turns
                .iter()
                .map(|turn| turn_view(library, turn))
                .collect::<Result<Vec<_>, _>>()?;
            Ok((lib, jobs, assets, turns, counts))
        })?;
        Ok(json!({
            "model": model,
            "models": models,
            "drafts": lib.drafts,
            "activeId": lib.active_draft_id,
            "activeConversationId": lib.active_conversation_id,
            "conversations": lib.conversations,
            "conversationsHasMore": lib.conversations_has_more,
            "conversationsNextCursor": lib.conversations_next_cursor,
            "turns": turns,
            "turnsHasMore": lib.turns.has_more,
            "turnsNextCursor": lib.turns.next_cursor,
            "selectedAssetId": lib.selected_asset_id,
            "jobs": jobs,
            "assets": assets,
            "jobCounts": [counts.0, counts.1, counts.2],
            "execution": execution,
            "error": null
        }))
    }
    fn execution_view(&self) -> Value {
        if let Some(controller) = &self.generation {
            let s = controller.snapshot();
            json!({"busy":s.busy,"jobId":s.job_id,"attemptId":s.attempt_id,"stage":s.stage,"summary":s.summary,"step":s.step,"total":s.total,"logs":s.logs,"cleanupError":s.cleanup_error})
        } else {
            json!({"busy":false,"jobId":null,"attemptId":null,"stage":"idle","summary":"尚未开始生成","step":null,"total":null,"logs":"","cleanupError":null})
        }
    }
    pub fn install(&self, id: &str) -> Result<Value, String> {
        let _schedule = self
            .scheduling
            .lock()
            .map_err(|_| "提交锁异常".to_string())?;
        self.ensure_open()?;
        if self.library.is_none() {
            return Err(self
                .startup_error
                .clone()
                .unwrap_or_else(|| "本地库不可用".into()));
        }
        if self
            .generation
            .as_ref()
            .is_some_and(GenerationController::is_busy)
            || self.with_library(|library| library.has_active_generation())?
        {
            return Err("请等待生成和清理完成后安装".into());
        }
        let installation = self.installation.as_ref().ok_or("安装器不可用")?;
        // Host-side hardware gate: refuse before `start` creates any install state. A direct IPC
        // call must not bypass the model-page gate; the rejection text is the catalog reason.
        {
            let catalog = self.catalog.as_ref().map_err(|error| error.clone())?;
            enforce_hardware_gate(catalog, id, &self.device_facts)?;
        }
        installation.start(id)?;
        Ok(self.model_view_for(id))
    }
    pub fn cancel_install(&self, id: &str) -> Result<Value, String> {
        self.installation
            .as_ref()
            .ok_or("安装器不可用")?
            .cancel(id)?;
        Ok(self.model_view_for(id))
    }
    fn installation_busy(&self) -> bool {
        self.installation
            .as_ref()
            .is_some_and(Installations::is_busy)
    }
    pub fn save_draft(
        &self,
        draft: DraftInput,
        expected_version: u64,
    ) -> Result<DraftRecord, String> {
        let params = self.validate_registered_draft_params(&draft.params)?;
        let params = serde_json::to_value(params).map_err(|error| error.to_string())?;
        self.with_library(|library| {
            library.save_draft(
                DraftRecord {
                    id: draft.id,
                    name: draft.name,
                    prompt: draft.prompt,
                    params,
                    collapsed: draft.collapsed,
                    version: expected_version,
                    prompt_source: draft.prompt_source_id,
                },
                expected_version,
            )
        })
    }
    pub fn select(&self, id: u64, asset: Option<String>) -> Result<(), String> {
        let asset = asset
            .map(AssetId::new)
            .transpose()
            .map_err(|e| e.to_string())?;
        self.with_library(|library| {
            library.select_draft(id)?;
            library.select_asset(asset)
        })
    }
    pub fn conversation_list(
        &self,
        limit: u32,
        before_activity: Option<u64>,
    ) -> Result<Value, String> {
        let page = self.with_library(|library| library.conversations(limit, before_activity))?;
        Ok(json!({
            "conversations": page.conversations,
            "hasMore": page.has_more,
            "nextCursor": page.next_cursor
        }))
    }
    pub fn conversation_create(
        &self,
        input: Option<ConversationCreateInput>,
    ) -> Result<Value, String> {
        let initial_title = input
            .as_ref()
            .and_then(|value| value.initial_title.as_deref())
            .unwrap_or("新会话");
        let initial_prompt = input
            .as_ref()
            .and_then(|value| value.initial_prompt.as_deref())
            .unwrap_or("");
        let prompt_source = input
            .as_ref()
            .and_then(|value| value.prompt_source_id.as_deref());
        if initial_title.is_empty() || initial_title.len() > 1024 {
            return Err("新会话标题无效".into());
        }
        if initial_prompt.len() > 1024 * 1024 {
            return Err("新会话初始提示词超出保存范围".into());
        }
        if prompt_source.is_some_and(|value| !valid_prompt_source(value)) {
            return Err("灵感案例来源标识无效".into());
        }
        if prompt_source.is_some() && initial_prompt.is_empty() {
            return Err("灵感案例来源缺少初始提示词".into());
        }
        let (params, warnings) = self.resolve_new_conversation_defaults(input.as_ref())?;
        let params = serde_json::to_value(params).map_err(|error| error.to_string())?;
        let created = self.with_library(|library| {
            library.create_conversation_with_content(
                params,
                initial_title,
                initial_prompt,
                prompt_source,
            )
        })?;
        Ok(json!({
            "conversation": created.conversation,
            "draft": created.draft,
            "defaultsWarnings": warnings
        }))
    }
    pub fn draft_create(&self) -> Result<DraftRecord, String> {
        let (params, _) = self.resolve_new_conversation_defaults(None)?;
        let params = serde_json::to_value(params).map_err(|error| error.to_string())?;
        self.with_library(|library| library.create_conversation_with_params(params))
            .map(|created| created.draft)
    }
    pub fn conversation_select(&self, conversation_id: &str) -> Result<Value, String> {
        let id = ConversationId::new(conversation_id).map_err(|error| error.to_string())?;
        let conversation = self.with_library(|library| {
            library.select_conversation(&id)?;
            library.conversation(&id)
        })?;
        Ok(json!({"conversation": conversation}))
    }
    /// Confirmed conversation deletion. A target that still owns the active
    /// execution or its cleanup is refused before the library transaction, so
    /// deleting never implicitly cancels a running send.
    pub fn conversation_delete(
        &self,
        conversation_id: &str,
        expected_draft_version: u64,
    ) -> Result<Value, String> {
        let id = ConversationId::new(conversation_id).map_err(|error| error.to_string())?;
        if let Some(generation) = self.generation.as_ref() {
            let view = generation.snapshot();
            if view.busy
                && let Some(job_id) = view
                    .job_id
                    .as_deref()
                    .and_then(|value| JobId::new(value).ok())
            {
                let owner = self
                    .try_with_library(|library| library.job(&job_id).map(|job| job.conversation_id))
                    .and_then(|result| result.ok())
                    .flatten();
                if owner.as_ref() == Some(&id) {
                    return Err("目标会话仍在生成或清理，请先停止并等待清理结束".into());
                }
            }
        }
        let result =
            self.with_library(|library| library.delete_conversation(&id, expected_draft_version))?;
        serde_json::to_value(result).map_err(|error| error.to_string())
    }
    pub fn conversation_rename(&self, conversation_id: &str, title: &str) -> Result<Value, String> {
        let id = ConversationId::new(conversation_id).map_err(|error| error.to_string())?;
        let conversation = self.with_library(|library| library.rename_conversation(&id, title))?;
        Ok(json!({"conversation": conversation}))
    }
    pub fn conversation_history(
        &self,
        conversation_id: &str,
        before_sequence: Option<u64>,
        limit: u32,
    ) -> Result<Value, String> {
        let id = ConversationId::new(conversation_id).map_err(|error| error.to_string())?;
        self.with_library(|library| {
            let page = library.turns(&id, limit, before_sequence)?;
            let turns = page
                .turns
                .iter()
                .map(|turn| turn_view(library, turn))
                .collect::<crate::adapters::library::Result<Vec<_>>>()?;
            Ok(json!({
                "turns": turns,
                "hasMore": page.has_more,
                "nextCursor": page.next_cursor
            }))
        })
    }
    pub fn conversation_turn_detail(&self, turn_id: &str) -> Result<Value, String> {
        let id = TurnId::new(turn_id).map_err(|error| error.to_string())?;
        self.with_library(|library| {
            let turn = library.turn(&id)?;
            let attempts = turn
                .job
                .as_ref()
                .map(|job| library.attempts(&job.job_id))
                .transpose()?
                .unwrap_or_default()
                .into_iter()
                .map(|attempt| job_view(&attempt))
                .collect::<Vec<_>>();
            Ok(json!({
                "turnId": turn.id,
                "attempts": attempts
            }))
        })
    }
    pub fn conversation_send(
        self: &Arc<Self>,
        input: ConversationSendInput,
    ) -> Result<Value, String> {
        let catalog = self.catalog.as_ref().map_err(|error| error.clone())?;
        if !catalog.executable(&input.model_id) {
            return Err("当前会话选择的模型尚不可执行，请切换到已就绪模型".into());
        }
        // Host-side hardware gate: a direct IPC send must not bypass the composer gate. Reject
        // before `accept_turn`, so no Turn/Attempt/Job or draft consumption is created.
        enforce_hardware_gate(catalog, &input.model_id, &self.device_facts)?;
        let conversation_id =
            ConversationId::new(input.conversation_id).map_err(|error| error.to_string())?;
        let submission_key =
            IdempotencyKey::new(input.submission_key).map_err(|error| error.to_string())?;
        let _schedule = self
            .scheduling
            .lock()
            .map_err(|_| "提交锁异常".to_string())?;
        self.ensure_open()?;
        if self
            .generation
            .as_ref()
            .is_some_and(GenerationController::is_busy)
            || self.installation_busy()
        {
            return Err("生成、清理或模型安装仍在进行，请稍后重试".into());
        }
        let source_asset_id = input
            .source_asset_id
            .as_deref()
            .map(AssetId::new)
            .transpose()
            .map_err(|error| error.to_string())?;
        if let Some(source) = &source_asset_id {
            self.with_library(|library| library.resolve_source(source))?;
        }
        let capability_id = input.capability_id.unwrap_or_else(|| {
            if source_asset_id.is_some()
                && let Some(edit) = klein_edit_capability(&input.model_id)
            {
                return edit.into();
            }
            capability_for_model(&input.model_id).into()
        });
        let accepted = self.with_library(|library| {
            library.accept_turn(AcceptTurnInput {
                conversation_id: conversation_id.clone(),
                draft_id: input.draft_id,
                expected_version: input.expected_version,
                submission_key,
                prompt: input.prompt,
                model_id: input.model_id,
                capability_id,
                source_asset_id,
                aspect: input.aspect,
                count: input.count,
                seed: input.seed,
            })
        })?;
        drop(_schedule);
        if accepted.created {
            let service = self.clone();
            let turn = accepted.turn.clone();
            if let Err(error) = std::thread::Builder::new()
                .name("studio-conversation-preflight".into())
                .spawn(move || service.run_turn_preflight(turn))
            {
                let _ = self.with_library(|library| {
                    library.reject_turn(&accepted.turn.id, &format!("无法启动后台预检：{error}"))
                });
            }
        }
        let turn = self.with_library(|library| {
            let turn = library.turn(&accepted.turn.id)?;
            turn_view(library, &turn)
        })?;
        Ok(json!({
            "turn": turn,
            "draft": accepted.draft,
            "created": accepted.created
        }))
    }
    fn run_turn_preflight(self: Arc<Self>, turn: TurnRecord) {
        // Stage traces: these are the only way to tell a stranded preflight from a slow one on
        // a GUI process, whose stderr is not otherwise visible.
        let started = std::time::Instant::now();
        crate::adapters::logging::debug("turn.preflight.start", &[("turn", turn.id.to_string())]);
        match self.prepare_turn(&turn) {
            Ok(()) => crate::adapters::logging::info(
                "turn.preflight.done",
                &[
                    ("turn", turn.id.to_string()),
                    ("ms", started.elapsed().as_millis().to_string()),
                ],
            ),
            Err(error) => {
                crate::adapters::logging::warn(
                    "turn.preflight.failed",
                    &[
                        ("turn", turn.id.to_string()),
                        ("ms", started.elapsed().as_millis().to_string()),
                        ("error", error.clone()),
                    ],
                );
                let _ = self.with_library(|library| library.reject_turn(&turn.id, &error));
            }
        }
    }
    fn prepare_turn(&self, turn: &TurnRecord) -> Result<(), String> {
        self.ensure_open()?;
        {
            let current = self.with_library(|library| library.turn(&turn.id))?;
            match current.status {
                TurnStatus::CancelRequested => {
                    let _ = self.with_library(|library| library.finish_cancelled_turn(&current.id));
                    return Ok(());
                }
                status if status != TurnStatus::Pending => return Ok(()),
                _ => {}
            }
        }
        // Defense in depth for turns accepted before the gate existed or before device facts
        // changed: reject here too, before `bind_turn_job` creates an Attempt/Job.
        {
            let catalog = self.catalog.as_ref().map_err(|error| error.clone())?;
            enforce_hardware_gate(catalog, &turn.model_id, &self.device_facts)?;
        }
        // A device that only satisfies the compact tier runs the model at 768 square; other
        // aspects would silently fall back to the 1024 standard recipe, so reject them.
        let compact_tier = self
            .catalog
            .as_ref()
            .ok()
            .and_then(|catalog| catalog.model_facts(&turn.model_id, &self.device_facts))
            .and_then(|facts| facts.tier.id)
            .is_some_and(|id| id == "compact");
        crate::adapters::logging::debug("turn.preflight.gate", &[("turn", turn.id.to_string())]);
        let (width, height) = compact_aspect_size(&turn.aspect, compact_tier)?;
        let seed = if turn.seed.is_empty() {
            None
        } else {
            Some(
                turn.seed
                    .parse::<u32>()
                    .map_err(|_| "种子须为 0–4294967295".to_string())?,
            )
        };
        crate::adapters::logging::debug(
            "turn.preflight.geometry",
            &[
                ("turn", turn.id.to_string()),
                ("size", format!("{width}x{height}")),
            ],
        );
        let installation = self.installation.as_ref().ok_or("安装器不可用")?;
        crate::adapters::logging::debug(
            "turn.preflight.ready.begin",
            &[("turn", turn.id.to_string())],
        );
        let ready = installation
            .ready_paths(&turn.model_id)
            .map_err(|error| format!("{error}（阶段：读取就绪记录）"))?;
        crate::adapters::logging::debug(
            "turn.preflight.ready.end",
            &[("turn", turn.id.to_string())],
        );
        crate::adapters::logging::debug(
            "turn.preflight.environment.begin",
            &[("turn", turn.id.to_string())],
        );
        let environment = installation
            .plan_environment(&turn.model_id)
            .map_err(|error| format!("{error}（阶段：规划运行环境）"))?;
        crate::adapters::logging::debug(
            "turn.preflight.environment.end",
            &[("turn", turn.id.to_string())],
        );
        {
            let current = self.with_library(|library| library.turn(&turn.id))?;
            if current.status == TurnStatus::CancelRequested {
                let _ = self.with_library(|library| library.finish_cancelled_turn(&current.id));
                return Ok(());
            }
            if current.status != TurnStatus::Pending {
                return Ok(());
            }
        }
        crate::adapters::logging::debug(
            "turn.preflight.freeze.begin",
            &[("turn", turn.id.to_string())],
        );
        let random = Uuid::new_v4();
        let bytes = random.as_bytes();
        let random_seed = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let request_id = RequestId::new(format!("request-{}", turn.submission_key))
            .map_err(|error| error.to_string())?;
        let plan_id = PlanId::new(format!("plan-{}", turn.submission_key))
            .map_err(|error| error.to_string())?;
        let idempotency_key =
            IdempotencyKey::new(turn.submission_key.clone()).map_err(|error| error.to_string())?;
        let project_id = ProjectId::new("default").map_err(|error| error.to_string())?;
        let (request, plan) = if turn.model_id == BIREFNET_INSTALL_ID {
            let source_id = turn
                .source_asset_id
                .as_ref()
                .ok_or("BiRefNet 去背景需要一张源图")?;
            let resolved = self.with_library(|library| library.resolve_source(source_id))?;
            let frozen = freeze_birefnet(
                BiRefNetInput {
                    request_id,
                    project_id,
                    idempotency_key,
                    plan_id,
                    source: SourceAsset {
                        asset: resolved.asset,
                        facts: SourceFacts {
                            width: resolved.width,
                            height: resolved.height,
                            has_transparent_pixels: resolved.has_transparent_pixels,
                        },
                    },
                },
                environment,
            )
            .map_err(|error| error.to_string())?;
            (frozen.request, frozen.plan)
        } else if turn.model_id == REALESRGAN_INSTALL_ID {
            let source_id = turn
                .source_asset_id
                .as_ref()
                .ok_or("Real-ESRGAN 放大需要一张源图")?;
            let resolved = self.with_library(|library| library.resolve_source(source_id))?;
            let frozen = freeze_realesrgan(
                RealEsrganInput {
                    request_id,
                    project_id,
                    idempotency_key,
                    plan_id,
                    source: SourceAsset {
                        asset: resolved.asset,
                        facts: SourceFacts {
                            width: resolved.width,
                            height: resolved.height,
                            has_transparent_pixels: resolved.has_transparent_pixels,
                        },
                    },
                },
                environment,
            )
            .map_err(|error| error.to_string())?;
            (frozen.request, frozen.plan)
        } else if matches!(
            turn.model_id.as_str(),
            QWEN_IMAGE_21_INSTALL_ID | QWEN_IMAGE_21_TURBO_INSTALL_ID | MOSS_IMAGE_SFT_INSTALL_ID
        ) {
            let recipe = match turn.model_id.as_str() {
                QWEN_IMAGE_21_INSTALL_ID => Recipe::QwenImage21,
                QWEN_IMAGE_21_TURBO_INSTALL_ID => Recipe::QwenImage21Turbo,
                MOSS_IMAGE_SFT_INSTALL_ID => Recipe::MossImageSft,
                _ => unreachable!(),
            };
            let frozen = freeze_text_model(
                recipe,
                TextModelInput {
                    request_id,
                    project_id,
                    idempotency_key,
                    plan_id,
                    prompt: turn.prompt.clone(),
                    seed,
                },
                random_seed,
                environment,
            )
            .map_err(|error| error.to_string())?;
            (frozen.request, frozen.plan)
        } else if turn.model_id == KLEIN_INSTALL_ID {
            let source = turn
                .source_asset_id
                .as_ref()
                .map(|id| {
                    let resolved = self.with_library(|library| library.resolve_source(id))?;
                    Ok::<_, String>(SourceAsset {
                        asset: resolved.asset,
                        facts: SourceFacts {
                            width: resolved.width,
                            height: resolved.height,
                            has_transparent_pixels: resolved.has_transparent_pixels,
                        },
                    })
                })
                .transpose()?;
            let frozen = freeze_klein(
                KleinInput {
                    request_id,
                    project_id,
                    idempotency_key,
                    plan_id,
                    prompt: turn.prompt.clone(),
                    seed,
                    source,
                },
                random_seed,
                environment,
            )
            .map_err(|error| error.to_string())?;
            (frozen.request, frozen.plan)
        } else if turn.model_id == WINDOWS_BIREFNET_MODEL {
            let source_id = turn
                .source_asset_id
                .as_ref()
                .ok_or("BiRefNet 去背景需要一张源图")?;
            let resolved = self.with_library(|library| library.resolve_source(source_id))?;
            let frozen = freeze_windows_birefnet(
                BiRefNetInput {
                    request_id,
                    project_id,
                    idempotency_key,
                    plan_id,
                    source: SourceAsset {
                        asset: resolved.asset,
                        facts: SourceFacts {
                            width: resolved.width,
                            height: resolved.height,
                            has_transparent_pixels: resolved.has_transparent_pixels,
                        },
                    },
                },
                environment,
            )
            .map_err(|error| error.to_string())?;
            (frozen.request, frozen.plan)
        } else if turn.model_id == WINDOWS_REALESRGAN_MODEL {
            let source_id = turn
                .source_asset_id
                .as_ref()
                .ok_or("Real-ESRGAN 放大需要一张源图")?;
            let resolved = self.with_library(|library| library.resolve_source(source_id))?;
            let frozen = freeze_windows_realesrgan(
                RealEsrganInput {
                    request_id,
                    project_id,
                    idempotency_key,
                    plan_id,
                    source: SourceAsset {
                        asset: resolved.asset,
                        facts: SourceFacts {
                            width: resolved.width,
                            height: resolved.height,
                            has_transparent_pixels: resolved.has_transparent_pixels,
                        },
                    },
                },
                environment,
            )
            .map_err(|error| error.to_string())?;
            (frozen.request, frozen.plan)
        } else if matches!(
            turn.model_id.as_str(),
            WINDOWS_QWEN_MODEL | WINDOWS_QWEN_TURBO_MODEL | WINDOWS_MOSS_MODEL
        ) {
            // A compact-tier turn asks for 768 square, which selects the compact recipe identity.
            let recipe = match (turn.model_id.as_str(), width == 768) {
                (WINDOWS_QWEN_MODEL, false) => Recipe::QwenImage21Windows,
                (WINDOWS_QWEN_MODEL, true) => Recipe::QwenImage21WindowsCompact,
                (WINDOWS_QWEN_TURBO_MODEL, false) => Recipe::QwenImage21TurboWindows,
                (WINDOWS_QWEN_TURBO_MODEL, true) => Recipe::QwenImage21TurboWindowsCompact,
                (WINDOWS_MOSS_MODEL, false) => Recipe::MossImageSftWindows,
                (WINDOWS_MOSS_MODEL, true) => Recipe::MossImageSftWindowsCompact,
                _ => unreachable!(),
            };
            let frozen = freeze_windows_text_model(
                recipe,
                TextModelInput {
                    request_id,
                    project_id,
                    idempotency_key,
                    plan_id,
                    prompt: turn.prompt.clone(),
                    seed,
                },
                random_seed,
                environment,
            )
            .map_err(|error| error.to_string())?;
            (frozen.request, frozen.plan)
        } else if turn.model_id == WINDOWS_KLEIN_MODEL {
            let source = turn
                .source_asset_id
                .as_ref()
                .map(|id| {
                    let resolved = self.with_library(|library| library.resolve_source(id))?;
                    Ok::<_, String>(SourceAsset {
                        asset: resolved.asset,
                        facts: SourceFacts {
                            width: resolved.width,
                            height: resolved.height,
                            has_transparent_pixels: resolved.has_transparent_pixels,
                        },
                    })
                })
                .transpose()?;
            let input = KleinInput {
                request_id,
                project_id,
                idempotency_key,
                plan_id,
                prompt: turn.prompt.clone(),
                seed,
                source,
            };
            let frozen = if compact_tier {
                freeze_windows_klein_compact(input, random_seed, environment)
            } else {
                freeze_windows_klein(input, random_seed, environment)
            }
            .map_err(|error| error.to_string())?;
            (frozen.request, frozen.plan)
        } else if turn.model_id == WINDOWS_ZIMAGE_INSTALL_ID {
            let frozen = if compact_tier {
                freeze_windows_z_image_compact(
                    GenerationInput {
                        request_id,
                        project_id,
                        idempotency_key,
                        plan_id,
                        prompt: turn.prompt.clone(),
                        width,
                        height,
                        count: turn.count,
                        seed,
                    },
                    random_seed,
                    environment,
                )
            } else {
                freeze_windows_z_image(
                    GenerationInput {
                        request_id: request_id.clone(),
                        project_id: project_id.clone(),
                        idempotency_key: idempotency_key.clone(),
                        plan_id: plan_id.clone(),
                        prompt: turn.prompt.clone(),
                        width,
                        height,
                        count: turn.count,
                        seed,
                    },
                    random_seed,
                    environment,
                )
            }
            .map_err(|error| error.to_string())?;
            (frozen.request, frozen.plan)
        } else {
            let frozen = freeze_z_image(
                GenerationInput {
                    request_id,
                    project_id,
                    idempotency_key,
                    plan_id,
                    prompt: turn.prompt.clone(),
                    width,
                    height,
                    count: turn.count,
                    seed,
                },
                random_seed,
                environment,
            )
            .map_err(|error| error.to_string())?;
            (frozen.request, frozen.plan)
        };
        crate::adapters::logging::debug("turn.preflight.frozen", &[("turn", turn.id.to_string())]);
        let _schedule = self
            .scheduling
            .lock()
            .map_err(|_| "提交锁异常".to_string())?;
        crate::adapters::logging::debug("turn.preflight.lock", &[("turn", turn.id.to_string())]);
        let binding = self
            .with_library(|library| library.bind_turn_job(&turn.id, request, plan))
            .map_err(|error| format!("{error}（阶段：绑定任务）"))?;
        crate::adapters::logging::info("turn.preflight.bound", &[("turn", turn.id.to_string())]);
        if binding.job.state == studio_core::generation::AttemptState::CancelRequested {
            let cancelled = self.with_library(|library| {
                library.finish_after_reap(
                    &binding.job.attempt,
                    crate::adapters::library::Completion::Cancelled,
                )
            })?;
            let _ = cancelled;
            return Ok(());
        }
        if let Err(error) = self.ensure_open() {
            let _ = self.with_library(|library| {
                library.finish_after_reap(
                    &binding.job.attempt,
                    crate::adapters::library::Completion::Failed {
                        code: "APP_CLOSING".into(),
                        detail: error.clone(),
                    },
                )
            });
            return Err(error);
        }
        let controller = self.generation.as_ref().ok_or("生成控制器不可用")?;
        if let Err(error) = controller
            .start(binding.job.clone(), ready)
            .map_err(|error| format!("{error}（阶段：启动执行）"))
        {
            let _ = self.with_library(|library| {
                library.finish_after_reap(
                    &binding.job.attempt,
                    crate::adapters::library::Completion::Failed {
                        code: "START_FAILED".into(),
                        detail: error.clone(),
                    },
                )
            });
            return Err(error);
        }
        Ok(())
    }
    pub fn conversation_cancel_turn(&self, turn_id: &str) -> Result<Value, String> {
        let id = TurnId::new(turn_id).map_err(|error| error.to_string())?;
        let turn = self.with_library(|library| library.turn(&id))?;
        if let Some(job) = &turn.job {
            let controller = self.generation.as_ref().ok_or("生成控制器不可用")?;
            controller.cancel(&job.job_id)?;
        } else if matches!(
            turn.status,
            TurnStatus::Pending | TurnStatus::CancelRequested
        ) {
            self.with_library(|library| library.request_cancel_unbound_turn(&id))?;
        }
        self.with_library(|library| {
            let turn = library.turn(&id)?;
            turn_view(library, &turn)
        })
    }
    pub fn submit(&self, input: SubmitInput) -> Result<Value, String> {
        let _schedule = self
            .scheduling
            .lock()
            .map_err(|_| "提交锁异常".to_string())?;
        self.ensure_open()?;
        let (width, height) = match input.aspect.as_str() {
            "1:1" => (1024, 1024),
            "4:3" => (1024, 768),
            "3:4" => (768, 1024),
            _ => return Err("当前模型不支持此画幅，请明确修改参数".into()),
        };
        let seed = if input.seed.is_empty() {
            None
        } else {
            if !input.seed.bytes().all(|b| b.is_ascii_digit()) {
                return Err("种子须为u32整数或留空".into());
            }
            Some(
                input
                    .seed
                    .parse::<u32>()
                    .map_err(|_| "种子须为0–4294967295".to_string())?,
            )
        };
        if input.submission_key.len() > 96 {
            return Err("提交键过长".into());
        }
        let key = IdempotencyKey::new(input.submission_key.clone()).map_err(|e| e.to_string())?;
        let exists = self.with_library(|library| library.job_for_submission(&key))?;
        if let Some(job) = exists {
            if job.request.instruction != input.prompt
                || job.request.requested_output.width != width
                || job.request.requested_output.height != height
                || job.request.requested_output.count != input.count
                || job.request.parameters.get("seed") != Some(&json!(seed))
            {
                return Err("同一提交键对应不同内容".into());
            }
            return Ok(job_view(&job));
        }
        let controller = self.generation.as_ref().ok_or("生成控制器不可用")?;
        if controller.is_busy() {
            return Err("生成或旧执行清理仍在进行，请稍后重试".into());
        }
        self.with_library(|library| {
            if library.drafts()?.iter().any(|d| d.id == input.draft_id) {
                Ok(())
            } else {
                Err(crate::adapters::library::LibraryError::Invalid(
                    "草稿不存在".into(),
                ))
            }
        })?;
        let installation = self.installation.as_ref().ok_or("安装器不可用")?;
        let ready = installation.ready_paths(INSTALL_ID)?;
        let environment = installation.plan_environment(INSTALL_ID)?;
        self.ensure_open()?;
        let random = Uuid::new_v4();
        let bytes = random.as_bytes();
        let random_seed = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let frozen = freeze_z_image(
            GenerationInput {
                request_id: RequestId::new(format!("request-{}", input.submission_key))
                    .map_err(|e| e.to_string())?,
                project_id: ProjectId::new("default").map_err(|e| e.to_string())?,
                idempotency_key: key,
                plan_id: PlanId::new(format!("plan-{}", input.submission_key))
                    .map_err(|e| e.to_string())?,
                prompt: input.prompt,
                width,
                height,
                count: input.count,
                seed,
            },
            random_seed,
            environment,
        )
        .map_err(|e| e.to_string())?;
        let submission =
            self.with_library(|library| library.submit(frozen.request, frozen.plan))?;
        if submission.created {
            self.reject_start_while_closing(&submission.job)?;
            controller.start(submission.job.clone(), ready)?;
        }
        Ok(job_view(&submission.job))
    }
    pub fn cancel(&self, id: &str) -> Result<Value, String> {
        let id = JobId::new(id).map_err(|e| e.to_string())?;
        let job = self
            .generation
            .as_ref()
            .ok_or("生成控制器不可用")?
            .cancel(&id)?;
        Ok(job_view(&job))
    }
    pub fn retry(&self, id: &str) -> Result<Value, String> {
        let _schedule = self
            .scheduling
            .lock()
            .map_err(|_| "提交锁异常".to_string())?;
        self.ensure_open()?;
        let controller = self.generation.as_ref().ok_or("生成控制器不可用")?;
        if controller.is_busy() {
            return Err("请等待当前生成和清理完成".into());
        }
        let id = JobId::new(id).map_err(|e| e.to_string())?;
        let installation = self.installation.as_ref().ok_or("安装器不可用")?;
        let previous = self.with_library(|library| library.job(&id))?;
        #[cfg(windows)]
        if previous.request.model_variant_id.as_str() == studio_core::generation::Z_IMAGE_MODEL {
            return Err("该历史任务使用 macOS MFLUX v1，Windows 不会隐式转换执行身份".into());
        }
        // Legacy v1 froze the model variant, which predates the product install ID.
        let install_id = if previous.request.model_variant_id.as_str()
            == studio_core::generation::Z_IMAGE_MODEL
        {
            INSTALL_ID
        } else {
            previous.request.model_variant_id.as_str()
        };
        let ready = installation.ready_paths(install_id)?;
        let environment = installation.plan_environment(install_id)?;
        let scheduler_matches = studio_core::recipes::Recipe::from_request(&previous.request)
            .is_ok_and(|recipe| {
                // Windows recipes freeze their own scheduler/sampling parameters and are checked
                // against the plan environment at freeze time; the legacy macOS comparison below
                // only applies to the v1/v2 macOS recipes.
                recipe.is_windows()
                    || matches!(
                        recipe,
                        studio_core::recipes::Recipe::BiRefNet
                            | studio_core::recipes::Recipe::RealEsrgan
                    )
                    || previous.plan.effective_parameters.get("scheduler")
                        == Some(&json!(environment.scheduler))
            });
        if !retry_components_match(
            &previous.plan.components,
            &environment.components,
            previous.request.model_variant_id.as_str() == studio_core::generation::Z_IMAGE_MODEL,
        ) || previous.plan.runtime != environment.runtime
            || previous.plan.device != environment.device
            || !scheduler_matches
        {
            return Err("原任务的运行环境已变化，不能改写冻结计划；请从草稿创建新任务".into());
        }
        self.ensure_open()?;
        let job = self.with_library(|library| library.retry(&id))?;
        self.reject_start_while_closing(&job)?;
        controller.start(job.clone(), ready)?;
        Ok(job_view(&job))
    }
    pub fn task_list(&self, limit: u32, offset: u32) -> Result<Value, String> {
        let jobs = self.with_library(|library| library.jobs(limit, offset))?;
        let counts = self.with_library(|library| library.job_counts())?;
        Ok(
            json!({"jobs":jobs.iter().map(job_view).collect::<Vec<_>>(),"counts":[counts.0,counts.1,counts.2]}),
        )
    }
    pub fn read_asset(&self, id: &str) -> Result<Vec<u8>, String> {
        let id = AssetId::new(id).map_err(|e| e.to_string())?;
        self.with_library(|library| library.read_asset_png(&id))
    }
    pub fn export(&self, id: &str, path: &Path, overwrite_confirmed: bool) -> Result<(), String> {
        let id = AssetId::new(id).map_err(|e| e.to_string())?;
        self.with_library(|library| library.export_asset_png(&id, path, overwrite_confirmed))
    }
    pub fn shutdown(&self) {
        if let Some(generation) = &self.generation {
            generation.shutdown();
        }
        if let Some(installation) = &self.installation {
            installation.shutdown();
        }
    }
    fn ensure_open(&self) -> Result<(), String> {
        if self.closing.load(Ordering::Acquire) {
            Err("应用正在关闭，未启动新的执行".into())
        } else {
            Ok(())
        }
    }
    fn reject_start_while_closing(&self, job: &JobRecord) -> Result<(), String> {
        if let Err(error) = self.ensure_open() {
            self.with_library(|library| {
                library.finish_after_reap(
                    &job.attempt,
                    crate::adapters::library::Completion::Failed {
                        code: "APP_CLOSING".into(),
                        detail: error.clone(),
                    },
                )
            })?;
            return Err(error);
        }
        Ok(())
    }
    pub fn allow_close(&self) {
        self.closing.store(true, Ordering::Release);
    }
}

pub fn job_view(job: &JobRecord) -> Value {
    json!({"id":job.job_id,"turnId":job.turn_id,"conversationId":job.conversation_id,"attemptId":job.attempt.attempt_id,"state":format!("{:?}",job.state),"prompt":job.request.instruction,"width":job.plan.effective_output.width,"height":job.plan.effective_output.height,"seed":job.plan.effective_parameters.get("seed"),"error":job.error,"assetId":job.manifest.as_ref().and_then(|m|m.outputs.first()).map(|a|a.asset_id.clone()),"manifest":job.manifest})
}

fn retry_components_match(
    frozen: &[studio_core::contracts::ComponentSnapshot],
    installed: &[studio_core::contracts::ComponentSnapshot],
    legacy_z_image: bool,
) -> bool {
    const PREFIX: &str = "https://huggingface.co/Tongyi-MAI/Z-Image-Turbo/";
    frozen.len() == installed.len()
        && frozen.iter().zip(installed).all(|(old, current)| {
            old == current
                || legacy_z_image
                    && old.role == current.role
                    && old.content_hash == current.content_hash
                    && old.source.revision == current.source.revision
                    && old.source.source.strip_prefix(PREFIX).is_some_and(|path| {
                        current.source.source
                            == format!("{PREFIX}resolve/{}/{path}", old.source.revision)
                    })
        })
}

fn turn_view(library: &Library, turn: &TurnRecord) -> crate::adapters::library::Result<Value> {
    let fallback_state = match turn.status {
        TurnStatus::Pending => "Pending",
        TurnStatus::CancelRequested => "CancelRequested",
        TurnStatus::Cancelled => "Cancelled",
        TurnStatus::Rejected => "Rejected",
        TurnStatus::Interrupted => "Interrupted",
        TurnStatus::Job => "Submitted",
    };
    let job_id = turn.job.as_ref().map(|job| job.job_id.clone());
    let attempt_id = turn.job.as_ref().map(|job| job.attempt.attempt_id.clone());
    let state = turn
        .job
        .as_ref()
        .map_or_else(|| fallback_state.into(), |job| format!("{:?}", job.state));
    let width = turn.job.as_ref().map_or_else(
        || display_width(&turn.aspect),
        |job| job.plan.effective_output.width,
    );
    let height = turn.job.as_ref().map_or_else(
        || display_height(&turn.aspect),
        |job| job.plan.effective_output.height,
    );
    let seed = turn
        .job
        .as_ref()
        .and_then(|job| job.plan.effective_parameters.get("seed"))
        .cloned()
        .unwrap_or(Value::Null);
    let output_ids = turn
        .job
        .as_ref()
        .and_then(|job| job.manifest.as_ref())
        .map(|manifest| {
            manifest
                .outputs
                .iter()
                .map(|asset| asset.asset_id.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let asset_for_role = |role: &str| {
        output_ids.iter().find_map(|asset_id| {
            library
                .asset(asset_id)
                .ok()
                .filter(|asset| asset.output_role == role)
        })
    };
    let primary_record = asset_for_role("primary");
    let mask_record = asset_for_role("mask");
    let asset_id = primary_record
        .as_ref()
        .map(|asset| asset.asset.asset_id.to_string());
    let mask_asset_id = mask_record
        .as_ref()
        .map(|asset| asset.asset.asset_id.to_string());
    let asset_record = primary_record;
    let asset_available = asset_record.as_ref().is_some_and(|asset| asset.available);
    let asset_deletion_state = asset_record
        .as_ref()
        .map(|asset| asset.deletion_state.clone());
    let mask_deletion_state = mask_record
        .as_ref()
        .map(|asset| asset.deletion_state.clone());
    Ok(json!({
        "id": turn.id,
        "conversationId": turn.conversation_id,
        "sequence": turn.sequence,
        "submissionKey": turn.submission_key,
        "prompt": turn.prompt,
        "modelId": turn.model_id,
        "capabilityId": turn.capability_id,
        "sourceAssetId": turn.source_asset_id,
        "aspect": turn.aspect,
        "count": turn.count,
        "seed": seed,
        "requestedSeed": turn.seed,
        "status": format!("{:?}", turn.status),
        "state": state,
        "jobId": job_id,
        "attemptId": attempt_id,
        "width": width,
        "height": height,
        "assetId": asset_id,
        "assetAvailable": asset_available,
        "assetError": asset_record.and_then(|asset| asset.availability_error),
        "assetDeletionState": asset_deletion_state,
        "maskAssetId": mask_asset_id,
        "maskAssetAvailable": mask_record.as_ref().is_some_and(|asset| asset.available),
        "maskAssetError": mask_record.and_then(|asset| asset.availability_error),
        "maskAssetDeletionState": mask_deletion_state,
        "error": turn.error.as_ref().or_else(|| turn.job.as_ref().and_then(|job| job.error.as_ref())),
        "progressStage": turn.progress_stage,
        "progressStep": turn.progress_step,
        "progressTotal": turn.progress_total,
        "receivedAt": turn.received_at
    }))
}

fn display_width(aspect: &str) -> u32 {
    if aspect == "3:4" { 768 } else { 1024 }
}

fn display_height(aspect: &str) -> u32 {
    if aspect == "4:3" { 768 } else { 1024 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    fn fixture() -> (tempfile::TempDir, StudioService) {
        let root = tempfile::tempdir().unwrap();
        let mut service = StudioService::new(
            AppPaths::resolve(Some(root.path())).unwrap(),
            root.path().join("not-installed-supervisor"),
        );
        service.device_facts = DeviceFacts {
            vram_gb: Some(12),
            ram_gb: 32,
            compute_capability: Some((8, 6)),
        };
        (root, service)
    }
    #[test]
    fn legacy_source_url_notation_does_not_change_frozen_component_identity() {
        use studio_core::contracts::{ComponentSnapshot, SourceRevision};
        let installed = ComponentSnapshot {
            role: "component-00-transformer".into(),
            content_hash: ContentHash::new("a".repeat(64)).unwrap(),
            source: SourceRevision { source: "https://huggingface.co/Tongyi-MAI/Z-Image-Turbo/resolve/fixed/transformer/model.safetensors".into(), revision: "fixed".into() },
        };
        let mut old = installed.clone();
        old.source.source =
            "https://huggingface.co/Tongyi-MAI/Z-Image-Turbo/transformer/model.safetensors".into();
        assert!(retry_components_match(
            std::slice::from_ref(&old),
            std::slice::from_ref(&installed),
            true
        ));
        assert!(!retry_components_match(
            std::slice::from_ref(&old),
            std::slice::from_ref(&installed),
            false
        ));
        for field in ["hash", "revision", "role", "repository"] {
            let mut wrong = old.clone();
            match field {
                "hash" => wrong.content_hash = ContentHash::new("b".repeat(64)).unwrap(),
                "revision" => wrong.source.revision = "other".into(),
                "role" => wrong.role = "other".into(),
                _ => {
                    wrong.source.source =
                        "https://huggingface.co/other/repo/transformer/model.safetensors".into()
                }
            }
            assert!(
                !retry_components_match(&[wrong], std::slice::from_ref(&installed), true),
                "{field}"
            );
        }
        assert!(!retry_components_match(&[old], &[], true));
    }

    #[test]
    fn legacy_v1_retry_checks_the_registered_z_image_installation() {
        use studio_core::{
            contracts::{ComponentSnapshot, DeviceSnapshot, RuntimeSnapshot, SourceRevision},
            generation::PlanEnvironment,
        };
        let (_root, service) = fixture();
        let frozen = freeze_z_image(
            GenerationInput {
                request_id: RequestId::new("legacy-retry-request").unwrap(),
                project_id: ProjectId::new("default").unwrap(),
                idempotency_key: IdempotencyKey::new("legacy-retry-key").unwrap(),
                plan_id: PlanId::new("legacy-retry-plan").unwrap(),
                prompt: "Legacy v1 image".into(),
                width: 1024,
                height: 1024,
                count: 1,
                seed: Some(42),
            },
            42,
            PlanEnvironment {
                components: vec![ComponentSnapshot {
                    role: "transformer".into(),
                    source: SourceRevision {
                        source: "fixture:legacy-model".into(),
                        revision: "fixed".into(),
                    },
                    content_hash: ContentHash::new("a".repeat(64)).unwrap(),
                }],
                runtime: RuntimeSnapshot {
                    package_id: "fixture-runtime".into(),
                    engine: "fixture".into(),
                    source: SourceRevision {
                        source: "fixture:legacy-runtime".into(),
                        revision: "fixed".into(),
                    },
                },
                device: DeviceSnapshot {
                    device_id: "fixture-device".into(),
                    precision: "fixture".into(),
                },
                scheduler: "linear".into(),
            },
        )
        .unwrap();
        let submitted = service
            .with_library(|library| library.submit(frozen.request, frozen.plan))
            .unwrap();
        let error = service.retry(submitted.job.job_id.as_str()).unwrap_err();
        assert!(!error.contains("未登记的安装标识"), "{error}");
        let models = service.model_views();
        #[cfg(windows)]
        {
            assert!(error.contains("Windows 不会隐式转换执行身份"), "{error}");
            assert_eq!(models.len(), 7);
            assert_eq!(models[0]["id"], WINDOWS_ZIMAGE_INSTALL_ID);
            // The profile may be reported as not_installed (registered profile) or failed
            // (installer unavailable in the fixture); both are honest "not ready" states.
            assert!(
                matches!(
                    models[0]["phase"].as_str(),
                    Some("not_installed" | "failed")
                ),
                "{}",
                models[0]["phase"]
            );
            for model in &models {
                assert_eq!(model["ready"], false, "{}", model["id"]);
            }
        }
        #[cfg(not(windows))]
        {
            assert_eq!(models[0]["phase"], "unavailable");
            assert!(
                models[1..]
                    .iter()
                    .all(|model| model["phase"] == "not_installed")
            );
        }
    }
    #[test]
    fn empty_library_can_save_and_restore_without_models() {
        let (root, service) = fixture();
        let first = service.snapshot().unwrap();
        assert_eq!(first["model"]["ready"], false);
        assert_eq!(first["drafts"][0]["id"], 1);
        assert_eq!(first["jobCounts"], json!([0, 0, 0]));
        let record = service
            .save_draft(
                DraftInput {
                    id: 1,
                    name: "草稿 1".into(),
                    prompt: "中文\nA quiet lake".into(),
                    params: json!({
                        "modelId": DEFAULT_MODEL_ID,
                        "aspect": if cfg!(windows) { "1:1" } else { "4:3" },
                        "count": 1,
                        "seed": "42"
                    }),
                    collapsed: true,
                    prompt_source_id: None,
                },
                1,
            )
            .unwrap();
        assert_eq!(record.version, 2);
        drop(service);
        let restored = StudioService::new(
            AppPaths::resolve(Some(root.path())).unwrap(),
            root.path().join("not-installed-supervisor"),
        );
        assert_eq!(
            restored.snapshot().unwrap()["drafts"][0]["prompt"],
            "中文\nA quiet lake"
        );
    }
    #[test]
    fn ipc_inputs_do_not_expose_paths_and_unready_submit_creates_no_job() {
        assert!(serde_json::from_value::<DraftInput>(json!({"id":1,"name":"x","prompt":"x","params":{},"collapsed":false,"path":"/tmp/forbidden"})).is_err());
        let (_root, service) = fixture();
        assert!(
            service
                .submit(SubmitInput {
                    draft_id: 1,
                    prompt: "湖泊".into(),
                    aspect: "1:1".into(),
                    count: 1,
                    seed: "".into(),
                    submission_key: "intent-one".into()
                })
                .is_err()
        );
        assert_eq!(service.snapshot().unwrap()["jobCounts"], json!([0, 0, 0]));
        assert!(
            service
                .save_draft(
                    DraftInput {
                        id: 1,
                        name: "草稿 1".into(),
                        prompt: "preserved".into(),
                        params: json!({"modelId":"z-image-turbo-mflux-v1","aspect":{},"count":1,"seed":""}),
                        collapsed: false,
                        prompt_source_id: None,
                    },
                    1
                )
                .is_err()
        );
    }

    #[test]
    fn conversation_receive_returns_quickly_and_missing_model_settles_rejected() {
        let root = tempfile::tempdir().unwrap();
        let mut service = StudioService::new(
            AppPaths::resolve(Some(root.path())).unwrap(),
            root.path().join("not-installed-supervisor"),
        );
        service.device_facts = DeviceFacts {
            vram_gb: Some(12),
            ram_gb: 32,
            compute_capability: Some((8, 6)),
        };
        let service = Arc::new(service);
        let typed = service
            .save_draft(
                DraftInput {
                    id: 1,
                    name: "新会话".into(),
                    prompt: "接收后异步预检".into(),
                    params: json!({
                        "modelId": DEFAULT_MODEL_ID,
                        "aspect": "1:1",
                        "count": 1,
                        "seed": ""
                    }),
                    collapsed: false,
                    prompt_source_id: None,
                },
                1,
            )
            .unwrap();
        let conversation = service
            .snapshot()
            .unwrap()
            .get("activeConversationId")
            .and_then(Value::as_str)
            .unwrap()
            .to_owned();
        let response = service
            .conversation_send(ConversationSendInput {
                conversation_id: conversation,
                draft_id: 1,
                expected_version: typed.version,
                prompt: "接收后异步预检".into(),
                model_id: DEFAULT_MODEL_ID.into(),
                capability_id: None,
                source_asset_id: None,
                aspect: "1:1".into(),
                count: 1,
                seed: String::new(),
                submission_key: "conversation-send-fast".into(),
            })
            .unwrap();
        assert!(matches!(
            response["turn"]["state"].as_str(),
            Some("Pending" | "Rejected")
        ));
        assert_eq!(service.snapshot().unwrap()["drafts"][0]["prompt"], "");
        assert_eq!(service.snapshot().unwrap()["jobCounts"], json!([0, 0, 0]));
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let status = service.snapshot().unwrap()["turns"][0]["state"]
                .as_str()
                .unwrap()
                .to_owned();
            if status == "Rejected" {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "preflight did not settle: {status}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(service.snapshot().unwrap()["jobCounts"], json!([0, 0, 0]));
    }

    #[test]
    fn new_conversation_uses_persisted_defaults_and_explicit_override() {
        let (_root, service) = fixture();
        #[cfg(windows)]
        {
            assert!(
                service
                    .save_creation_defaults(CreationDefaults {
                        model_id: DEFAULT_MODEL_ID.into(),
                        aspect: "4:3".into(),
                        count: 1,
                        seed: "42".into(),
                    })
                    .is_err()
            );
            service
                .save_creation_defaults(CreationDefaults {
                    model_id: DEFAULT_MODEL_ID.into(),
                    aspect: "1:1".into(),
                    count: 1,
                    seed: "42".into(),
                })
                .unwrap();
            let persisted = service.conversation_create(None).unwrap();
            assert_eq!(persisted["draft"]["params"]["aspect"], "1:1");
            assert_eq!(persisted["draft"]["params"]["seed"], "42");
            assert!(
                service
                    .conversation_create(Some(ConversationCreateInput {
                        model_id: Some(DEFAULT_MODEL_ID.into()),
                        aspect: Some("3:4".into()),
                        count: Some(1),
                        seed: Some(String::new()),
                        initial_title: None,
                        initial_prompt: None,
                        prompt_source_id: None,
                    }))
                    .is_err()
            );
            let overridden = service
                .conversation_create(Some(ConversationCreateInput {
                    model_id: Some(DEFAULT_MODEL_ID.into()),
                    aspect: Some("1:1".into()),
                    count: Some(1),
                    seed: Some(String::new()),
                    initial_title: None,
                    initial_prompt: None,
                    prompt_source_id: None,
                }))
                .unwrap();
            assert_eq!(overridden["draft"]["params"]["aspect"], "1:1");
            assert_eq!(overridden["draft"]["params"]["seed"], "");
        }
        #[cfg(not(windows))]
        {
            service
                .save_creation_defaults(CreationDefaults {
                    model_id: DEFAULT_MODEL_ID.into(),
                    aspect: "4:3".into(),
                    count: 1,
                    seed: "42".into(),
                })
                .unwrap();
            let persisted = service.conversation_create(None).unwrap();
            assert_eq!(persisted["draft"]["params"]["aspect"], "4:3");
            assert_eq!(persisted["draft"]["params"]["seed"], "42");

            let overridden = service
                .conversation_create(Some(ConversationCreateInput {
                    model_id: Some(DEFAULT_MODEL_ID.into()),
                    aspect: Some("3:4".into()),
                    count: Some(1),
                    seed: Some(String::new()),
                    initial_title: None,
                    initial_prompt: None,
                    prompt_source_id: None,
                }))
                .unwrap();
            assert_eq!(overridden["draft"]["params"]["aspect"], "3:4");
            assert_eq!(overridden["draft"]["params"]["seed"], "");
        }
    }

    #[test]
    #[cfg(not(windows))]
    fn qwen_and_moss_defaults_require_an_explicit_square_aspect() {
        let (_root, service) = fixture();
        for model_id in [
            studio_core::recipes::QWEN_IMAGE_21_MODEL,
            studio_core::recipes::QWEN_IMAGE_21_TURBO_MODEL,
            studio_core::recipes::MOSS_IMAGE_SFT_MODEL,
        ] {
            let invalid = CreationDefaults {
                model_id: model_id.into(),
                aspect: "4:3".into(),
                count: 1,
                seed: "42".into(),
            };
            assert!(service.save_creation_defaults(invalid).is_err());
            let valid = CreationDefaults {
                model_id: model_id.into(),
                aspect: "1:1".into(),
                count: 1,
                seed: "42".into(),
            };
            service.save_creation_defaults(valid).unwrap();
            let created = service.conversation_create(None).unwrap();
            assert_eq!(created["draft"]["params"]["modelId"], model_id);
            assert_eq!(created["draft"]["params"]["aspect"], "1:1");
            assert_eq!(created["draft"]["params"]["seed"], "42");
        }
    }

    #[test]
    fn invalid_create_override_is_rejected_without_creating_a_conversation() {
        let (_root, service) = fixture();
        let before = service.snapshot().unwrap()["conversations"]
            .as_array()
            .unwrap()
            .len();
        assert!(
            service
                .conversation_create(Some(ConversationCreateInput {
                    model_id: Some("not-registered".into()),
                    aspect: None,
                    count: None,
                    seed: None,
                    initial_title: None,
                    initial_prompt: None,
                    prompt_source_id: None,
                }))
                .is_err()
        );
        let after = service.snapshot().unwrap()["conversations"]
            .as_array()
            .unwrap()
            .len();
        assert_eq!(before, after);
    }

    #[test]
    fn inspiration_prompt_is_created_with_resolved_defaults_and_source() {
        let (_root, service) = fixture();
        let created = service
            .conversation_create(Some(ConversationCreateInput {
                model_id: None,
                aspect: None,
                count: None,
                seed: None,
                initial_title: Some("秋季咖啡海报".into()),
                initial_prompt: Some("完整案例提示词\n[COFFEE]".into()),
                prompt_source_id: Some("awesome-gpt-image-2:case:544".into()),
            }))
            .unwrap();
        assert_eq!(created["conversation"]["title"], "秋季咖啡海报");
        assert_eq!(created["conversation"]["titleSource"], "manual");
        assert_eq!(created["draft"]["prompt"], "完整案例提示词\n[COFFEE]");
        assert_eq!(
            created["draft"]["promptSourceId"],
            "awesome-gpt-image-2:case:544"
        );
        assert_eq!(created["draft"]["params"]["modelId"], DEFAULT_MODEL_ID);
        assert_eq!(service.snapshot().unwrap()["jobCounts"], json!([0, 0, 0]));
    }

    #[test]
    fn invalid_inspiration_source_is_rejected_without_a_partial_conversation() {
        let (_root, service) = fixture();
        let before = service.snapshot().unwrap()["conversations"]
            .as_array()
            .unwrap()
            .len();
        assert!(
            service
                .conversation_create(Some(ConversationCreateInput {
                    model_id: None,
                    aspect: None,
                    count: None,
                    seed: None,
                    initial_title: Some("案例".into()),
                    initial_prompt: Some("提示词".into()),
                    prompt_source_id: Some("https://unregistered.example/case".into()),
                }))
                .is_err()
        );
        assert_eq!(
            service.snapshot().unwrap()["conversations"]
                .as_array()
                .unwrap()
                .len(),
            before
        );
    }

    #[test]
    fn removed_saved_model_is_preserved_and_reported() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::resolve(Some(root.path())).unwrap();
        std::fs::write(
            paths.data.join("application-preferences.json"),
            r#"{
              "schemaVersion": 1,
              "appearance": {
                "theme": "light",
                "fontScale": "default",
                "density": "comfortable",
                "reducedMotion": false,
                "sidebarCollapsed": false
              },
              "creationDefaults": {
                "modelId": "removed-model",
                "aspect": "4:3",
                "count": 1,
                "seed": "9"
              }
            }"#,
        )
        .unwrap();
        let service = StudioService::new(paths, root.path().join("not-installed-supervisor"));
        let created = service.conversation_create(None).unwrap();
        assert_eq!(created["draft"]["params"]["modelId"], "removed-model");
        assert_eq!(created["draft"]["params"]["aspect"], "4:3");
        assert_eq!(created["draft"]["params"]["seed"], "9");
        assert!(
            created["defaultsWarnings"]
                .as_array()
                .is_some_and(|warnings| !warnings.is_empty())
        );
    }

    #[cfg(windows)]
    #[test]
    fn mac_defaults_are_preserved_until_the_user_selects_windows() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::resolve(Some(root.path())).unwrap();
        std::fs::write(
            paths.data.join("application-preferences.json"),
            r#"{
              "schemaVersion": 1,
              "appearance": {
                "theme": "light",
                "fontScale": "default",
                "density": "comfortable",
                "reducedMotion": false,
                "sidebarCollapsed": false
              },
              "creationDefaults": {
                "modelId": "z-image-turbo-mflux-v1",
                "aspect": "4:3",
                "count": 1,
                "seed": "17"
              }
            }"#,
        )
        .unwrap();
        let service = StudioService::new(paths, root.path().join("not-installed-supervisor"));
        let created = service.conversation_create(None).unwrap();
        assert_eq!(
            created["draft"]["params"]["modelId"],
            "z-image-turbo-mflux-v1"
        );
        assert_eq!(created["draft"]["params"]["aspect"], "4:3");
        assert_eq!(
            created["draft"]["params"]["capabilityId"],
            studio_core::generation::Z_IMAGE_CAPABILITY
        );
        assert!(
            created["defaultsWarnings"]
                .as_array()
                .is_some_and(|warnings| !warnings.is_empty())
        );
        assert!(
            service
                .validate_registered_draft_params(&created["draft"]["params"])
                .is_ok()
        );
        let saved = service
            .save_draft(
                DraftInput {
                    id: created["draft"]["id"].as_u64().unwrap(),
                    name: created["draft"]["name"].as_str().unwrap().into(),
                    prompt: "保留旧模型时仍可保存文字".into(),
                    params: created["draft"]["params"].clone(),
                    collapsed: false,
                    prompt_source_id: None,
                },
                created["draft"]["version"].as_u64().unwrap(),
            )
            .unwrap();
        assert_eq!(saved.prompt, "保留旧模型时仍可保存文字");
        assert_eq!(
            saved.params["capabilityId"],
            studio_core::generation::Z_IMAGE_CAPABILITY
        );
    }

    #[test]
    fn catalog_query_exposes_independent_model_install_actions() {
        let (_root, service) = fixture();
        let catalog = service.model_catalog().unwrap();
        assert_eq!(catalog["counts"]["total"], 7);
        let klein_id = if cfg!(windows) {
            WINDOWS_KLEIN_MODEL
        } else {
            "flux2-klein-4b-distilled"
        };
        let klein = catalog["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == klein_id)
            .unwrap();
        assert_eq!(klein["productStatus"], "implemented");
        assert_eq!(klein["executable"], false);
        // Install/use entry points also require the device to satisfy the registered thresholds.
        assert_eq!(klein["canInstall"], klein["hardwareEligible"]);
    }

    #[test]
    fn settings_snapshot_reports_the_current_platform_runtime() {
        let (_root, service) = fixture();
        let snapshot = service.settings_snapshot(false).unwrap();
        assert_eq!(snapshot["catalog"]["counts"]["total"], 7);
        assert_eq!(
            snapshot["about"]["modelSources"].as_array().unwrap().len(),
            7
        );
        let runtimes = snapshot["environment"]["runtimes"].as_array().unwrap();
        let snapshots = service.installation.as_ref().unwrap().snapshots();
        assert_eq!(runtimes.len(), snapshots.len());
        for (row, install) in runtimes.iter().zip(&snapshots) {
            assert_eq!(row["installId"], install.install_id);
            assert!(row["label"].is_string());
        }
        #[cfg(windows)]
        {
            assert_eq!(
                snapshot["environment"]["runtimeName"],
                "Diffusers CUDA v1 + v2 共享运行时"
            );
            assert_eq!(snapshot["environment"]["pythonVersion"], "3.12.10");
            assert_eq!(runtimes[0]["installId"], WINDOWS_ZIMAGE_INSTALL_ID);
            assert_eq!(
                runtimes[0]["runtimeLockId"],
                studio_core::generation::Z_IMAGE_WINDOWS_RUNTIME
            );
            if let Some(klein) = runtimes
                .iter()
                .find(|row| row["installId"] == WINDOWS_KLEIN_MODEL)
            {
                assert_eq!(
                    klein["runtimeLockId"],
                    studio_core::generation::WINDOWS_V2_RUNTIME
                );
                assert!(klein["hardwareEligible"].is_boolean());
            }
            assert!(
                snapshot["about"]["capabilityScope"]
                    .as_str()
                    .is_some_and(|scope| scope.contains("七个型号"))
            );
            assert!(snapshot["environment"]["deviceFacts"].is_string());
        }
        #[cfg(not(windows))]
        {
            assert_eq!(
                snapshot["environment"]["runtimeName"],
                "MFLUX / MLX + ONNX Runtime CPU + ncnn Vulkan"
            );
            assert_eq!(snapshot["environment"]["pythonVersion"], "3.13.12");
            assert_eq!(runtimes.len(), 7);
            for row in runtimes {
                assert_eq!(row["runtimeLockId"], Value::Null);
            }
        }
    }

    #[test]
    fn capability_drafts_survive_tool_switches_and_restart_without_consuming_text() {
        let (root, service) = fixture();
        let prompt = "中文原文".repeat(12_000);
        service.save_draft(DraftInput {
            id: 1, name: "能力隔离".into(), prompt: prompt.clone(),
            params: json!({"modelId": DEFAULT_MODEL_ID, "aspect":"1:1", "count":1,"seed":"73"}),
            collapsed:false, prompt_source_id:None,
        }, 1).unwrap();
        let conversation = service.snapshot().unwrap()["activeConversationId"]
            .as_str()
            .unwrap()
            .to_owned();
        let switched = service
            .with_library(|library| {
                library.apply_model_to_conversation(
                    &ConversationId::new(&conversation).unwrap(),
                    studio_core::recipes::REALESRGAN_MODEL,
                )
            })
            .unwrap();
        assert!(switched.prompt.is_empty());
        assert_eq!(switched.params["seed"], "");
        service
            .validate_registered_draft_params(&switched.params)
            .unwrap();
        assert_eq!(
            switched.params["capabilityDrafts"][default_capability_id()]["prompt"],
            prompt
        );
        drop(service);
        let restored = StudioService::new(
            AppPaths::resolve(Some(root.path())).unwrap(),
            root.path().join("not-installed-supervisor"),
        );
        let switched_back = restored
            .with_library(|library| {
                library.apply_model_to_conversation(
                    &ConversationId::new(&conversation).unwrap(),
                    DEFAULT_MODEL_ID,
                )
            })
            .unwrap();
        assert_eq!(switched_back.prompt, prompt);
        assert_eq!(switched_back.params["aspect"], "1:1");
        assert_eq!(switched_back.params["seed"], "73");
        assert_eq!(restored.snapshot().unwrap()["jobCounts"], json!([0, 0, 0]));
        let editable = restored
            .save_draft(
                DraftInput {
                    id: 1,
                    name: "能力隔离".into(),
                    prompt: "可提交的短提示词".into(),
                    params: switched_back.params.clone(),
                    collapsed: false,
                    prompt_source_id: None,
                },
                switched_back.version,
            )
            .unwrap();
        let accepted = restored
            .with_library(|library| {
                library.accept_turn(AcceptTurnInput {
                    conversation_id: ConversationId::new(&conversation).unwrap(),
                    draft_id: 1,
                    expected_version: editable.version,
                    submission_key: IdempotencyKey::new("scoped-draft-send").unwrap(),
                    prompt: editable.prompt.clone(),
                    model_id: DEFAULT_MODEL_ID.into(),
                    capability_id: default_capability_id(),
                    source_asset_id: None,
                    aspect: "1:1".into(),
                    count: 1,
                    seed: "73".into(),
                })
            })
            .unwrap();
        assert!(accepted.draft.prompt.is_empty());
        assert_eq!(
            accepted.draft.params["capabilityDrafts"],
            editable.params["capabilityDrafts"]
        );
        let mut invalid = switched_back.params.clone();
        invalid["capabilityDrafts"]["unknown.capability"] = json!({"prompt":"", "aspect":"1:1", "seed":"", "sourceAssetId":null,"promptSourceId":null});
        assert!(restored.validate_registered_draft_params(&invalid).is_err());
        invalid = switched_back.params;
        invalid["capabilityDrafts"][studio_core::recipes::REALESRGAN_CAPABILITY]["seed"] =
            json!("42");
        assert!(restored.validate_registered_draft_params(&invalid).is_err());
    }

    #[test]
    fn applying_a_model_to_one_conversation_preserves_its_prompt_and_other_settings() {
        let (_root, service) = fixture();
        let record = service
            .save_draft(
                DraftInput {
                    id: 1,
                    name: "会话 A".into(),
                    prompt: "保持这段提示词".into(),
                    params: json!({
                        "modelId": DEFAULT_MODEL_ID,
                        "aspect": "1:1",
                        "count": 1,
                        "seed": "17"
                    }),
                    collapsed: false,
                    prompt_source_id: None,
                },
                1,
            )
            .unwrap();
        let current = service.snapshot().unwrap();
        let conversation = current["activeConversationId"].as_str().unwrap();
        let changed = service
            .with_library(|library| {
                library.apply_model_to_conversation(
                    &ConversationId::new(conversation).unwrap(),
                    "flux2-klein-4b-distilled",
                )
            })
            .unwrap();
        assert_eq!(changed.version, record.version + 1);
        assert_eq!(changed.prompt, "保持这段提示词");
        assert_eq!(changed.params["modelId"], "flux2-klein-4b-distilled");
        assert_eq!(changed.params["aspect"], "1:1");
        assert_eq!(changed.params["seed"], "17");

        let second = service.conversation_create(None).unwrap();
        let second_conversation = second["conversation"]["id"].as_str().unwrap();
        let second_draft = second["draft"]["id"].as_u64().unwrap();
        let second_record = service
            .save_draft(
                DraftInput {
                    id: second_draft,
                    name: "会话 B".into(),
                    prompt: "B 的提示词保持不变".into(),
                    params: json!({
                        "modelId": "flux2-klein-4b-distilled",
                        "aspect": "1:1",
                        "count": 1,
                        "seed": "",
                        "capabilityId": "image.text-to-image.flux2-klein-4b.v1",
                        "sourceAssetId": null
                    }),
                    collapsed: false,
                    prompt_source_id: None,
                },
                second["draft"]["version"].as_u64().unwrap(),
            )
            .unwrap();
        service
            .with_library(|library| {
                library.apply_model_to_conversation(
                    &ConversationId::new(conversation).unwrap(),
                    "flux2-klein-4b-distilled",
                )
            })
            .unwrap();
        let snapshot = service.snapshot().unwrap();
        let b = snapshot["drafts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|draft| draft["id"] == second_draft)
            .unwrap();
        assert_eq!(b["version"], second_record.version);
        assert_eq!(b["prompt"], "B 的提示词保持不变");
        assert_eq!(b["params"]["aspect"], "1:1");
        let selected = snapshot["activeConversationId"].as_str().unwrap();
        assert_eq!(selected, second_conversation);
    }

    #[test]
    #[cfg(not(windows))]
    fn applying_new_text_models_preserves_incompatible_aspect_until_user_edits() {
        let (_root, service) = fixture();
        service
            .save_draft(
                DraftInput {
                    id: 1,
                    name: "会话".into(),
                    prompt: "保持提示词与画幅".into(),
                    params: json!({
                        "modelId": DEFAULT_MODEL_ID,
                        "aspect": "4:3",
                        "count": 1,
                        "seed": "17"
                    }),
                    collapsed: false,
                    prompt_source_id: None,
                },
                1,
            )
            .unwrap();
        let snapshot = service.snapshot().unwrap();
        let conversation =
            ConversationId::new(snapshot["activeConversationId"].as_str().unwrap()).unwrap();
        for (model_id, capability) in [
            (
                studio_core::recipes::QWEN_IMAGE_21_MODEL,
                studio_core::recipes::QWEN_IMAGE_21_CAPABILITY,
            ),
            (
                studio_core::recipes::QWEN_IMAGE_21_TURBO_MODEL,
                studio_core::recipes::QWEN_IMAGE_21_TURBO_CAPABILITY,
            ),
            (
                studio_core::recipes::MOSS_IMAGE_SFT_MODEL,
                studio_core::recipes::MOSS_IMAGE_SFT_CAPABILITY,
            ),
        ] {
            let changed = service
                .with_library(|library| {
                    library.apply_model_to_conversation(&conversation, model_id)
                })
                .unwrap();
            assert_eq!(changed.prompt, "保持提示词与画幅");
            assert_eq!(changed.params["modelId"], model_id);
            assert_eq!(changed.params["capabilityId"], capability);
            assert_eq!(changed.params["aspect"], "4:3");
            assert_eq!(changed.params["seed"], "17");
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_model_views_project_the_seven_models_with_hardware_facts() {
        let (_root, service) = fixture();
        let models = service.model_views();
        let expected = [
            WINDOWS_ZIMAGE_INSTALL_ID,
            WINDOWS_KLEIN_MODEL,
            WINDOWS_BIREFNET_MODEL,
            WINDOWS_REALESRGAN_MODEL,
            WINDOWS_QWEN_MODEL,
            WINDOWS_QWEN_TURBO_MODEL,
            WINDOWS_MOSS_MODEL,
        ];
        assert_eq!(
            models
                .iter()
                .map(|model| model["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            expected
        );
        for model in &models {
            let facts = &model["staticFacts"];
            assert!(facts["hardwareEligible"].is_boolean(), "{}", model["id"]);
            assert!(facts["deviceFacts"].is_string(), "{}", model["id"]);
            assert!(facts["runtimeLockId"].is_string(), "{}", model["id"]);
        }
        assert_eq!(
            models[0]["staticFacts"]["runtimeLockId"],
            studio_core::generation::Z_IMAGE_WINDOWS_RUNTIME
        );
        assert_eq!(
            models[3]["staticFacts"]["runtimeLockId"],
            studio_core::generation::REALESRGAN_WINDOWS_RUNTIME
        );
        assert_eq!(
            models[1]["staticFacts"]["runtimeLockId"],
            studio_core::generation::WINDOWS_V2_RUNTIME
        );
        let klein = &models[1]["staticFacts"];
        assert_eq!(klein["minVramGb"], 8);
        assert_eq!(klein["standardMinVramGb"], 12);
        assert_eq!(klein["minRamGb"], 32);
        assert_eq!(klein["tier"], "standard");
        assert_eq!(klein["resolution"], 1024);
        assert!(klein["licenseNotice"].as_str().unwrap().contains("Apache"));
        let qwen = &models[4]["staticFacts"];
        assert!(qwen["licenseNotice"].as_str().unwrap().contains("非商业"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_capability_mapping_uses_the_frozen_identities() {
        use studio_core::generation as windows;
        for (model, capability) in [
            (WINDOWS_KLEIN_MODEL, windows::KLEIN_WINDOWS_TEXT_CAPABILITY),
            (WINDOWS_BIREFNET_MODEL, windows::BIREFNET_WINDOWS_CAPABILITY),
            (
                WINDOWS_REALESRGAN_MODEL,
                windows::REALESRGAN_WINDOWS_CAPABILITY,
            ),
            (
                WINDOWS_QWEN_MODEL,
                windows::QWEN_IMAGE_21_WINDOWS_CAPABILITY,
            ),
            (
                WINDOWS_QWEN_TURBO_MODEL,
                windows::QWEN_IMAGE_21_TURBO_WINDOWS_CAPABILITY,
            ),
            (
                WINDOWS_MOSS_MODEL,
                windows::MOSS_IMAGE_SFT_WINDOWS_CAPABILITY,
            ),
        ] {
            assert_eq!(capability_for_model(model), capability, "{model}");
            assert!(model_accepts_aspect(model, "1:1"), "{model}");
            assert!(!model_accepts_aspect(model, "4:3"), "{model}");
        }
        assert_eq!(
            klein_edit_capability(WINDOWS_KLEIN_MODEL),
            Some(windows::KLEIN_WINDOWS_EDIT_CAPABILITY)
        );
        assert_eq!(klein_edit_capability(WINDOWS_QWEN_MODEL), None);
        assert!(is_source_only_tool_model(WINDOWS_BIREFNET_MODEL));
        assert!(is_source_only_tool_model(WINDOWS_REALESRGAN_MODEL));
        assert!(!is_source_only_tool_model(WINDOWS_KLEIN_MODEL));
        assert!(is_source_only_tool_capability(
            windows::BIREFNET_WINDOWS_CAPABILITY
        ));
        assert!(!is_source_only_tool_capability(
            windows::KLEIN_WINDOWS_TEXT_CAPABILITY
        ));
        // Every registered Windows draft combination is structurally accepted with a 1:1 aspect;
        // a source-only tool draft may not carry a seed.
        let (_root, service) = fixture();
        for (model, capability) in [
            (WINDOWS_KLEIN_MODEL, windows::KLEIN_WINDOWS_TEXT_CAPABILITY),
            (WINDOWS_BIREFNET_MODEL, windows::BIREFNET_WINDOWS_CAPABILITY),
            (
                WINDOWS_REALESRGAN_MODEL,
                windows::REALESRGAN_WINDOWS_CAPABILITY,
            ),
            (
                WINDOWS_QWEN_MODEL,
                windows::QWEN_IMAGE_21_WINDOWS_CAPABILITY,
            ),
            (
                WINDOWS_QWEN_TURBO_MODEL,
                windows::QWEN_IMAGE_21_TURBO_WINDOWS_CAPABILITY,
            ),
            (
                WINDOWS_MOSS_MODEL,
                windows::MOSS_IMAGE_SFT_WINDOWS_CAPABILITY,
            ),
        ] {
            let params = json!({
                "modelId": model,
                "aspect": "1:1",
                "count": 1,
                "seed": "",
                "capabilityId": capability,
                "sourceAssetId": null
            });
            service
                .validate_registered_draft_params(&params)
                .unwrap_or_else(|error| panic!("{model} draft rejected: {error}"));
        }
        let tool_with_seed = json!({
            "modelId": WINDOWS_BIREFNET_MODEL,
            "aspect": "1:1",
            "count": 1,
            "seed": "42",
            "capabilityId": windows::BIREFNET_WINDOWS_CAPABILITY,
            "sourceAssetId": null
        });
        assert!(
            service
                .validate_registered_draft_params(&tool_with_seed)
                .unwrap_err()
                .contains("不接受种子")
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_runtime_lock_matrix_is_pinned_for_settings_rows() {
        use studio_core::generation as windows;
        // v1 runtime: Z-Image stays on its unchanged first-round runtime.
        assert_eq!(
            runtime_lock_id(WINDOWS_ZIMAGE_INSTALL_ID),
            Some(windows::Z_IMAGE_WINDOWS_RUNTIME)
        );
        // ncnn Vulkan: Real-ESRGAN has its own native runtime.
        assert_eq!(
            runtime_lock_id(WINDOWS_REALESRGAN_MODEL),
            Some(windows::REALESRGAN_WINDOWS_RUNTIME)
        );
        // Shared v2 runtime: Klein, BiRefNet, Qwen base/Turbo and MOSS.
        for model in [
            WINDOWS_KLEIN_MODEL,
            WINDOWS_BIREFNET_MODEL,
            WINDOWS_QWEN_MODEL,
            WINDOWS_QWEN_TURBO_MODEL,
            WINDOWS_MOSS_MODEL,
        ] {
            assert_eq!(
                runtime_lock_id(model),
                Some(windows::WINDOWS_V2_RUNTIME),
                "{model}"
            );
        }
        assert_eq!(runtime_lock_id("unknown-install-id"), None);
    }

    /// Save a draft that is structured for the given model/capability so a send can reach the
    /// hardware gate instead of failing draft validation.
    #[cfg(windows)]
    fn save_gate_draft(
        service: &StudioService,
        model_id: &str,
        capability_id: &str,
        prompt: &str,
    ) -> DraftRecord {
        service
            .save_draft(
                DraftInput {
                    id: 1,
                    name: "硬件门槛".into(),
                    prompt: prompt.into(),
                    params: json!({
                        "modelId": model_id,
                        "aspect": "1:1",
                        "count": 1,
                        "seed": "",
                        "capabilityId": capability_id,
                        "sourceAssetId": null
                    }),
                    collapsed: false,
                    prompt_source_id: None,
                },
                1,
            )
            .unwrap()
    }

    #[test]
    fn compact_tier_renders_768_square_and_rejects_other_aspects() {
        assert_eq!(compact_aspect_size("1:1", true), Ok((768, 768)));
        assert_eq!(compact_aspect_size("1:1", false), Ok((1024, 1024)));
        assert_eq!(compact_aspect_size("4:3", false), Ok((1024, 768)));
        assert_eq!(compact_aspect_size("3:4", false), Ok((768, 1024)));
        let error = compact_aspect_size("4:3", true).unwrap_err();
        assert!(error.contains("紧凑档"), "{error}");
        assert!(compact_aspect_size("16:9", false).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn install_is_rejected_below_the_hardware_threshold_before_starting() {
        let (_root, mut service) = fixture();
        service.device_facts = DeviceFacts {
            vram_gb: Some(8),
            ram_gb: 16,
            compute_capability: Some((7, 5)),
        };
        for model in [WINDOWS_KLEIN_MODEL, WINDOWS_MOSS_MODEL] {
            let error = service.install(model).unwrap_err();
            assert!(error.contains("当前设备不满足硬件门槛"), "{model}: {error}");
            // At 8 GB the compact tier's VRAM requirement is met; the 16 GB RAM is what blocks it.
            assert!(
                error.contains("内存 ≥ 32 GB（当前 16 GB）"),
                "{model}: {error}"
            );
            assert!(error.contains("安装与生成入口已禁用"), "{model}: {error}");
            // Rejection happened before `Installations::start`: the profile is still not started.
            let phase = service.model_view_for(model)["phase"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            assert!(
                matches!(phase.as_str(), "not_installed" | "failed" | "unavailable"),
                "{model} started installing: {phase}"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn conversation_send_is_rejected_below_the_hardware_threshold_without_a_turn() {
        let (_root, mut service) = fixture();
        service.device_facts = DeviceFacts {
            vram_gb: Some(8),
            ram_gb: 16,
            compute_capability: Some((7, 5)),
        };
        let saved = save_gate_draft(
            &service,
            WINDOWS_KLEIN_MODEL,
            studio_core::generation::KLEIN_WINDOWS_TEXT_CAPABILITY,
            "硬件门槛拒绝",
        );
        let conversation = service.snapshot().unwrap()["activeConversationId"]
            .as_str()
            .unwrap()
            .to_owned();
        let service = Arc::new(service);
        let error = service
            .conversation_send(ConversationSendInput {
                conversation_id: conversation,
                draft_id: 1,
                expected_version: saved.version,
                prompt: "硬件门槛拒绝".into(),
                model_id: WINDOWS_KLEIN_MODEL.into(),
                capability_id: Some(studio_core::generation::KLEIN_WINDOWS_TEXT_CAPABILITY.into()),
                source_asset_id: None,
                aspect: "1:1".into(),
                count: 1,
                seed: String::new(),
                submission_key: "hardware-gate-send-rejected".into(),
            })
            .unwrap_err();
        assert!(error.contains("当前设备不满足硬件门槛"), "{error}");
        assert!(error.contains("安装与生成入口已禁用"), "{error}");
        // No Turn/Attempt/Job exists and the stored draft was not consumed.
        let snapshot = service.snapshot().unwrap();
        assert_eq!(snapshot["turns"].as_array().unwrap().len(), 0);
        assert_eq!(snapshot["jobCounts"], json!([0, 0, 0]));
        assert_eq!(snapshot["drafts"][0]["prompt"], "硬件门槛拒绝");
    }

    #[cfg(windows)]
    #[test]
    fn unprovable_device_facts_fail_closed_at_install_and_send() {
        let (_root, mut service) = fixture();
        service.device_facts = DeviceFacts {
            vram_gb: None,
            ram_gb: 0,
            compute_capability: None,
        };
        let install_error = service.install(WINDOWS_QWEN_MODEL).unwrap_err();
        assert!(
            install_error.contains("无法确认硬件门槛"),
            "{install_error}"
        );
        assert!(install_error.contains("nvidia-smi"), "{install_error}");
        assert!(
            install_error.contains("安装与生成入口已禁用"),
            "{install_error}"
        );
        let saved = save_gate_draft(
            &service,
            WINDOWS_QWEN_MODEL,
            studio_core::generation::QWEN_IMAGE_21_WINDOWS_CAPABILITY,
            "无法确认设备",
        );
        let conversation = service.snapshot().unwrap()["activeConversationId"]
            .as_str()
            .unwrap()
            .to_owned();
        let service = Arc::new(service);
        let send_error = service
            .conversation_send(ConversationSendInput {
                conversation_id: conversation,
                draft_id: 1,
                expected_version: saved.version,
                prompt: "无法确认设备".into(),
                model_id: WINDOWS_QWEN_MODEL.into(),
                capability_id: Some(
                    studio_core::generation::QWEN_IMAGE_21_WINDOWS_CAPABILITY.into(),
                ),
                source_asset_id: None,
                aspect: "1:1".into(),
                count: 1,
                seed: String::new(),
                submission_key: "hardware-gate-unknown-send".into(),
            })
            .unwrap_err();
        assert!(send_error.contains("无法确认硬件门槛"), "{send_error}");
        assert_eq!(service.snapshot().unwrap()["jobCounts"], json!([0, 0, 0]));
    }

    #[cfg(windows)]
    #[test]
    fn exactly_registered_hardware_thresholds_are_allowed() {
        let catalog = ModelCatalog::load().unwrap();
        let at_threshold = DeviceFacts {
            vram_gb: Some(12),
            ram_gb: 32,
            compute_capability: Some((8, 6)),
        };
        for model in [
            WINDOWS_ZIMAGE_INSTALL_ID,
            WINDOWS_KLEIN_MODEL,
            WINDOWS_BIREFNET_MODEL,
            WINDOWS_REALESRGAN_MODEL,
            WINDOWS_QWEN_MODEL,
            WINDOWS_QWEN_TURBO_MODEL,
            WINDOWS_MOSS_MODEL,
        ] {
            enforce_hardware_gate(&catalog, model, &at_threshold)
                .unwrap_or_else(|error| panic!("{model} must be allowed: {error}"));
        }
        // Below the standard threshold the compact tier takes over instead of blocking.
        let low_vram = DeviceFacts {
            vram_gb: Some(11),
            ram_gb: 32,
            compute_capability: Some((8, 6)),
        };
        enforce_hardware_gate(&catalog, WINDOWS_KLEIN_MODEL, &low_vram)
            .expect("11 GB VRAM must fall back to the 8 GB compact tier");
        // Below every tier the model is rejected with the compact requirement.
        let tiny_vram = DeviceFacts {
            vram_gb: Some(7),
            ram_gb: 32,
            compute_capability: Some((8, 6)),
        };
        let vram_error = enforce_hardware_gate(&catalog, WINDOWS_KLEIN_MODEL, &tiny_vram)
            .expect_err("7 GB VRAM must not satisfy the 8 GB compact threshold");
        assert!(
            vram_error.contains("显存 ≥ 8 GB（当前 7 GB）"),
            "{vram_error}"
        );
        let low_ram = DeviceFacts {
            vram_gb: Some(12),
            ram_gb: 31,
            compute_capability: Some((8, 6)),
        };
        let ram_error = enforce_hardware_gate(&catalog, WINDOWS_KLEIN_MODEL, &low_ram)
            .expect_err("31 GB RAM must not satisfy a 32 GB threshold");
        assert!(
            ram_error.contains("内存 ≥ 32 GB（当前 31 GB）"),
            "{ram_error}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn hardware_gate_allows_a_send_at_exactly_the_registered_threshold() {
        let (_root, mut service) = fixture();
        service.device_facts = DeviceFacts {
            vram_gb: Some(12),
            ram_gb: 32,
            compute_capability: Some((8, 6)),
        };
        // A parity model at exactly its registered threshold must pass both the host gate and the
        // library turn-input allowlist (the seven-model threshold matrix itself is covered by
        // `exactly_registered_hardware_thresholds_are_allowed`).
        let saved = save_gate_draft(
            &service,
            WINDOWS_KLEIN_MODEL,
            studio_core::generation::KLEIN_WINDOWS_TEXT_CAPABILITY,
            "恰好达到门槛",
        );
        let conversation = service.snapshot().unwrap()["activeConversationId"]
            .as_str()
            .unwrap()
            .to_owned();
        let service = Arc::new(service);
        let response = service
            .conversation_send(ConversationSendInput {
                conversation_id: conversation,
                draft_id: 1,
                expected_version: saved.version,
                prompt: "恰好达到门槛".into(),
                model_id: WINDOWS_KLEIN_MODEL.into(),
                capability_id: Some(studio_core::generation::KLEIN_WINDOWS_TEXT_CAPABILITY.into()),
                source_asset_id: None,
                aspect: "1:1".into(),
                count: 1,
                seed: String::new(),
                submission_key: "hardware-gate-send-allowed".into(),
            })
            .unwrap();
        assert!(
            matches!(
                response["turn"]["state"].as_str(),
                Some("Pending" | "Rejected")
            ),
            "{}",
            response["turn"]["state"]
        );
        // The preflight still settles (the fixture has no installed model), but the failure is the
        // install state, not the hardware gate: the send itself was accepted.
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let state = service.snapshot().unwrap()["turns"][0]["state"]
                .as_str()
                .unwrap()
                .to_owned();
            if state == "Rejected" {
                assert!(
                    !service.snapshot().unwrap()["turns"][0]["error"]
                        .as_str()
                        .unwrap_or_default()
                        .contains("硬件门槛"),
                    "the accepted send must not be rejected by the hardware gate"
                );
                break;
            }
            assert!(
                Instant::now() < deadline,
                "preflight did not settle: {state}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn hardware_gate_does_not_apply_to_ids_outside_the_catalog() {
        let catalog = ModelCatalog::load().unwrap();
        let unknown_facts = DeviceFacts {
            vram_gb: None,
            ram_gb: 0,
            compute_capability: None,
        };
        // Unknown ids are rejected by the install/turn path itself; the gate must not invent a
        // hardware threshold for them.
        assert!(enforce_hardware_gate(&catalog, "not-registered", &unknown_facts).is_ok());
        #[cfg(windows)]
        for macos_id in ["z-image-turbo-mflux-v1", "flux2-klein-4b-distilled"] {
            assert!(
                enforce_hardware_gate(&catalog, macos_id, &unknown_facts).is_ok(),
                "macOS id must not be gated by the Windows catalog: {macos_id}"
            );
        }
        // On macOS the catalog registers no hardware thresholds, so unprovable facts cannot block
        // any existing path (macOS behavior unchanged).
        #[cfg(not(windows))]
        for id in [
            "z-image-turbo-mflux-v1",
            "flux2-klein-4b-distilled",
            "birefnet-general-epoch-244",
            "realesrgan-x4plus",
            "qwen-image-2.1-mflux-v1",
            "qwen-image-2.1-turbo-mflux-v1",
            "moss-image-sft-mflux-v1",
        ] {
            assert!(
                enforce_hardware_gate(&catalog, id, &unknown_facts).is_ok(),
                "macOS model must keep its ungated path: {id}"
            );
        }
    }

    #[test]
    fn conversation_delete_through_the_service_returns_the_successor_and_replays_safely() {
        let (_root, service) = fixture();
        let before = service.snapshot().unwrap();
        let conversation_id = before["activeConversationId"].as_str().unwrap().to_string();
        let draft_id = before["activeId"].as_u64().unwrap();
        let version = before["drafts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|draft| draft["id"].as_u64() == Some(draft_id))
            .and_then(|draft| draft["version"].as_u64())
            .unwrap();
        let result = service
            .conversation_delete(&conversation_id, version)
            .unwrap();
        assert_eq!(result["deleted"], true);
        let successor = result["activeConversationId"].as_str().unwrap().to_string();
        assert_ne!(successor, conversation_id);
        let after = service.snapshot().unwrap();
        assert_eq!(after["conversations"].as_array().unwrap().len(), 1);
        assert_eq!(
            after["activeConversationId"].as_str().unwrap(),
            successor.as_str()
        );
        // replaying the old confirmation returns the existing state without a second replacement
        let replay = service
            .conversation_delete(&conversation_id, version)
            .unwrap();
        assert_eq!(replay["deleted"], false);
        assert_eq!(
            replay["activeConversationId"].as_str().unwrap(),
            successor.as_str()
        );
        assert_eq!(
            service.snapshot().unwrap()["conversations"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
}
