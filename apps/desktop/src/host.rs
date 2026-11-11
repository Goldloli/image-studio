#[cfg(windows)]
use crate::adapters::PlatformError;
use crate::{
    DIAGNOSTIC_MODES,
    adapters::{
        AppPaths,
        diagnostics::{DiagnosticConfig, DiagnosticController, DiagnosticSnapshot},
        library::{AssetLibraryQuery, TaskCenterQuery},
        preferences::{AppearancePreferences, CreationDefaults},
        prompt_library::PromptLibraryStore,
    },
    options::LaunchOptions,
    studio::{
        ConversationCreateInput, ConversationSendInput, DraftInput, StudioService, SubmitInput,
    },
};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{Emitter, Manager, State, webview::PageLoadEvent};

/// Only the host owns executable paths. IPC never accepts program or directory arguments.
pub struct HostState {
    paths: AppPaths,
    worker: PathBuf,
    session: Mutex<DiagnosticSession>,
    studio: Arc<StudioService>,
    prompt_library: Result<Arc<PromptLibraryStore>, String>,
}

struct DiagnosticSession {
    enabled: bool,
    mode: String,
    controller: Option<DiagnosticController>,
}

impl HostState {
    fn new(options: &LaunchOptions, paths: AppPaths, worker: PathBuf) -> Self {
        let supervisor = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("image-studio-desktop"))
            .with_file_name(if cfg!(windows) {
                "inference-supervisor.exe"
            } else {
                "inference-supervisor"
            });
        crate::adapters::logging::init(&paths.logs);
        // A GUI process otherwise dies silently: capture panics in the same log as the rest.
        std::panic::set_hook(Box::new(|info| {
            crate::adapters::logging::warn(
                "app.panic",
                &[("detail", info.to_string().replace('\n', " "))],
            );
        }));
        let studio = Arc::new(StudioService::new(paths.clone(), supervisor));
        // A send may strand a turn as `pending` while its submitting thread is blocked; the
        // watchdog bounds that wait so the composer can never sit on "receiving prompt" forever.
        let watchdog = Arc::downgrade(&studio);
        let _ = std::thread::Builder::new()
            .name("turn-watchdog".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    let Some(studio) = watchdog.upgrade() else {
                        break;
                    };
                    let reaped = studio.reap_stale_pending_turns(60_000);
                    if reaped > 0 {
                        crate::adapters::logging::warn(
                            "turn.watchdog.reaped",
                            &[("count", reaped.to_string())],
                        );
                    }
                }
            });
        let prompt_library = PromptLibraryStore::load(&paths).map(Arc::new);
        Self {
            paths,
            worker,
            session: Mutex::new(DiagnosticSession {
                enabled: options.diagnostics,
                mode: options.mode.clone(),
                controller: None,
            }),
            studio,
            prompt_library,
        }
    }
    fn session(&self) -> MutexGuard<'_, DiagnosticSession> {
        self.session
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
    fn enable(&self, development: bool) -> Result<bool, String> {
        let mut session = self.session();
        if !development && !session.enabled {
            return Err("发布版本需要在启动时显式指定 --diagnostics".into());
        }
        session.enabled = true;
        Ok(true)
    }
    fn start(&self, mode: &str) -> Result<DiagnosticState, String> {
        if !DIAGNOSTIC_MODES.contains(&mode) {
            return Err("未知通信诊断模式".into());
        }
        let mut session = self.session();
        require_diagnostics(&session)?;
        if session
            .controller
            .as_ref()
            .is_some_and(|controller| controller.snapshot().is_running)
        {
            return Err("通信诊断已经运行，请先停止或等待结束".into());
        }
        let mut config = DiagnosticConfig::new(self.worker.clone());
        config.mode = mode.to_owned();
        config.work_dir = Some(self.paths.work.clone());
        session.mode = mode.to_owned();
        // Conflated snapshots are polled by the frontend; no per-progress IPC/event queue.
        session.controller = Some(DiagnosticController::start(config, Arc::new(|| {})));
        Ok(diagnostic_state(&session))
    }
    fn stop(&self) -> Result<DiagnosticState, String> {
        let session = self.session();
        require_diagnostics(&session)?;
        if let Some(controller) = &session.controller {
            controller.stop();
        }
        Ok(diagnostic_state(&session))
    }
    fn stop_for_exit(&self) {
        self.studio.shutdown();
        if let Some(controller) = &self.session().controller {
            controller.stop();
        }
    }
    fn prompt_library(&self) -> Result<Arc<PromptLibraryStore>, String> {
        self.prompt_library.clone()
    }
}

fn require_diagnostics(session: &DiagnosticSession) -> Result<(), String> {
    if session.enabled {
        Ok(())
    } else {
        Err("请先显式启用开发通信诊断".into())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatus {
    inference_available: bool,
    status: String,
    supported_models: usize,
    diagnostics_enabled: bool,
    development_build: bool,
    version: &'static str,
    platform: &'static str,
}

#[tauri::command]
fn workspace_status(state: State<'_, HostState>) -> WorkspaceStatus {
    let model = state.studio.model_view();
    let ready = model["ready"].as_bool().unwrap_or(false);
    WorkspaceStatus {
        inference_available: ready,
        status: model["summary"].as_str().unwrap_or("模型尚未就绪").into(),
        supported_models: usize::from(ready),
        diagnostics_enabled: state.session().enabled,
        development_build: cfg!(debug_assertions),
        version: env!("CARGO_PKG_VERSION"),
        platform: std::env::consts::OS,
    }
}

async fn blocking<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|e| format!("后台任务失败：{e}"))?
}

#[tauri::command]
async fn studio_snapshot(state: State<'_, HostState>) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.snapshot()).await
}
#[tauri::command]
async fn model_catalog(state: State<'_, HostState>) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.model_catalog()).await
}
#[tauri::command]
async fn feature_catalog(state: State<'_, HostState>) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.feature_catalog()).await
}
#[tauri::command]
async fn prompt_library_snapshot(state: State<'_, HostState>) -> Result<serde_json::Value, String> {
    let library = state.prompt_library()?;
    blocking(move || Ok(library.snapshot())).await
}
#[tauri::command]
async fn prompt_library_favorite(
    state: State<'_, HostState>,
    case_id: String,
    favorite: bool,
) -> Result<serde_json::Value, String> {
    let library = state.prompt_library()?;
    blocking(move || library.set_favorite(&case_id, favorite)).await
}
#[tauri::command]
async fn prompt_library_image(
    state: State<'_, HostState>,
    case_id: String,
) -> Result<serde_json::Value, String> {
    let library = state.prompt_library()?;
    blocking(move || library.ensure_image(&case_id)).await
}
#[tauri::command]
async fn prompt_library_clear_cache(
    state: State<'_, HostState>,
) -> Result<serde_json::Value, String> {
    let library = state.prompt_library()?;
    blocking(move || library.clear_cache()).await
}
#[tauri::command]
async fn preferences_snapshot(state: State<'_, HostState>) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || Ok(studio.preferences_snapshot())).await
}
#[tauri::command]
async fn preferences_reload(state: State<'_, HostState>) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || Ok(studio.reload_preferences())).await
}
#[tauri::command]
async fn preferences_save(
    state: State<'_, HostState>,
    appearance: AppearancePreferences,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.save_appearance(appearance)).await
}
#[tauri::command]
async fn creation_defaults_save(
    state: State<'_, HostState>,
    defaults: CreationDefaults,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.save_creation_defaults(defaults)).await
}
#[tauri::command]
async fn settings_snapshot(state: State<'_, HostState>) -> Result<serde_json::Value, String> {
    let diagnostics_enabled = state.session().enabled;
    let studio = state.studio.clone();
    blocking(move || studio.settings_snapshot(diagnostics_enabled)).await
}
#[tauri::command]
async fn storage_open(state: State<'_, HostState>, location_id: String) -> Result<(), String> {
    let studio = state.studio.clone();
    blocking(move || studio.open_storage_location(&location_id)).await
}
#[tauri::command]
async fn model_install(
    state: State<'_, HostState>,
    install_id: String,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.install(&install_id)).await
}
#[tauri::command]
async fn model_cancel(
    state: State<'_, HostState>,
    install_id: String,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.cancel_install(&install_id)).await
}
#[tauri::command]
async fn draft_save(
    state: State<'_, HostState>,
    draft: DraftInput,
    expected_version: u64,
) -> Result<crate::adapters::library::DraftRecord, String> {
    let studio = state.studio.clone();
    blocking(move || studio.save_draft(draft, expected_version)).await
}
#[tauri::command]
async fn draft_create(
    state: State<'_, HostState>,
) -> Result<crate::adapters::library::DraftRecord, String> {
    let studio = state.studio.clone();
    blocking(move || studio.draft_create()).await
}
#[tauri::command]
async fn conversation_list(
    state: State<'_, HostState>,
    limit: u32,
    before_activity: Option<u64>,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.conversation_list(limit, before_activity)).await
}
#[tauri::command]
async fn conversation_create(
    state: State<'_, HostState>,
    input: Option<ConversationCreateInput>,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.conversation_create(input)).await
}
#[tauri::command]
async fn conversation_apply_model(
    state: State<'_, HostState>,
    conversation_id: String,
    model_id: String,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.apply_model_to_conversation(&conversation_id, &model_id)).await
}
#[tauri::command]
async fn conversation_select(
    state: State<'_, HostState>,
    conversation_id: String,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.conversation_select(&conversation_id)).await
}
#[tauri::command]
async fn conversation_rename(
    state: State<'_, HostState>,
    conversation_id: String,
    title: String,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.conversation_rename(&conversation_id, &title)).await
}
#[tauri::command]
async fn conversation_delete(
    state: State<'_, HostState>,
    conversation_id: String,
    expected_draft_version: u64,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.conversation_delete(&conversation_id, expected_draft_version)).await
}
#[tauri::command]
async fn conversation_history(
    state: State<'_, HostState>,
    conversation_id: String,
    before_sequence: Option<u64>,
    limit: u32,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.conversation_history(&conversation_id, before_sequence, limit)).await
}
#[tauri::command]
async fn conversation_turn_detail(
    state: State<'_, HostState>,
    turn_id: String,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.conversation_turn_detail(&turn_id)).await
}
#[tauri::command]
async fn conversation_send(
    state: State<'_, HostState>,
    input: ConversationSendInput,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.conversation_send(input)).await
}
#[tauri::command]
async fn conversation_cancel_turn(
    state: State<'_, HostState>,
    turn_id: String,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.conversation_cancel_turn(&turn_id)).await
}
#[tauri::command]
async fn workspace_select(
    state: State<'_, HostState>,
    active_id: u64,
    selected_asset_id: Option<String>,
) -> Result<(), String> {
    let studio = state.studio.clone();
    blocking(move || studio.select(active_id, selected_asset_id)).await
}
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn generation_submit(
    state: State<'_, HostState>,
    draft_id: u64,
    prompt: String,
    aspect: String,
    count: u32,
    seed: String,
    submission_key: String,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || {
        studio.submit(SubmitInput {
            draft_id,
            prompt,
            aspect,
            count,
            seed,
            submission_key,
        })
    })
    .await
}
#[tauri::command]
async fn generation_cancel(
    state: State<'_, HostState>,
    job_id: String,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.cancel(&job_id)).await
}
#[tauri::command]
async fn generation_retry(
    state: State<'_, HostState>,
    job_id: String,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.retry(&job_id)).await
}
#[tauri::command]
async fn task_list(
    state: State<'_, HostState>,
    limit: u32,
    offset: u32,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.task_list(limit, offset)).await
}
#[tauri::command]
async fn asset_library_query(
    state: State<'_, HostState>,
    query: AssetLibraryQuery,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.asset_library_query(query)).await
}
#[tauri::command]
async fn asset_library_item(
    state: State<'_, HostState>,
    asset_id: String,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.asset_library_item(&asset_id)).await
}
#[tauri::command]
async fn asset_set_favorite(
    state: State<'_, HostState>,
    asset_id: String,
    favorite: bool,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.asset_set_favorite(&asset_id, favorite)).await
}
#[tauri::command]
async fn asset_delete(
    state: State<'_, HostState>,
    asset_id: String,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.asset_delete(&asset_id)).await
}
#[tauri::command]
async fn task_center_query(
    state: State<'_, HostState>,
    query: TaskCenterQuery,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.task_center_query(query)).await
}
#[tauri::command]
async fn task_center_detail(
    state: State<'_, HostState>,
    job_id: String,
) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.task_center_detail(&job_id)).await
}
#[tauri::command]
async fn asset_export(
    state: State<'_, HostState>,
    asset_id: String,
) -> Result<serde_json::Value, String> {
    studio_core::domain::AssetId::new(&asset_id).map_err(|e| e.to_string())?;
    let target = rfd::AsyncFileDialog::new()
        .set_title("导出 PNG 图片")
        .add_filter("PNG 图片", &["png"])
        .set_file_name("Image Studio.png")
        .save_file()
        .await;
    let Some(target) = target else {
        return Ok(serde_json::json!({"cancelled":true}));
    };
    let studio = state.studio.clone();
    let path = target.path().to_path_buf();
    // A newly chosen name receives atomic no-replace semantics. An existing
    // target is never replaced without an explicit user confirmation; declining
    // returns the same shape as cancelling the save dialog and leaves the old
    // file untouched.
    let overwrite_confirmed = match std::fs::symlink_metadata(&path) {
        Ok(_) => match confirm_export_overwrite().await {
            ExportOverwriteChoice::Overwrite => true,
            ExportOverwriteChoice::KeepExisting => {
                return Ok(serde_json::json!({"cancelled":true}));
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.to_string()),
    };
    blocking(move || studio.export(&asset_id, &path, overwrite_confirmed)).await?;
    Ok(serde_json::json!({"cancelled":false}))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportOverwriteChoice {
    Overwrite,
    KeepExisting,
}

fn resolve_export_overwrite(
    exists: bool,
    answer: rfd::MessageDialogResult,
) -> ExportOverwriteChoice {
    if !exists {
        return ExportOverwriteChoice::Overwrite;
    }
    match answer {
        rfd::MessageDialogResult::Yes | rfd::MessageDialogResult::Ok => {
            ExportOverwriteChoice::Overwrite
        }
        _ => ExportOverwriteChoice::KeepExisting,
    }
}

async fn confirm_export_overwrite() -> ExportOverwriteChoice {
    let answer = rfd::AsyncMessageDialog::new()
        .set_title("覆盖导出文件")
        .set_description("目标文件已存在。覆盖后旧文件会被替换，是否继续？")
        .set_level(rfd::MessageLevel::Warning)
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
        .await;
    resolve_export_overwrite(true, answer)
}
#[tauri::command]
async fn source_import(state: State<'_, HostState>) -> Result<serde_json::Value, String> {
    let selected = rfd::AsyncFileDialog::new()
        .set_title("选择源图")
        .add_filter("PNG 或 JPEG", &["png", "jpg", "jpeg"])
        .pick_file()
        .await;
    let Some(selected) = selected else {
        return Ok(serde_json::json!({"cancelled":true}));
    };
    let studio = state.studio.clone();
    let path = selected.path().to_path_buf();
    blocking(move || studio.import_source(&path)).await
}
#[tauri::command]
async fn source_list(state: State<'_, HostState>) -> Result<serde_json::Value, String> {
    let studio = state.studio.clone();
    blocking(move || studio.imported_sources()).await
}
#[tauri::command]
fn app_close(app: tauri::AppHandle, state: State<'_, HostState>) {
    state.studio.allow_close();
    state.stop_for_exit();
    app.exit(0);
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticState {
    enabled: bool,
    mode: String,
    modes: &'static [&'static str],
    snapshot: Option<DiagnosticView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticView {
    phase: String,
    summary: String,
    probe_passed: bool,
    is_running: bool,
    pid: Option<u32>,
    child_pid: Option<u32>,
    error_code: Option<String>,
    logs: String,
    progress_events: u64,
}

impl From<DiagnosticSnapshot> for DiagnosticView {
    fn from(value: DiagnosticSnapshot) -> Self {
        Self {
            phase: format!("{:?}", value.phase),
            summary: value.summary,
            probe_passed: value.probe_passed,
            is_running: value.is_running,
            pid: value.pid,
            child_pid: value.child_pid,
            error_code: value.error_code,
            logs: value.logs,
            progress_events: value.progress_events,
        }
    }
}

fn diagnostic_state(session: &DiagnosticSession) -> DiagnosticState {
    DiagnosticState {
        enabled: session.enabled,
        mode: session.mode.clone(),
        modes: DIAGNOSTIC_MODES,
        snapshot: session
            .controller
            .as_ref()
            .map(|controller| controller.snapshot().into()),
    }
}

#[tauri::command]
fn diagnostic_enable(state: State<'_, HostState>) -> Result<bool, String> {
    state.enable(cfg!(debug_assertions))
}

#[tauri::command]
async fn diagnostic_start(
    state: State<'_, HostState>,
    mode: String,
) -> Result<DiagnosticState, String> {
    state.start(&mode)
}

#[tauri::command]
fn diagnostic_stop(state: State<'_, HostState>) -> Result<DiagnosticState, String> {
    state.stop()
}

#[tauri::command]
fn diagnostic_snapshot(state: State<'_, HostState>) -> DiagnosticState {
    diagnostic_state(&state.session())
}

fn validate_viewport(width: u32, height: u32, scale: Option<u8>) -> Result<(), String> {
    if ![(960, 640), (1280, 800)].contains(&(width, height)) {
        return Err("开发窗口尺寸仅支持 960x640 或 1280x800".into());
    }
    if scale.is_some_and(|value| value != 1 && value != 2) {
        return Err("开发比例请求仅支持 1 或 2；原生 DPR 由 WebView 决定".into());
    }
    Ok(())
}

#[tauri::command]
fn diagnostic_viewport(
    window: tauri::WebviewWindow,
    state: State<'_, HostState>,
    width: u32,
    height: u32,
    scale: Option<u8>,
) -> Result<(), String> {
    require_diagnostics(&state.session())?;
    validate_viewport(width, height, scale)?;
    if window.label() != "main" {
        return Err("窗口不可用".into());
    }
    window
        .set_size(tauri::LogicalSize::new(width, height))
        .map_err(|error| error.to_string())
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let options = LaunchOptions::parse_from(std::env::args_os().skip(1))
        .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message))?;
    if options.help {
        println!("{}", LaunchOptions::HELP);
        return Ok(());
    }
    let paths = match AppPaths::resolve(options.data_dir.as_deref()) {
        Ok(paths) => paths,
        Err(error) => {
            #[cfg(windows)]
            if let PlatformError::DataPathTooLong { ref path, .. } = error {
                // The rejected root is the data path's parent (both for --data-dir
                // and the %LOCALAPPDATA% default); write the report inside it.
                let logs = path
                    .parent()
                    .map(|root| root.join("logs"))
                    .unwrap_or_else(|| PathBuf::from("logs"));
                let _ = std::fs::create_dir_all(&logs);
                crate::startup::fail_startup(&logs, &error.to_string());
            }
            return Err(error.into());
        }
    };
    crate::startup::install_panic_hook(paths.logs.clone());
    if let Err(error) = crate::startup::configure_webview_profile(&paths.cache) {
        crate::startup::fail_startup(
            &paths.logs,
            &format!(
                "无法创建{}：{error}",
                if cfg!(windows) {
                    " WebView2 用户数据目录"
                } else {
                    " WebView 缓存目录"
                }
            ),
        );
    }
    let worker = options.worker.clone().unwrap_or_else(|| {
        std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("image-studio-desktop"))
            .with_file_name(if cfg!(windows) {
                "diagnostic-worker.exe"
            } else {
                "diagnostic-worker"
            })
    });
    let host = HostState::new(&options, paths, worker);
    let initial_size = options.diagnostic_size;
    let diagnostics = options.diagnostics;
    let requested_scale = options.diagnostic_scale;
    let ready = Arc::new(AtomicBool::new(false));
    let app = tauri::Builder::default()
        .manage(host)
        .register_asynchronous_uri_scheme_protocol("studio-prompt",|context,request,responder|{
            let library=context.app_handle().state::<HostState>().prompt_library();
            let source_id=request.uri().path().trim_start_matches('/').parse::<u64>();
            let allowed=request.method()==tauri::http::Method::GET
                && request.uri().host()==Some("localhost")
                && source_id.is_ok();
            tauri::async_runtime::spawn_blocking(move||{
                let response=if allowed {
                    let case_id=format!("awesome-gpt-image-2:case:{}",source_id.unwrap());
                    match library.and_then(|store|store.read_cached_image(&case_id)) {
                        Ok((bytes,mime))=>tauri::http::Response::builder()
                            .status(200)
                            .header("Content-Type",mime)
                            .header("Cache-Control","private, max-age=3600")
                            .header("Access-Control-Allow-Origin","*")
                            .body(bytes),
                        Err(_)=>tauri::http::Response::builder()
                            .status(404)
                            .header("Content-Type","text/plain")
                            .body(b"Prompt image unavailable".to_vec()),
                    }
                } else {
                    tauri::http::Response::builder()
                        .status(400)
                        .body(b"Invalid prompt image request".to_vec())
                };
                if let Ok(response)=response {
                    responder.respond(response);
                }
            });
        })
        .register_asynchronous_uri_scheme_protocol("studio-asset",|context,request,responder|{
            let studio=context.app_handle().state::<HostState>().studio.clone();
            let id=request.uri().path().trim_start_matches('/').to_owned();
            let allowed=request.method()==tauri::http::Method::GET&&request.uri().host()==Some("localhost")&&studio_core::domain::AssetId::new(&id).is_ok();
            tauri::async_runtime::spawn_blocking(move||{
                let response=if allowed {match studio.read_asset(&id){Ok(bytes)=>tauri::http::Response::builder().status(200).header("Content-Type","image/png").header("Cache-Control","no-store").header("Access-Control-Allow-Origin","*").body(bytes),Err(_)=>tauri::http::Response::builder().status(404).header("Content-Type","text/plain").body(b"Asset unavailable".to_vec())}}
                    else{tauri::http::Response::builder().status(400).body(b"Invalid asset request".to_vec())};
                if let Ok(response)=response{responder.respond(response);}
            });
        })
        .on_window_event(|window,event|{
            if let tauri::WindowEvent::CloseRequested{api,..}=event
                && window.label()=="main" && !window.state::<HostState>().studio.closing.load(Ordering::Acquire){
                    api.prevent_close();let _=window.emit("studio-close-requested",());
                }
        })
        .setup(move |app| {
            if let Some(window) = app.get_webview_window("main")
                && let Some([width, height]) = initial_size {
                window.set_size(tauri::LogicalSize::new(width, height))?;
            }
            Ok(())
        })
        .on_page_load(move |webview, payload| {
            if diagnostics && webview.label() == "main" && payload.event() == PageLoadEvent::Finished
                && !ready.swap(true, Ordering::AcqRel) {
                eprintln!("DIAGNOSTIC_DESKTOP_READY tauri=2 requested_scale={requested_scale:?} native_dpr=webview window={initial_size:?}; image inference unavailable");
            }
        })
        .invoke_handler(tauri::generate_handler![workspace_status, diagnostic_enable, diagnostic_start, diagnostic_stop, diagnostic_snapshot, diagnostic_viewport, studio_snapshot, model_catalog, feature_catalog, prompt_library_snapshot, prompt_library_favorite, prompt_library_image, prompt_library_clear_cache, preferences_snapshot, preferences_reload, preferences_save, creation_defaults_save, settings_snapshot, storage_open, model_install, model_cancel, draft_save, draft_create, conversation_list, conversation_create, conversation_apply_model, conversation_select, conversation_rename, conversation_delete, conversation_history, conversation_turn_detail, conversation_send, conversation_cancel_turn, workspace_select, generation_submit, generation_cancel, generation_retry, task_list, asset_library_query, asset_library_item, asset_set_favorite, asset_delete, task_center_query, task_center_detail, asset_export, source_import, source_list, app_close])
        .build(tauri::generate_context!())?;
    app.run(|app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = &event
            && !app
                .state::<HostState>()
                .studio
                .closing
                .load(Ordering::Acquire)
        {
            api.prevent_exit();
            let _ = app.emit_to("main", "studio-close-requested", ());
            return;
        }
        if matches!(
            event,
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
        ) {
            app.state::<HostState>().stop_for_exit();
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(enabled: bool) -> (HostState, tempfile::TempDir) {
        let root = tempfile::tempdir().unwrap();
        let options = LaunchOptions {
            diagnostics: enabled,
            ..Default::default()
        };
        let host = HostState::new(
            &options,
            AppPaths::resolve(Some(root.path())).unwrap(),
            root.path().join("not-a-worker"),
        );
        (host, root)
    }

    #[test]
    fn startup_does_not_start_worker_or_allow_unrequested_diagnostics() {
        let (host, _root) = fixture(false);
        assert!(host.session().controller.is_none());
        assert!(host.start("normal").is_err());
        assert!(host.stop().is_err());
        assert!(host.enable(false).is_err());
        assert!(host.session().controller.is_none());
        assert!(host.enable(true).unwrap());
        assert!(host.session().enabled);
    }

    #[test]
    fn export_overwrite_requires_an_explicit_confirmation() {
        use rfd::MessageDialogResult;
        assert_eq!(
            resolve_export_overwrite(false, MessageDialogResult::No),
            ExportOverwriteChoice::Overwrite
        );
        assert_eq!(
            resolve_export_overwrite(true, MessageDialogResult::Yes),
            ExportOverwriteChoice::Overwrite
        );
        assert_eq!(
            resolve_export_overwrite(true, MessageDialogResult::Ok),
            ExportOverwriteChoice::Overwrite
        );
        assert_eq!(
            resolve_export_overwrite(true, MessageDialogResult::No),
            ExportOverwriteChoice::KeepExisting
        );
        assert_eq!(
            resolve_export_overwrite(true, MessageDialogResult::Cancel),
            ExportOverwriteChoice::KeepExisting
        );
        assert_eq!(
            resolve_export_overwrite(true, MessageDialogResult::Custom("no".into())),
            ExportOverwriteChoice::KeepExisting
        );
    }
}
