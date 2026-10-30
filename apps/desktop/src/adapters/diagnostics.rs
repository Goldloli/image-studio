use std::{
    path::PathBuf,
    process::Stdio,
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::watch,
    time::{Instant, timeout, timeout_at},
};
use worker_protocol::{Envelope, HostEvent, HostSession, MAX_FRAME_BYTES, Message, VersionRange};

const MAX_LOG_BYTES: usize = 128 * 1024;

#[derive(Clone, Debug)]
pub struct DiagnosticConfig {
    pub program: PathBuf,
    pub mode: String,
    pub work_dir: Option<PathBuf>,
    pub handshake_timeout: Duration,
    pub probe_timeout: Duration,
    pub shutdown_grace: Duration,
}

impl DiagnosticConfig {
    pub fn new(program: PathBuf) -> Self {
        Self {
            program,
            mode: "normal".into(),
            work_dir: None,
            handshake_timeout: Duration::from_secs(5),
            probe_timeout: Duration::from_secs(5),
            shutdown_grace: Duration::from_secs(2),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticPhase {
    Starting,
    Handshaking,
    Probing,
    Ready,
    Stopping,
    Stopped,
    Failed,
}

#[derive(Clone, Debug)]
pub struct DiagnosticSnapshot {
    pub phase: DiagnosticPhase,
    pub summary: String,
    pub probe_passed: bool,
    pub is_running: bool,
    pub pid: Option<u32>,
    pub child_pid: Option<u32>,
    pub error_code: Option<String>,
    pub logs: String,
    pub progress_events: u64,
}

type Shared = Arc<Mutex<DiagnosticSnapshot>>;
type Wake = Arc<dyn Fn() + Send + Sync>;

pub struct DiagnosticController {
    state: Shared,
    stop: watch::Sender<bool>,
    thread: Option<JoinHandle<()>>,
}

impl DiagnosticController {
    pub fn start(config: DiagnosticConfig, on_update: Wake) -> Self {
        let state = Arc::new(Mutex::new(DiagnosticSnapshot {
            phase: DiagnosticPhase::Starting,
            summary: "正在启动通信诊断；不执行图片生成".into(),
            probe_passed: false,
            is_running: true,
            pid: None,
            child_pid: None,
            error_code: None,
            logs: String::new(),
            progress_events: 0,
        }));
        let (stop, receiver) = watch::channel(false);
        let shared = state.clone();
        let thread = std::thread::spawn(move || {
            let result = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime.block_on(run(config, shared.clone(), receiver, &on_update)),
                Err(error) => Err(Failure::new("RUNTIME_START_FAILED", error.to_string())),
            };
            mutate(&shared, &on_update, |s| {
                if let Err(error) = result {
                    s.phase = DiagnosticPhase::Failed;
                    s.summary = error.summary;
                    s.error_code = Some(error.code);
                }
                s.is_running = false;
            });
        });
        Self {
            state,
            stop,
            thread: Some(thread),
        }
    }

    pub fn snapshot(&self) -> DiagnosticSnapshot {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Signals the background supervisor; never waits in the UI drawing path.
    pub fn stop(&self) {
        let _ = self.stop.send(true);
    }
}

impl Drop for DiagnosticController {
    fn drop(&mut self) {
        self.stop();
        if let Some(thread) = self.thread.take() {
            // The UI only replaces finished controllers; application exit also joins cleanup.
            let _ = thread.join();
        }
    }
}

#[derive(Debug)]
struct Failure {
    code: String,
    summary: String,
}

impl Failure {
    fn new(code: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            summary: summary.into(),
        }
    }
}

fn mutate(state: &Shared, wake: &Wake, update: impl FnOnce(&mut DiagnosticSnapshot)) {
    update(&mut state.lock().unwrap_or_else(|e| e.into_inner()));
    wake();
}

fn log(state: &Shared, text: &str) {
    let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
    state.logs.push_str(text);
    if state.logs.len() > MAX_LOG_BYTES {
        let mut remove = state.logs.len() - MAX_LOG_BYTES;
        while !state.logs.is_char_boundary(remove) {
            remove += 1;
        }
        state.logs.drain(..remove);
    }
}

async fn write(stdin: &mut ChildStdin, message: Envelope) -> Result<(), Failure> {
    let bytes = message
        .encode_line()
        .map_err(|e| Failure::new(e.code(), e.to_string()))?;
    stdin
        .write_all(&bytes)
        .await
        .map_err(|e| Failure::new("PIPE_WRITE", e.to_string()))?;
    stdin
        .flush()
        .await
        .map_err(|e| Failure::new("PIPE_WRITE", e.to_string()))
}

async fn read_frame(reader: &mut (impl AsyncBufRead + Unpin)) -> Result<Option<Vec<u8>>, Failure> {
    let mut frame = Vec::new();
    loop {
        let available = reader
            .fill_buf()
            .await
            .map_err(|e| Failure::new("PIPE_READ", e.to_string()))?;
        if available.is_empty() {
            return if frame.is_empty() {
                Ok(None)
            } else {
                Err(Failure::new("TRUNCATED_FRAME", "诊断控制帧未以换行结束"))
            };
        }
        let end = available.iter().position(|byte| *byte == b'\n');
        let take = end.unwrap_or(available.len());
        if frame.len().saturating_add(take) > MAX_FRAME_BYTES {
            return Err(Failure::new(
                "FRAME_TOO_LARGE",
                "控制消息超过 64 KiB，诊断已停止",
            ));
        }
        frame.extend_from_slice(&available[..take]);
        reader.consume(take + usize::from(end.is_some()));
        if end.is_some() {
            return Ok(Some(frame));
        }
    }
}

async fn receive(
    reader: &mut BufReader<ChildStdout>,
    session: &mut HostSession,
    state: &Shared,
    wake: &Wake,
) -> Result<HostEvent, Failure> {
    loop {
        let frame = read_frame(reader)
            .await?
            .ok_or_else(|| Failure::new("WORKER_LOST", "诊断 worker 已断开"))?;
        let envelope =
            Envelope::decode_frame(&frame).map_err(|e| Failure::new(e.code(), e.to_string()))?;
        match session.accept(&envelope) {
            Ok(event) => {
                match &envelope.message {
                    Message::Progress { .. } => {
                        // Conflation: one current snapshot, no growing UI event queue.
                        let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                        s.progress_events = s.progress_events.saturating_add(1);
                    }
                    Message::ProbeResult { child_pid, .. } => {
                        mutate(state, wake, |s| s.child_pid = *child_pid);
                    }
                    _ => {}
                }
                return Ok(event);
            }
            Err(error) if error.is_ignorable_rejection() => {
                log(state, &format!("拒绝消息 [{}] {}\n", error.code(), error));
            }
            Err(error) => return Err(Failure::new(error.code(), error.to_string())),
        }
    }
}

struct GroupGuard(u32);
impl GroupGuard {
    fn kill(&self) {
        #[cfg(unix)]
        if let Ok(pid) = i32::try_from(self.0)
            && pid > 0
        {
            // SAFETY: the positive PID belongs to the process group just spawned by us.
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        }
    }
}
impl Drop for GroupGuard {
    fn drop(&mut self) {
        self.kill();
    }
}

async fn run(
    config: DiagnosticConfig,
    state: Shared,
    mut stop: watch::Receiver<bool>,
    wake: &Wake,
) -> Result<(), Failure> {
    if *stop.borrow() {
        mutate(&state, wake, |s| {
            s.phase = DiagnosticPhase::Stopped;
            s.summary = "诊断已取消，未启动进程".into();
        });
        return Ok(());
    }
    let program = config.program.canonicalize().map_err(|e| {
        Failure::new(
            "WORKER_NOT_FOUND",
            format!(
                "无法打开诊断程序 {}：{e}。请先构建 workspace。",
                config.program.display()
            ),
        )
    })?;
    let mut command = Command::new(program);
    command
        .arg("--mode")
        .arg(&config.mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    if let Some(work) = &config.work_dir {
        command.current_dir(work);
    }
    let mut child = command
        .spawn()
        .map_err(|e| Failure::new("WORKER_START_FAILED", e.to_string()))?;
    let mut group = GroupGuard(
        child
            .id()
            .ok_or_else(|| Failure::new("WORKER_START_FAILED", "缺少进程标识"))?,
    );
    mutate(&state, wake, |s| {
        s.pid = Some(group.0);
        s.phase = DiagnosticPhase::Handshaking;
        s.summary = "正在协商诊断协议".into();
    });
    let mut input = child.stdin.take().expect("piped stdin");
    let mut output = BufReader::new(child.stdout.take().expect("piped stdout"));
    let mut stderr = child.stderr.take().expect("piped stderr");
    let log_state = state.clone();
    let log_task = tokio::spawn(async move {
        let mut chunk = [0u8; 4096];
        loop {
            match stderr.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(count) => log(&log_state, &String::from_utf8_lossy(&chunk[..count])),
            }
        }
    });
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut session = HostSession::new(
        format!("diagnostic-{}-{stamp}", group.0),
        VersionRange::CURRENT,
    )
    .map_err(|e| Failure::new(e.code(), e.to_string()))?;
    let result = session_loop(
        &config,
        &state,
        wake,
        &mut stop,
        &mut session,
        &mut child,
        &mut input,
        &mut output,
    )
    .await;
    mutate(&state, wake, |s| {
        s.phase = DiagnosticPhase::Stopping;
        s.summary = "正在停止诊断并清理进程".into();
    });
    let graceful = cleanup(
        &config,
        &mut session,
        &mut child,
        &mut input,
        &mut output,
        &state,
        wake,
    )
    .await;
    // Even a graceful parent may leave descendants holding stdout/stderr open.
    group.kill();
    // A reaped PID can be reused. Disarm the guard after the final group signal.
    group.0 = 0;
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill().await;
    }
    let _ = child.wait().await;
    let mut log_task = log_task;
    if timeout(Duration::from_millis(100), &mut log_task)
        .await
        .is_err()
    {
        log_task.abort();
        let _ = log_task.await;
    }
    drop(group);
    match result {
        Ok(()) => {
            mutate(&state, wake, |s| {
                s.phase = DiagnosticPhase::Stopped;
                s.summary = if graceful {
                    "诊断已停止，受管理进程已清理".into()
                } else {
                    s.error_code = Some("FORCED_TERMINATION".into());
                    "诊断未按期退出，已强制终止并清理进程".into()
                };
            });
            Ok(())
        }
        Err(error) => Err(error),
    }
}

#[allow(clippy::too_many_arguments)]
async fn session_loop(
    config: &DiagnosticConfig,
    state: &Shared,
    wake: &Wake,
    stop: &mut watch::Receiver<bool>,
    session: &mut HostSession,
    child: &mut Child,
    input: &mut ChildStdin,
    output: &mut BufReader<ChildStdout>,
) -> Result<(), Failure> {
    write(
        input,
        session
            .hello("hello", "image-studio-foundation")
            .map_err(|e| Failure::new(e.code(), e.to_string()))?,
    )
    .await?;
    tokio::select! {
        _ = stop.changed() => return Ok(()),
        event = timeout(config.handshake_timeout, receive(output, session, state, wake)) => {
            match event {
                Ok(Ok(HostEvent::HandshakeAccepted)) => {},
                Ok(Err(error)) => return Err(error),
                Err(_) => return Err(Failure::new("HANDSHAKE_TIMEOUT", "诊断握手超时")),
                _ => return Err(Failure::new("PROTOCOL_PHASE", "握手阶段收到错误消息")),
            }
        }
    }
    mutate(state, wake, |s| {
        s.phase = DiagnosticPhase::Probing;
        s.summary = "正在验证通信探针".into();
    });
    write(
        input,
        session
            .probe("probe", "image-studio-diagnostic-only")
            .map_err(|e| Failure::new(e.code(), e.to_string()))?,
    )
    .await?;
    let deadline = Instant::now() + config.probe_timeout;
    loop {
        tokio::select! {
            _ = stop.changed() => return Ok(()),
            status = child.wait() => return Err(Failure::new("WORKER_EXITED", format!("诊断 worker 提前退出：{status:?}"))),
            event = timeout_at(deadline, receive(output, session, state, wake)) => {
                match event {
                    Ok(Ok(HostEvent::ProbeCompleted)) => break,
                    Ok(Ok(_)) => {},
                    Ok(Err(error)) => return Err(error),
                    Err(_) => return Err(Failure::new("PROBE_TIMEOUT", "通信探针超时")),
                }
            }
        }
    }
    mutate(state, wake, |s| {
        s.phase = DiagnosticPhase::Ready;
        s.probe_passed = true;
        s.summary = "通信诊断通过；不代表 GPU、模型加载或图片生成通过".into();
    });
    loop {
        tokio::select! {
            _ = stop.changed() => return Ok(()),
            status = child.wait() => return Err(Failure::new("WORKER_EXITED", format!("诊断 worker 意外退出：{status:?}"))),
            event = receive(output, session, state, wake) => { event?; }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn cleanup(
    config: &DiagnosticConfig,
    session: &mut HostSession,
    child: &mut Child,
    input: &mut ChildStdin,
    output: &mut BufReader<ChildStdout>,
    state: &Shared,
    wake: &Wake,
) -> bool {
    let deadline = Instant::now() + config.shutdown_grace;
    if !session.is_handshaken() {
        return false;
    }
    let Ok(message) = session.shutdown("shutdown") else {
        return false;
    };
    if write(input, message).await.is_err() {
        return false;
    }
    let acknowledgement = timeout_at(deadline, async {
        loop {
            if let HostEvent::ShutdownAcknowledged = receive(output, session, state, wake).await? {
                return Ok::<_, Failure>(());
            }
        }
    })
    .await;
    if !matches!(acknowledgement, Ok(Ok(()))) {
        return false;
    }
    matches!(timeout_at(deadline, child.wait()).await, Ok(Ok(status)) if status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unfinished_line_is_rejected_before_unbounded_allocation() {
        let bytes = vec![b'x'; MAX_FRAME_BYTES + 1];
        let mut reader = BufReader::new(bytes.as_slice());
        assert_eq!(
            read_frame(&mut reader).await.unwrap_err().code,
            "FRAME_TOO_LARGE"
        );
    }

    #[tokio::test]
    async fn eof_does_not_complete_a_json_line_without_its_delimiter() {
        let mut reader = BufReader::new(&b"{}"[..]);
        assert_eq!(
            read_frame(&mut reader).await.unwrap_err().code,
            "TRUNCATED_FRAME"
        );
    }

    #[test]
    fn unicode_log_tail_stays_within_byte_budget() {
        let state = Arc::new(Mutex::new(DiagnosticSnapshot {
            phase: DiagnosticPhase::Starting,
            summary: String::new(),
            probe_passed: false,
            is_running: false,
            pid: None,
            child_pid: None,
            error_code: None,
            logs: String::new(),
            progress_events: 0,
        }));
        log(&state, &"中文".repeat(MAX_LOG_BYTES));
        assert!(state.lock().unwrap().logs.len() <= MAX_LOG_BYTES);
    }
}
