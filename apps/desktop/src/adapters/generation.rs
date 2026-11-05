//! One background generation exchange. Durable state changes always pass through Library.
//! A native candidate is still provisional until process exit, lease release and SQLite commit.
use super::{
    AppPaths,
    installation::{ReadyPaths, WINDOWS_ZIMAGE_WORKER_MODULE},
    library::{
        CancelOutcome, CandidateDescriptor, Completion, JobRecord, Library, LibraryError,
        RoleCandidate,
    },
};
use fs2::FileExt;
use inference_supervisor::{SupervisorConfig, Timeouts};
use serde::Serialize;
use std::{
    fs::{File, OpenOptions},
    io::{BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Child, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use studio_core::{
    domain::JobId,
    generation::{
        AttemptState, BIREFNET_WINDOWS_CAPABILITY, KLEIN_WINDOWS_EDIT_CAPABILITY,
        KLEIN_WINDOWS_TEXT_CAPABILITY, MOSS_IMAGE_SFT_WINDOWS_CAPABILITY,
        QWEN_IMAGE_21_TURBO_WINDOWS_CAPABILITY, QWEN_IMAGE_21_WINDOWS_CAPABILITY,
        REALESRGAN_WINDOWS_CAPABILITY, WINDOWS_DIFFUSERS_WORKER, WINDOWS_ONNX_WORKER,
        WINDOWS_REALESRGAN_WORKER,
    },
    recipes::{
        BIREFNET_CAPABILITY, KLEIN_EDIT_CAPABILITY, KLEIN_TEXT_CAPABILITY,
        MOSS_IMAGE_SFT_CAPABILITY, QWEN_IMAGE_21_CAPABILITY, QWEN_IMAGE_21_TURBO_CAPABILITY,
        REALESRGAN_CAPABILITY, Recipe, validate_recipe_plan,
    },
};
use uuid::Uuid;
use worker_protocol::generation::{
    self as wire, Envelope, ExecuteRequest, ExecuteRequestV2, ExecutionContext, HostSession,
    InputAssetDescriptor, Message, OutputCandidate, OutputCandidateV2, OutputSpec, ProgressStage,
    WorkerEvent, WorkerIdentity,
};

#[derive(Clone, Debug, Default, Serialize)]
pub struct ExecutionView {
    pub busy: bool,
    pub job_id: Option<String>,
    pub attempt_id: Option<String>,
    pub stage: String,
    pub summary: String,
    pub step: Option<u32>,
    pub total: Option<u32>,
    pub logs: String,
    pub cleanup_error: Option<String>,
}

#[derive(Default)]
struct Control {
    cancel: AtomicBool,
    shutdown: AtomicBool,
}
struct State {
    view: ExecutionView,
    control: Option<Arc<Control>>,
}
struct Inner {
    paths: AppPaths,
    library: Arc<Mutex<Library>>,
    supervisor: PathBuf,
    state: Mutex<State>,
}
#[derive(Clone)]
pub struct GenerationController {
    inner: Arc<Inner>,
}

impl GenerationController {
    pub fn new(paths: AppPaths, library: Arc<Mutex<Library>>, supervisor_path: PathBuf) -> Self {
        Self {
            inner: Arc::new(Inner {
                paths,
                library,
                supervisor: supervisor_path,
                state: Mutex::new(State {
                    view: ExecutionView::default(),
                    control: None,
                }),
            }),
        }
    }
    pub fn snapshot(&self) -> ExecutionView {
        let mut view = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .view
            .clone();
        if !view.busy && lease_busy(&self.inner.paths.data.join("execution.lock")) {
            view.busy = true;
            view.stage = "cleanup".into();
            view.summary = "上次执行仍在清理，暂时无法启动新生成".into();
        }
        view
    }
    pub fn is_busy(&self) -> bool {
        self.snapshot().busy
    }
    pub fn start(&self, job: JobRecord, ready: ReadyPaths) -> Result<(), String> {
        {
            let mut state = self.inner.state.lock().map_err(error)?;
            if state.view.busy {
                return Err("已有活动生成，请等待本次执行完成".into());
            }
            if lease_busy(&self.inner.paths.data.join("execution.lock")) {
                drop(state);
                self.finish_failure(&job, "EXECUTION_BUSY", "旧执行尚未回收，未启动本次计算");
                return Err("生成资源仍忙碌，请等待清理完成".into());
            }
            let control = Arc::new(Control::default());
            state.view = ExecutionView {
                busy: true,
                job_id: Some(job.job_id.to_string()),
                attempt_id: Some(job.attempt.attempt_id.to_string()),
                stage: "preparing".into(),
                summary: "正在准备本地模型".into(),
                ..Default::default()
            };
            state.control = Some(control);
        }
        let this = self.clone();
        let failed_job = job.clone();
        if let Err(e) = std::thread::Builder::new()
            .name("studio-generation".into())
            .spawn(move || this.run(job, ready))
        {
            self.finish_failure(&failed_job, "HOST_THREAD", &e.to_string());
            return Err(e.to_string());
        }
        Ok(())
    }
    pub fn cancel(&self, job_id: &JobId) -> Result<JobRecord, String> {
        let mut library = self.inner.library.lock().map_err(error)?;
        let job = library.job(job_id).map_err(error)?;
        match library.request_cancel(&job.attempt).map_err(error)? {
            CancelOutcome::AlreadyFinished { job } => Ok(job),
            CancelOutcome::Accepted { job } => {
                drop(library);
                let mut state = self.inner.state.lock().map_err(error)?;
                if state.view.job_id.as_deref() == Some(job_id.as_str()) {
                    if let Some(control) = &state.control {
                        control.cancel.store(true, Ordering::Release);
                    }
                    state.view.stage = "cancel_requested".into();
                    state.view.summary = "正在取消并回收计算进程".into();
                }
                Ok(job)
            }
        }
    }
    /// Closing the host pipe is the native supervisor's cleanup trigger. No historical PID is used.
    pub fn shutdown(&self) {
        if let Some(control) = &self
            .inner
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .control
        {
            control.shutdown.store(true, Ordering::Release);
        }
    }
    fn finish_view(&self, stage: &str, summary: &str) {
        let mut state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        state.view.busy = false;
        state.view.stage = stage.into();
        state.view.summary = summary.into();
        state.view.cleanup_error = None;
        state.control = None;
    }
    fn finish_failure(&self, job: &JobRecord, code: &str, detail: &str) {
        let recorded = self
            .inner
            .library
            .lock()
            .map_err(error)
            .and_then(|mut library| {
                let record = library
                    .finish_after_reap(
                        &job.attempt,
                        Completion::Failed {
                            code: code.into(),
                            detail: detail.into(),
                        },
                    )
                    .map_err(error)?;
                let _ = library.cleanup_after_reap(&job.attempt);
                Ok(record)
            });
        match recorded {
            Ok(record) if record.state == AttemptState::Cancelled => {
                self.finish_view("cancelled", "生成已取消，资源已回收")
            }
            Ok(_) => self.finish_view("failed", detail),
            Err(storage_error) => {
                self.finish_view("storage_error", "计算已回收，但任务记录保存失败");
                self.inner
                    .state
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .view
                    .cleanup_error = Some(format!(
                    "任务记录保存失败：{storage_error}。请检查空间后重启应用，将未完成记录恢复为中断。"
                ));
            }
        }
    }
    fn run(&self, job: JobRecord, ready: ReadyPaths) {
        let result = self.prepare(&job, &ready);
        let (config, run_dir, control) = match result {
            Ok(value) => value,
            Err(detail) => {
                self.finish_failure(&job, "PREPARATION", &detail);
                return;
            }
        };
        let config_path = run_dir.join("supervisor.json");
        #[cfg(windows)]
        let spawn_result: Result<
            (Child, super::windows_process::WindowsProcessJob),
            String,
        > = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&config_path)
                .map_err(error)?;
            serde_json::to_writer(&mut file, &config).map_err(error)?;
            file.sync_all().map_err(error)?;
            let mut command = std::process::Command::new(&self.inner.supervisor);
            command
                .arg("--config")
                .arg(config_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let (child, job) =
                super::windows_process::spawn_suspended_in_job(&mut command).map_err(error)?;
            Ok((child, job))
        })();
        #[cfg(not(windows))]
        let spawn_result: Result<(Child, ()), String> = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&config_path)
                .map_err(error)?;
            serde_json::to_writer(&mut file, &config).map_err(error)?;
            file.sync_all().map_err(error)?;
            let mut command = std::process::Command::new(&self.inner.supervisor);
            command
                .arg("--config")
                .arg(config_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            command.spawn().map(|child| (child, ())).map_err(error)
        })();
        // The Windows job keeps the outer process group alive until stream cleanup finishes.
        // Other targets deliberately have no owning guard to drop.
        #[cfg_attr(not(windows), allow(unused_variables))]
        let (child, outer_job) = match spawn_result {
            Ok(spawned) => spawned,
            Err(detail) => {
                self.finish_failure(&job, "SUPERVISOR_START", &detail);
                return;
            }
        };
        let result = self.exchange(child, &job, &ready, &config, &control);
        #[cfg(windows)]
        drop(outer_job);
        match result {
            Ok(terminal) => {
                let final_record = self.commit_terminal(&job, terminal);
                match final_record {
                    Ok(record) => {
                        let (stage, summary) = match record.state {
                            AttemptState::Succeeded => ("succeeded", "图片已生成并保存到本机"),
                            AttemptState::Cancelled => ("cancelled", "生成已取消，资源已回收"),
                            _ => ("failed", "生成未完成，请查看任务详情"),
                        };
                        self.finish_view(stage, summary);
                    }
                    Err(detail) => {
                        // File/DB validation failure can only become Failed after native cleanup.
                        self.finish_failure(&job, "RESULT_COMMIT", &detail);
                    }
                }
            }
            Err(detail) => {
                self.finish_failure(&job, "GENERATION_EXCHANGE", &detail);
            }
        }
        if let Ok(library) = self.inner.library.lock() {
            let _ = library.cleanup_after_reap(&job.attempt);
        }
    }
    fn prepare(
        &self,
        job: &JobRecord,
        ready: &ReadyPaths,
    ) -> Result<(SupervisorConfig, PathBuf, Arc<Control>), String> {
        if ready.worker_module != "studio_mflux_worker"
            && ready.worker_module != "studio_mflux_worker_v2"
            && ready.worker_module != "studio_mflux_worker_v3"
            && ready.worker_module != "studio_onnx_worker"
            && ready.worker_module != "realesrgan-native-worker"
            && ready.worker_module != "studio_diffusers_worker"
            && ready.worker_module != WINDOWS_ZIMAGE_WORKER_MODULE
        {
            return Err("计算模块身份不符".into());
        }
        let control = self
            .inner
            .state
            .lock()
            .map_err(error)?
            .control
            .clone()
            .ok_or("执行控制已失效")?;
        let mut library = self.inner.library.lock().map_err(error)?;
        let source = library
            .source_facts_for_request(&job.request)
            .map_err(error)?;
        validate_recipe_plan(&job.request, &job.plan, source).map_err(error)?;
        library
            .mark_phase(&job.attempt, AttemptState::Preparing)
            .map_err(error)?;
        let run_dir = library
            .attempt_run_dir(&job.attempt)
            .map_err(error)?
            .canonicalize()
            .map_err(error)?;
        let recipe = Recipe::from_request(&job.request).map_err(error)?;
        if let Some(expected_build) = expected_worker_build(recipe)
            && ready.worker_build_id != expected_build
        {
            return Err("Windows 计算模块构建身份不符".into());
        }
        // Qwen base/Turbo and MOSS only start from the install-time prepared NF4
        // cache; the worker derives it from the model path, so refuse early with an
        // actionable message instead of failing inside the worker.
        let mut worker_args = ready.worker_args.clone();
        if staged_windows_recipe(recipe) {
            let prepared = prepared_cache_path(ready)?;
            for component in ["text_encoder", "transformer"] {
                if !prepared.join(component).join("config.json").is_file() {
                    return Err(format!(
                        "量化缓存缺失或损坏，请重新安装所选型号（{}）。",
                        prepared.display()
                    ));
                }
            }
            worker_args.push("--prepared-dir".into());
            worker_args.push(prepared.to_str().ok_or("量化缓存路径不是 UTF-8")?.into());
        }
        let timeouts = match recipe {
            Recipe::QwenImage21
            | Recipe::QwenImage21Turbo
            | Recipe::MossImageSft
            | Recipe::QwenImage21Windows
            | Recipe::QwenImage21TurboWindows
            | Recipe::MossImageSftWindows
            | Recipe::QwenImage21WindowsCompact
            | Recipe::QwenImage21TurboWindowsCompact
            | Recipe::MossImageSftWindowsCompact => Timeouts {
                handshake_ms: 30_000,
                loading_ms: 600_000,
                generation_ms: 1_920_000,
                cancel_grace_ms: 5_000,
                cleanup_deadline_ms: 30_000,
            },
            // Klein's four-step Windows recipe runs in a fresh worker process with
            // NF4 + CPU offload; keep the product 900 s generation budget explicit.
            Recipe::KleinTextWindows
            | Recipe::KleinEditWindows
            | Recipe::KleinTextWindowsCompact
            | Recipe::KleinEditWindowsCompact => Timeouts {
                handshake_ms: 30_000,
                loading_ms: 600_000,
                generation_ms: 900_000,
                cancel_grace_ms: 5_000,
                cleanup_deadline_ms: 30_000,
            },
            _ => Timeouts::default(),
        };
        let worker_program = match recipe {
            Recipe::RealEsrgan => self
                .inner
                .supervisor
                .parent()
                .ok_or("监督器目录缺失")?
                .join("realesrgan-worker"),
            Recipe::RealEsrganWindows => self
                .inner
                .supervisor
                .parent()
                .ok_or("监督器目录缺失")?
                .join("realesrgan-worker.exe"),
            _ => ready.worker_program.clone(),
        };
        let config = SupervisorConfig {
            python_program: worker_program,
            worker_args,
            working_dir: run_dir.clone(),
            execution_lock: self.inner.paths.data.join("execution.lock"),
            log_path: run_dir.join("worker.log"),
            expected_worker: WorkerIdentity {
                build_id: ready.worker_build_id.clone(),
                runtime_lock_id: ready.runtime_lock_id.clone(),
                model_snapshot_id: ready.model_snapshot_id.clone(),
            },
            protocol_version: if recipe == Recipe::ZImageTurbo { 1 } else { 2 },
            expected_capabilities: match recipe {
                Recipe::KleinText | Recipe::KleinEdit => {
                    vec![KLEIN_TEXT_CAPABILITY.into(), KLEIN_EDIT_CAPABILITY.into()]
                }
                Recipe::KleinTextWindows
                | Recipe::KleinEditWindows
                | Recipe::KleinTextWindowsCompact
                | Recipe::KleinEditWindowsCompact => vec![
                    KLEIN_WINDOWS_TEXT_CAPABILITY.into(),
                    KLEIN_WINDOWS_EDIT_CAPABILITY.into(),
                ],
                Recipe::BiRefNet => vec![BIREFNET_CAPABILITY.into()],
                Recipe::BiRefNetWindows => vec![BIREFNET_WINDOWS_CAPABILITY.into()],
                Recipe::RealEsrgan => vec![REALESRGAN_CAPABILITY.into()],
                Recipe::RealEsrganWindows => vec![REALESRGAN_WINDOWS_CAPABILITY.into()],
                Recipe::QwenImage21 => vec![QWEN_IMAGE_21_CAPABILITY.into()],
                Recipe::QwenImage21Windows | Recipe::QwenImage21WindowsCompact => {
                    vec![QWEN_IMAGE_21_WINDOWS_CAPABILITY.into()]
                }
                Recipe::QwenImage21Turbo => vec![QWEN_IMAGE_21_TURBO_CAPABILITY.into()],
                Recipe::QwenImage21TurboWindows | Recipe::QwenImage21TurboWindowsCompact => {
                    vec![QWEN_IMAGE_21_TURBO_WINDOWS_CAPABILITY.into()]
                }
                Recipe::MossImageSft => vec![MOSS_IMAGE_SFT_CAPABILITY.into()],
                Recipe::MossImageSftWindows | Recipe::MossImageSftWindowsCompact => {
                    vec![MOSS_IMAGE_SFT_WINDOWS_CAPABILITY.into()]
                }
                _ => vec![job.request.capability.capability_id.clone()],
            },
            timeouts,
        };
        Ok((config, run_dir, control))
    }
    /// The only return paths are before a process exists, or after it has exited and lease is free.
    fn exchange(
        &self,
        mut child: Child,
        job: &JobRecord,
        ready: &ReadyPaths,
        config: &SupervisorConfig,
        control: &Control,
    ) -> Result<Terminal, String> {
        let mut input = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (sender, receiver) = mpsc::sync_channel(8);
        let reader = stdout.map(|stdout| {
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stdout);
                loop {
                    match wire::read_frame(&mut reader) {
                        Ok(Some(frame)) => {
                            let event = Envelope::decode_frame(&frame).map(Box::new).map_err(error);
                            if sender.send(ReadEvent::Message(event)).is_err() {
                                break;
                            }
                        }
                        Ok(None) => {
                            let _ = sender.send(ReadEvent::Eof);
                            break;
                        }
                        Err(e) => {
                            let _ = sender.send(ReadEvent::Message(Err(e.to_string())));
                            let _ = sender.send(ReadEvent::Eof);
                            break;
                        }
                    }
                }
            })
        });
        let stderr_this = self.clone();
        let stderr_reader = stderr.map(|mut stream| {
            std::thread::spawn(move || {
                let mut buffer = [0; 4096];
                while let Ok(n) = stream.read(&mut buffer) {
                    if n == 0 {
                        break;
                    }
                    stderr_this.append_log(&buffer[..n]);
                }
            })
        });
        let mut terminal = None;
        let mut candidates = Vec::new();
        let mut fatal = None;
        let mut eof = reader.is_none();
        let recipe = Recipe::from_request(&job.request).map_err(error);
        let session_id = format!("session-{}", Uuid::new_v4());
        let mut session = match recipe.and_then(|recipe| {
            if recipe == Recipe::ZImageTurbo {
                HostSession::new(session_id, config.expected_worker.clone()).map_err(error)
            } else if recipe == Recipe::BiRefNet {
                HostSession::new_v2(
                    session_id,
                    config.expected_worker.clone(),
                    vec![BIREFNET_CAPABILITY.into()],
                )
                .map_err(error)
            } else if recipe == Recipe::RealEsrgan {
                HostSession::new_v2(
                    session_id,
                    config.expected_worker.clone(),
                    vec![REALESRGAN_CAPABILITY.into()],
                )
                .map_err(error)
            } else if recipe == Recipe::KleinText || recipe == Recipe::KleinEdit {
                HostSession::new_v2(
                    session_id,
                    config.expected_worker.clone(),
                    vec![KLEIN_TEXT_CAPABILITY.into(), KLEIN_EDIT_CAPABILITY.into()],
                )
                .map_err(error)
            } else if recipe == Recipe::QwenImage21 {
                HostSession::new_v2(
                    session_id,
                    config.expected_worker.clone(),
                    vec![QWEN_IMAGE_21_CAPABILITY.into()],
                )
                .map_err(error)
            } else if recipe == Recipe::QwenImage21Turbo {
                HostSession::new_v2(
                    session_id,
                    config.expected_worker.clone(),
                    vec![QWEN_IMAGE_21_TURBO_CAPABILITY.into()],
                )
                .map_err(error)
            } else if recipe == Recipe::MossImageSft {
                HostSession::new_v2(
                    session_id,
                    config.expected_worker.clone(),
                    vec![MOSS_IMAGE_SFT_CAPABILITY.into()],
                )
                .map_err(error)
            } else if recipe == Recipe::ZImageTurboCudaNf4 {
                HostSession::new_v2(
                    session_id,
                    config.expected_worker.clone(),
                    vec![job.request.capability.capability_id.clone()],
                )
                .map_err(error)
            } else if matches!(
                recipe,
                Recipe::KleinTextWindows
                    | Recipe::KleinEditWindows
                    | Recipe::KleinTextWindowsCompact
                    | Recipe::KleinEditWindowsCompact
            ) {
                HostSession::new_v2(
                    session_id,
                    config.expected_worker.clone(),
                    vec![
                        KLEIN_WINDOWS_TEXT_CAPABILITY.into(),
                        KLEIN_WINDOWS_EDIT_CAPABILITY.into(),
                    ],
                )
                .map_err(error)
            } else if recipe == Recipe::BiRefNetWindows {
                HostSession::new_v2(
                    session_id,
                    config.expected_worker.clone(),
                    vec![BIREFNET_WINDOWS_CAPABILITY.into()],
                )
                .map_err(error)
            } else if recipe == Recipe::RealEsrganWindows {
                HostSession::new_v2(
                    session_id,
                    config.expected_worker.clone(),
                    vec![REALESRGAN_WINDOWS_CAPABILITY.into()],
                )
                .map_err(error)
            } else if matches!(
                recipe,
                Recipe::QwenImage21Windows | Recipe::QwenImage21WindowsCompact
            ) {
                HostSession::new_v2(
                    session_id,
                    config.expected_worker.clone(),
                    vec![QWEN_IMAGE_21_WINDOWS_CAPABILITY.into()],
                )
                .map_err(error)
            } else if matches!(
                recipe,
                Recipe::QwenImage21TurboWindows | Recipe::QwenImage21TurboWindowsCompact
            ) {
                HostSession::new_v2(
                    session_id,
                    config.expected_worker.clone(),
                    vec![QWEN_IMAGE_21_TURBO_WINDOWS_CAPABILITY.into()],
                )
                .map_err(error)
            } else if matches!(
                recipe,
                Recipe::MossImageSftWindows | Recipe::MossImageSftWindowsCompact
            ) {
                HostSession::new_v2(
                    session_id,
                    config.expected_worker.clone(),
                    vec![MOSS_IMAGE_SFT_WINDOWS_CAPABILITY.into()],
                )
                .map_err(error)
            } else {
                Err("该后端会话尚未接入".into())
            }
        }) {
            Ok(s) => Some(s),
            Err(e) => {
                fatal = Some(e);
                None
            }
        };
        if input.is_none() || eof {
            fatal.get_or_insert("原生监督通道创建失败".into());
        }
        if let Some(session) = &mut session {
            match session.hello(
                format!("hello-{}", Uuid::new_v4()),
                "image-studio-host-0.1.0",
            ) {
                Ok(hello) => {
                    if let Some(stdin) = &mut input
                        && let Err(e) = wire::write_message(stdin, &hello)
                    {
                        fatal = Some(e.to_string());
                    }
                }
                Err(e) => fatal = Some(e.to_string()),
            }
        }
        let mut executed = false;
        let mut cancel_sent = false;
        let mut close_since = None;
        let mut status = None;
        let mut lifecycle_checked = false;
        let mut crashed_group = None;
        let mut last_log = Instant::now();
        loop {
            if control.shutdown.load(Ordering::Acquire) {
                fatal.get_or_insert("宿主正在退出，计算已请求回收".into());
            }
            if fatal.is_some() || control.cancel.load(Ordering::Acquire) && !executed {
                input.take();
                close_since.get_or_insert_with(Instant::now);
            } else if control.cancel.load(Ordering::Acquire) && executed && !cancel_sent {
                cancel_sent = true;
                if let (Some(session), Some(stdin)) = (&mut session, &mut input) {
                    match session
                        .cancel()
                        .and_then(|envelope| wire::write_message(stdin, &envelope))
                    {
                        Ok(()) => (),
                        Err(e) => {
                            fatal = Some(e.to_string());
                        }
                    }
                }
            }
            match receiver.recv_timeout(Duration::from_millis(40)) {
                Ok(ReadEvent::Message(Ok(envelope))) if fatal.is_none() => {
                    let accepted = session
                        .as_mut()
                        .ok_or_else(|| "缺少协议会话".to_string())
                        .and_then(|s| s.accept(&envelope).map_err(error));
                    match accepted {
                        Ok(WorkerEvent::HandshakeAccepted) => {
                            if control.cancel.load(Ordering::Acquire) {
                                input.take();
                                close_since.get_or_insert_with(Instant::now);
                                continue;
                            }
                            let execution = ExecutionContext {
                                job_id: job.job_id.to_string(),
                                attempt_id: job.attempt.attempt_id.to_string(),
                                execution_token: job.attempt.execution_token.to_string(),
                            };
                            let sent = Recipe::from_request(&job.request)
                                .map_err(error)
                                .and_then(|recipe| {
                                    let session = session.as_mut().ok_or("缺少协议会话")?;
                                    let envelope = if recipe == Recipe::ZImageTurbo {
                                        session.execute(
                                            format!("execute-{}", Uuid::new_v4()),
                                            execution,
                                            execute_request(job, ready, &config.working_dir)?,
                                        )
                                    } else {
                                        session.execute_v2(
                                            format!("execute-{}", Uuid::new_v4()),
                                            execution,
                                            execute_request_v2(job, ready, &config.working_dir)?,
                                        )
                                    }
                                    .map_err(error)?;
                                    Ok::<_, String>(envelope)
                                })
                                .and_then(|envelope| {
                                    wire::write_message(
                                        input.as_mut().ok_or("监督输入已关闭")?,
                                        &envelope,
                                    )
                                    .map_err(error)
                                });
                            match sent {
                                Ok(()) => executed = true,
                                Err(e) => fatal = Some(e),
                            }
                        }
                        Ok(WorkerEvent::Progress) => {
                            if let Message::Progress {
                                stage,
                                completed,
                                total,
                            } = &envelope.message
                            {
                                self.progress(job, *stage, *completed, *total);
                            }
                        }
                        Ok(WorkerEvent::OutputCandidate) => match envelope.message {
                            Message::OutputCandidate { candidate } => {
                                candidates.push(v1_role_candidate(candidate));
                            }
                            Message::OutputCandidateV2 { candidate } => {
                                candidates.push(v2_role_candidate(candidate));
                            }
                            _ => {
                                fatal = Some("候选消息与协议版本不符".into());
                            }
                        },
                        Ok(WorkerEvent::Cancelled) => terminal = Some(Terminal::Cancelled),
                        Ok(WorkerEvent::Error) => {
                            if let Message::Error { code, summary } = envelope.message {
                                terminal = Some(Terminal::Failed {
                                    code,
                                    detail: summary,
                                });
                            }
                        }
                        Ok(WorkerEvent::CleanupIncomplete) => {
                            self.cleanup_incomplete("计算回收尚未完成，资源保持忙碌");
                        }
                        Err(e) => {
                            fatal = Some(e);
                        }
                    }
                }
                Ok(ReadEvent::Message(Err(e))) => {
                    fatal.get_or_insert(e);
                }
                Ok(ReadEvent::Eof) | Err(mpsc::RecvTimeoutError::Disconnected) => eof = true,
                _ => (),
            }
            if status.is_none() {
                match child.try_wait() {
                    Ok(value) => status = value,
                    Err(e) => {
                        fatal.get_or_insert(e.to_string());
                        input.take();
                    }
                }
            }
            if status.is_some_and(|status| !status.success()) && !lifecycle_checked {
                lifecycle_checked = true;
                match inference_supervisor::read_current_lifecycle(&config.working_dir, &child) {
                    Ok(record) => {
                        if record.group_alive() {
                            self.cleanup_incomplete("原生监督异常退出，正在回收本次计算进程组");
                            if let Err(error) = record.terminate_group() {
                                self.append_log(
                                    format!("emergency group cleanup failed: {error}\n").as_bytes(),
                                );
                            }
                        }
                        crashed_group = Some(record);
                    }
                    Err(error) if lease_busy(&config.execution_lock) => {
                        self.cleanup_incomplete("监督已退出，但本次计算身份无法核实；资源保持忙碌");
                        self.append_log(
                            format!("current lifecycle unavailable: {error}\n").as_bytes(),
                        );
                    }
                    Err(_) => (),
                }
            }
            if close_since.is_some_and(|since| since.elapsed() > Duration::from_secs(5))
                && status.is_none()
            {
                self.cleanup_incomplete("原生监督仍在清理，尚不能确认资源已释放");
            }
            if last_log.elapsed() >= Duration::from_secs(1) {
                if let Ok(bytes) = tail(&config.log_path) {
                    self.replace_log(&bytes);
                }
                last_log = Instant::now();
            }
            if status.is_some() && eof {
                input.take();
                if !lease_busy(&config.execution_lock)
                    && crashed_group
                        .as_ref()
                        .is_none_or(|record| !record.group_alive())
                {
                    break;
                }
                self.cleanup_incomplete("旧计算仍持有执行租约，正在等待回收");
            }
        }
        drop(receiver);
        if let Some(reader) = reader {
            let _ = reader.join();
        }
        if let Some(reader) = stderr_reader {
            let _ = reader.join();
        }
        if let Ok(bytes) = tail(&config.log_path) {
            self.replace_log(&bytes);
        }
        if let Some(detail) = fatal {
            return Err(detail);
        }
        if control.cancel.load(Ordering::Acquire) {
            return Ok(Terminal::Cancelled);
        }
        if !status.is_some_and(|s| s.success()) {
            return Ok(match terminal {
                Some(Terminal::Failed { code, detail }) => Terminal::Failed { code, detail },
                _ => Terminal::Failed {
                    code: "SUPERVISOR_EXIT".into(),
                    detail: "原生监督异常退出，计算已回收".into(),
                },
            });
        }
        if !candidates.is_empty() {
            return Ok(Terminal::Candidates(candidates));
        }
        Ok(terminal.unwrap_or(Terminal::Failed {
            code: "MISSING_OUTPUT".into(),
            detail: "执行未提交候选图片".into(),
        }))
    }
    fn progress(
        &self,
        job: &JobRecord,
        stage: ProgressStage,
        step: Option<u32>,
        total: Option<u32>,
    ) {
        let next = match stage {
            ProgressStage::Loading | ProgressStage::Encoding => AttemptState::Preparing,
            ProgressStage::Sampling | ProgressStage::Inference => AttemptState::Running,
            ProgressStage::Decoding | ProgressStage::Writing => AttemptState::Committing,
        };
        if let Ok(mut library) = self.inner.library.lock()
            && library
                .job(&job.job_id)
                .is_ok_and(|record| record.state != next)
        {
            let _ = library.mark_phase(&job.attempt, next);
        }
        let mut state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        if state
            .control
            .as_ref()
            .is_some_and(|c| c.cancel.load(Ordering::Acquire))
        {
            return;
        }
        state.view.stage = format!("{stage:?}").to_lowercase();
        let stage_name = state.view.stage.clone();
        state.view.step = step;
        state.view.total = total;
        state.view.summary = match stage {
            ProgressStage::Loading => "正在加载模型",
            ProgressStage::Encoding => "正在编码提示词",
            ProgressStage::Sampling => "正在采样",
            ProgressStage::Inference => "正在执行本地推理",
            ProgressStage::Decoding => "正在解码图片",
            ProgressStage::Writing => "正在保存候选图片",
        }
        .into();
        drop(state);
        if let Ok(library) = self.inner.library.lock() {
            let _ = library.update_turn_progress(&job.job_id, &stage_name, step, total);
        }
    }
    fn cleanup_incomplete(&self, message: &str) {
        let mut state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        state.view.busy = true;
        state.view.stage = "cleanup".into();
        state.view.summary = message.into();
        state.view.cleanup_error = Some(message.into());
    }
    fn append_log(&self, bytes: &[u8]) {
        let mut state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        state.view.logs.push_str(&String::from_utf8_lossy(bytes));
        trim_log(&mut state.view.logs);
    }
    fn replace_log(&self, bytes: &[u8]) {
        let mut state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        state.view.logs = String::from_utf8_lossy(bytes).into_owned();
        trim_log(&mut state.view.logs);
    }
    fn commit_terminal(&self, job: &JobRecord, terminal: Terminal) -> Result<JobRecord, String> {
        let mut library = self.inner.library.lock().map_err(error)?;
        match terminal {
            Terminal::Candidates(candidates) => {
                match library.commit_outputs_after_reap(&job.attempt, candidates) {
                    Ok(record) => Ok(record),
                    Err(LibraryError::CancelAccepted) => library
                        .finish_after_reap(&job.attempt, Completion::Cancelled)
                        .map_err(error),
                    Err(e) => Err(e.to_string()),
                }
            }
            Terminal::Cancelled => library
                .finish_after_reap(&job.attempt, Completion::Cancelled)
                .map_err(error),
            Terminal::Failed { code, detail } => library
                .finish_after_reap(&job.attempt, Completion::Failed { code, detail })
                .map_err(error),
        }
    }
}

/// Qwen base/Turbo and MOSS Windows recipes start from an install-time prepared cache.
fn staged_windows_recipe(recipe: Recipe) -> bool {
    matches!(
        recipe,
        Recipe::QwenImage21Windows
            | Recipe::QwenImage21TurboWindows
            | Recipe::MossImageSftWindows
            | Recipe::QwenImage21WindowsCompact
            | Recipe::QwenImage21TurboWindowsCompact
            | Recipe::MossImageSftWindowsCompact
    )
}
/// The Windows diffusers worker derives the prepared cache by appending
/// `.prepared` to the model path, so the host must derive exactly that path from
/// the installed model directory (not from the worker snapshot identity).
fn prepared_cache_path(ready: &ReadyPaths) -> Result<PathBuf, String> {
    let model = ready.model_path.to_str().ok_or("模型路径不是 UTF-8")?;
    Ok(PathBuf::from(format!("{model}.prepared")))
}
fn expected_worker_build(recipe: Recipe) -> Option<&'static str> {
    match recipe {
        Recipe::KleinTextWindows
        | Recipe::KleinEditWindows
        | Recipe::KleinTextWindowsCompact
        | Recipe::KleinEditWindowsCompact
        | Recipe::QwenImage21Windows
        | Recipe::QwenImage21TurboWindows
        | Recipe::MossImageSftWindows
        | Recipe::QwenImage21WindowsCompact
        | Recipe::QwenImage21TurboWindowsCompact
        | Recipe::MossImageSftWindowsCompact => Some(WINDOWS_DIFFUSERS_WORKER),
        Recipe::BiRefNetWindows => Some(WINDOWS_ONNX_WORKER),
        Recipe::RealEsrganWindows => Some(WINDOWS_REALESRGAN_WORKER),
        _ => None,
    }
}

enum Terminal {
    Candidates(Vec<RoleCandidate>),
    Cancelled,
    Failed { code: String, detail: String },
}
enum ReadEvent {
    Message(Result<Box<Envelope>, String>),
    Eof,
}
fn execute_request(
    job: &JobRecord,
    ready: &ReadyPaths,
    run: &Path,
) -> Result<ExecuteRequest, String> {
    Ok(ExecuteRequest {
        prompt: job.request.instruction.clone(),
        width: job.plan.effective_output.width,
        height: job.plan.effective_output.height,
        seed: job
            .plan
            .effective_parameters
            .get("seed")
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok())
            .ok_or("冻结种子无效")?,
        num_inference_steps: 9,
        quantization_bits: 4,
        model_path: ready.model_path.to_str().ok_or("模型路径不是UTF-8")?.into(),
        output_dir: run.to_str().ok_or("执行路径不是UTF-8")?.into(),
    })
}
fn execute_request_v2(
    job: &JobRecord,
    ready: &ReadyPaths,
    run: &Path,
) -> Result<ExecuteRequestV2, String> {
    let recipe = Recipe::from_request(&job.request).map_err(error)?;
    let recipe_id = job
        .plan
        .effective_parameters
        .get("recipe")
        .and_then(|value| value.as_str())
        .ok_or("冻结计划缺少 recipe")?;
    let inputs = job
        .request
        .inputs
        .first()
        .map(|binding| {
            let path = run.join("input.png");
            let (width, height) = image::ImageReader::open(&path)
                .map_err(error)?
                .into_dimensions()
                .map_err(error)?;
            Ok::<InputAssetDescriptor, String>(InputAssetDescriptor {
                role: binding.role.clone(),
                path: path.to_str().ok_or("源图路径不是 UTF-8")?.into(),
                width,
                height,
                sha256: binding.asset.content_hash.as_str().into(),
            })
        })
        .transpose()?
        .into_iter()
        .collect();
    let max_bytes = if matches!(recipe, Recipe::RealEsrgan | Recipe::RealEsrganWindows) {
        wire::MAX_UPSCALE_OUTPUT_BYTES
    } else {
        32 * 1024 * 1024
    };
    let birefnet = matches!(recipe, Recipe::BiRefNet | Recipe::BiRefNetWindows);
    let mut outputs = vec![OutputSpec {
        role: "primary".into(),
        data_type: if birefnet {
            "image_rgba".into()
        } else {
            "image_rgb".into()
        },
        width: job.plan.effective_output.width,
        height: job.plan.effective_output.height,
        max_bytes,
    }];
    if birefnet {
        outputs.push(OutputSpec {
            role: "mask".into(),
            data_type: "mask".into(),
            width: job.plan.effective_output.width,
            height: job.plan.effective_output.height,
            max_bytes: 32 * 1024 * 1024,
        });
    }
    Ok(ExecuteRequestV2 {
        model_id: job.request.model_variant_id.to_string(),
        capability_id: job.request.capability.capability_id.clone(),
        recipe_id: recipe_id.into(),
        recipe_version: 1,
        runtime_lock_id: ready.runtime_lock_id.clone(),
        model_snapshot_id: ready.model_snapshot_id.clone(),
        instruction: job.request.instruction.clone(),
        seed: job
            .plan
            .effective_parameters
            .get("seed")
            .and_then(|value| value.as_u64())
            .and_then(|value| u32::try_from(value).ok()),
        sampling_steps: job
            .plan
            .effective_parameters
            .get("steps")
            .and_then(|value| value.as_u64())
            .and_then(|value| u32::try_from(value).ok()),
        parameters: job.plan.effective_parameters.clone(),
        inputs,
        outputs,
        model_path: ready
            .model_path
            .to_str()
            .ok_or("模型路径不是 UTF-8")?
            .into(),
        output_dir: run.to_str().ok_or("执行路径不是 UTF-8")?.into(),
    })
}
fn v1_role_candidate(candidate: OutputCandidate) -> RoleCandidate {
    RoleCandidate {
        role: "primary".into(),
        candidate: CandidateDescriptor {
            relative_path: candidate.relative_path,
            width: candidate.width,
            height: candidate.height,
            byte_count: candidate.size_bytes,
            sha256: candidate.sha256,
        },
    }
}
fn v2_role_candidate(candidate: OutputCandidateV2) -> RoleCandidate {
    RoleCandidate {
        role: candidate.role,
        candidate: CandidateDescriptor {
            relative_path: candidate.relative_path,
            width: candidate.width,
            height: candidate.height,
            byte_count: candidate.size_bytes,
            sha256: candidate.sha256,
        },
    }
}
fn error(value: impl std::fmt::Display) -> String {
    value.to_string()
}
fn lease_busy(path: &Path) -> bool {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let Ok(file) = options.open(path) else {
        return true;
    };
    if !file.metadata().is_ok_and(|m| m.is_file()) {
        return true;
    }
    FileExt::try_lock_exclusive(&file).is_err()
}
fn tail(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    file.seek(SeekFrom::Start(
        len.saturating_sub(inference_supervisor::MEMORY_LOG_BYTES as u64),
    ))?;
    let mut out = Vec::new();
    file.take(inference_supervisor::MEMORY_LOG_BYTES as u64)
        .read_to_end(&mut out)?;
    Ok(out)
}
fn trim_log(text: &mut String) {
    let limit = inference_supervisor::MEMORY_LOG_BYTES;
    if text.len() > limit {
        let mut split = text.len() - limit;
        while !text.is_char_boundary(split) {
            split += 1;
        }
        text.drain(..split);
    }
}

#[cfg(all(test, unix))]
mod tests;

#[cfg(test)]
mod generation_windows_tests {
    use super::*;
    #[test]
    fn prepared_cache_path_appends_to_the_installed_model_directory() {
        let ready = ReadyPaths {
            worker_program: PathBuf::from("python.exe"),
            worker_args: vec![],
            model_path: PathBuf::from("data/models/qwen-image-2.1-windows-v1"),
            worker_module: "studio_diffusers_worker".into(),
            worker_build_id: WINDOWS_DIFFUSERS_WORKER.into(),
            runtime_lock_id: "runtime".into(),
            model_snapshot_id:
                "qwen-image-2.1-windows-staged-d26bb61231c349cf6b7896fa83353113880e1ba3".into(),
        };
        assert_eq!(
            prepared_cache_path(&ready).unwrap(),
            PathBuf::from("data/models/qwen-image-2.1-windows-v1.prepared")
        );
    }
}
