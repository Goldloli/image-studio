//! Business facade for the local webview. Blocking work is called through spawn_blocking.
use crate::adapters::{
    AppPaths,
    catalog::ModelCatalog,
    feature_catalog::FeatureCatalog,
    generation::GenerationController,
    installation::{
        BIREFNET_INSTALL_ID, INSTALL_ID, InstallationSnapshot, Installations, KLEIN_INSTALL_ID,
        REALESRGAN_INSTALL_ID,
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
    generation::{GenerationInput, freeze_z_image},
    recipes::{
        BiRefNetInput, KleinInput, RealEsrganInput, SourceAsset, SourceFacts, freeze_birefnet,
        freeze_klein, freeze_realesrgan,
    },
};
use uuid::Uuid;

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
}

fn default_capability_id() -> String {
    studio_core::generation::Z_IMAGE_CAPABILITY.into()
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
    pub fn model_view(&self) -> Value {
        self.model_view_for(INSTALL_ID)
    }
    pub fn model_views(&self) -> Vec<Value> {
        [
            INSTALL_ID,
            KLEIN_INSTALL_ID,
            BIREFNET_INSTALL_ID,
            REALESRGAN_INSTALL_ID,
        ]
        .into_iter()
        .map(|id| self.model_view_for(id))
        .collect()
    }
    fn model_view_for(&self, install_id: &str) -> Value {
        let name = if install_id == KLEIN_INSTALL_ID {
            "FLUX.2 Klein 4B distilled"
        } else if install_id == BIREFNET_INSTALL_ID {
            "BiRefNet general epoch_244"
        } else if install_id == REALESRGAN_INSTALL_ID {
            "Real-ESRGAN x4plus"
        } else {
            "Z-Image Turbo"
        };
        let fallback_model_bytes = if install_id == KLEIN_INSTALL_ID {
            15_980_131_745_u64
        } else if install_id == BIREFNET_INSTALL_ID {
            972_666_916_u64
        } else if install_id == REALESRGAN_INSTALL_ID {
            33_540_549_u64
        } else {
            32_848_304_893_u64
        };
        let Some(s) = self
            .installation
            .as_ref()
            .and_then(|installation| installation.snapshot(install_id))
        else {
            return json!({"id":install_id,"name":name,"phase":"failed","summary":"安装清单不可用","ready":false,"runtimeReady":false,"modelReady":false,"deviceVerified":false,"receivedBytes":0,"totalBytes":0,"modelBytes":fallback_model_bytes,"runtimeBytes":0,"requiredBytes":0,"error":self.startup_error,"busy":false});
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
        json!({"id":s.install_id,"name":name,"phase":s.phase,"summary":summary,"ready":s.phase=="ready"&&s.runtime_ready&&s.model_ready&&s.device_verified,"runtimeReady":s.runtime_ready,"modelReady":s.model_ready,"deviceVerified":s.device_verified,"receivedBytes":s.received_bytes,"totalBytes":s.total_bytes,"modelBytes":s.model_bytes,"runtimeBytes":s.runtime_bytes,"requiredBytes":s.required_bytes,"error":s.error,"busy":busy})
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
        catalog.query(&installations, installation_error.as_deref())
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
        Ok(json!({
            "preferences": preferences,
            "catalog": catalog,
            "storage": storage,
            "environment": {
                "platform": std::env::consts::OS,
                "architecture": std::env::consts::ARCH,
                "runtimeName": "MFLUX / MLX + ONNX Runtime CPU + ncnn Vulkan",
                "runtimeVersion": "0.24.0 / 1.31.1 / 0.2.0",
                "pythonVersion": "3.13.12",
                "diagnosticsEnabled": diagnostics_enabled,
                "runtime": runtime
                ,"runtimes": installations
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
                "modelSources": [
                    "Z-Image Turbo · Tongyi-MAI/Z-Image-Turbo · Apache-2.0",
                    "FLUX.2 Klein 4B distilled · black-forest-labs/FLUX.2-klein-4B · Apache-2.0",
                    "BiRefNet general epoch_244 · ZhengPeng7/BiRefNet v1 · MIT",
                    "Real-ESRGAN x4plus · official ncnn package v0.2.5.0 · BSD-3-Clause"
                ],
                "capabilityScope": "Z-Image Turbo、FLUX.2 Klein 4B、BiRefNet epoch_244 与 Real-ESRGAN x4plus 已接入本地安装与执行链路。"
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
        if !catalog.contains(&params.model_id) {
            return Err("所选模型未在权威目录中登记".into());
        }
        let supported_combo = match (
            params.model_id.as_str(),
            params.capability_id.as_str(),
            params.source_asset_id.is_some(),
        ) {
            (DEFAULT_MODEL_ID, studio_core::generation::Z_IMAGE_CAPABILITY, false) => {
                matches!(params.aspect.as_str(), "1:1" | "4:3" | "3:4")
            }
            (
                studio_core::recipes::KLEIN_MODEL,
                studio_core::recipes::KLEIN_TEXT_CAPABILITY,
                false,
            )
            | (
                studio_core::recipes::KLEIN_MODEL,
                studio_core::recipes::KLEIN_EDIT_CAPABILITY,
                true,
            ) => params.aspect == "1:1",
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
            _ => false,
        };
        if !supported_combo {
            return Err("模型、能力、源图与画幅组合不受支持".into());
        }
        if matches!(
            params.model_id.as_str(),
            studio_core::recipes::BIREFNET_MODEL | studio_core::recipes::REALESRGAN_MODEL
        ) && !params.seed.is_empty()
        {
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
        };
        if !catalog.supports(&params.model_id, "text_to_image") {
            warnings.push("已保存的默认模型不在当前目录中，已回退为 Z-Image Turbo".into());
            params.model_id = DEFAULT_MODEL_ID.into();
        }
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
        if params.model_id == KLEIN_INSTALL_ID {
            params.aspect = "1:1".into();
            params.capability_id = studio_core::recipes::KLEIN_TEXT_CAPABILITY.into();
            params.source_asset_id = None;
        } else {
            params.capability_id = studio_core::generation::Z_IMAGE_CAPABILITY.into();
            params.source_asset_id = None;
        }
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
                        "error": asset.availability_error
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
            if input.model_id == KLEIN_INSTALL_ID && source_asset_id.is_some() {
                studio_core::recipes::KLEIN_EDIT_CAPABILITY.into()
            } else if input.model_id == KLEIN_INSTALL_ID {
                studio_core::recipes::KLEIN_TEXT_CAPABILITY.into()
            } else if input.model_id == BIREFNET_INSTALL_ID {
                studio_core::recipes::BIREFNET_CAPABILITY.into()
            } else if input.model_id == REALESRGAN_INSTALL_ID {
                studio_core::recipes::REALESRGAN_CAPABILITY.into()
            } else {
                studio_core::generation::Z_IMAGE_CAPABILITY.into()
            }
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
        if let Err(error) = self.prepare_turn(&turn) {
            let _ = self.with_library(|library| library.reject_turn(&turn.id, &error));
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
        let (width, height) = match turn.aspect.as_str() {
            "1:1" => (1024, 1024),
            "4:3" => (1024, 768),
            "3:4" => (768, 1024),
            _ => return Err("当前模型不支持此画幅，请明确修改参数".into()),
        };
        let seed = if turn.seed.is_empty() {
            None
        } else {
            Some(
                turn.seed
                    .parse::<u32>()
                    .map_err(|_| "种子须为 0–4294967295".to_string())?,
            )
        };
        let installation = self.installation.as_ref().ok_or("安装器不可用")?;
        let ready = installation.ready_paths(&turn.model_id)?;
        let environment = installation.plan_environment(&turn.model_id)?;
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
        let _schedule = self
            .scheduling
            .lock()
            .map_err(|_| "提交锁异常".to_string())?;
        let binding =
            self.with_library(|library| library.bind_turn_job(&turn.id, request, plan))?;
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
        if let Err(error) = controller.start(binding.job.clone(), ready) {
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
        let ready = installation.ready_paths(previous.request.model_variant_id.as_str())?;
        let environment =
            installation.plan_environment(previous.request.model_variant_id.as_str())?;
        let scheduler_matches = studio_core::recipes::Recipe::from_request(&previous.request)
            .is_ok_and(|recipe| {
                matches!(
                    recipe,
                    studio_core::recipes::Recipe::BiRefNet
                        | studio_core::recipes::Recipe::RealEsrgan
                ) || previous.plan.effective_parameters.get("scheduler")
                    == Some(&json!(environment.scheduler))
            });
        if previous.plan.components != environment.components
            || previous.plan.runtime != environment.runtime
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
        "maskAssetId": mask_asset_id,
        "maskAssetAvailable": mask_record.as_ref().is_some_and(|asset| asset.available),
        "maskAssetError": mask_record.and_then(|asset| asset.availability_error),
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
        let service = StudioService::new(
            AppPaths::resolve(Some(root.path())).unwrap(),
            root.path().join("not-installed-supervisor"),
        );
        (root, service)
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
                    params: json!({"modelId":"z-image-turbo-mflux-v1","aspect":"4:3","count":1,"seed":"42"}),
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
        let service = Arc::new(StudioService::new(
            AppPaths::resolve(Some(root.path())).unwrap(),
            root.path().join("not-installed-supervisor"),
        ));
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
    fn removed_saved_model_falls_back_only_that_field_and_reports_it() {
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
        assert_eq!(
            created["draft"]["params"]["modelId"],
            "z-image-turbo-mflux-v1"
        );
        assert_eq!(created["draft"]["params"]["aspect"], "4:3");
        assert_eq!(created["draft"]["params"]["seed"], "9");
        assert!(
            created["defaultsWarnings"]
                .as_array()
                .is_some_and(|warnings| !warnings.is_empty())
        );
    }

    #[test]
    fn catalog_query_exposes_independent_model_install_actions() {
        let (_root, service) = fixture();
        let catalog = service.model_catalog().unwrap();
        assert_eq!(catalog["counts"]["total"], 4);
        let planned = catalog["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == "flux2-klein-4b-distilled")
            .unwrap();
        assert_eq!(planned["productStatus"], "implemented");
        assert_eq!(planned["canInstall"], true);
        assert_eq!(planned["executable"], false);
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
}
