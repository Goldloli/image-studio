//! Native lifetime boundary for one generation Attempt. No model code runs in this process.
//! All executable paths and configuration originate in the trusted desktop host.

use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};
use worker_protocol::generation::{
    self as protocol, Envelope, HostSession, Message, ProgressStage, WorkerEvent, WorkerIdentity,
};
use worker_protocol::generation;

pub const MEMORY_LOG_BYTES: usize = 128 * 1024;
pub const DISK_LOG_BYTES: usize = 8 * 1024 * 1024;
const KEY_QUEUE_LIMIT: usize = 8;
pub const LIFECYCLE_RECORD_NAME: &str = "compute-lifecycle.json";

/// Ephemeral ownership evidence for this live supervisor invocation, never a recovery PID list.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessLifecycle {
    schema_version: u32,
    supervisor_pid: u32,
    compute_pgid: u32,
}
impl ProcessLifecycle {
    pub fn compute_pgid(&self) -> u32 {
        self.compute_pgid
    }
    pub fn group_alive(&self) -> bool {
        #[cfg(unix)]
        {
            group_alive(self.compute_pgid as i32)
        }
        #[cfg(not(unix))]
        {
            true
        }
    }
    pub fn terminate_group(&self) -> io::Result<()> {
        #[cfg(unix)]
        {
            signal_group(self.compute_pgid as i32, libc::SIGKILL)
        }
        #[cfg(not(unix))]
        {
            Err(io::Error::other("native group cleanup is unsupported"))
        }
    }
}

/// Only a caller retaining the current Child handle can authorize emergency cleanup.
/// A newly started host must not call this with a PID restored from its database or disk.
pub fn read_current_lifecycle(
    working_dir: &Path,
    supervisor: &Child,
) -> io::Result<ProcessLifecycle> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(working_dir.join(LIFECYCLE_RECORD_NAME))?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > 1024 {
        return Err(invalid("invalid lifecycle record type or size"));
    }
    let mut bytes = Vec::new();
    file.take(1025).read_to_end(&mut bytes)?;
    if bytes.len() > 1024 {
        return Err(invalid("lifecycle record exceeds limit"));
    }
    let record: ProcessLifecycle = serde_json::from_slice(&bytes).map_err(invalid)?;
    if record.schema_version != 1
        || record.supervisor_pid != supervisor.id()
        || record.compute_pgid <= 1
        || record.compute_pgid > i32::MAX as u32
        || record.compute_pgid == record.supervisor_pid
    {
        return Err(invalid(
            "lifecycle record does not belong to the current supervisor",
        ));
    }
    #[cfg(unix)]
    // SAFETY: getpgrp only reads this process's group identifier.
    if record.compute_pgid == unsafe { libc::getpgrp() } as u32 {
        return Err(invalid("lifecycle record refers to the host process group"));
    }
    Ok(record)
}

fn publish_lifecycle(working_dir: &Path, compute_pgid: u32) -> io::Result<()> {
    let temporary = working_dir.join("compute-lifecycle.pending");
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    serde_json::to_writer(
        &mut file,
        &ProcessLifecycle {
            schema_version: 1,
            supervisor_pid: std::process::id(),
            compute_pgid,
        },
    )
    .map_err(invalid)?;
    file.sync_all()?;
    let mut permissions = file.metadata()?.permissions();
    permissions.set_readonly(true);
    file.set_permissions(permissions)?;
    drop(file);
    // No-replace publication: a preexisting record can never be reused by another launch.
    std::fs::hard_link(&temporary, working_dir.join(LIFECYCLE_RECORD_NAME))?;
    std::fs::remove_file(temporary)?;
    File::open(working_dir)?.sync_all()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupervisorConfig {
    pub python_program: PathBuf,
    /// Defaults to -I -m studio_mflux_worker. Override only in a trusted fixture/test harness.
    #[serde(default = "worker_args")]
    pub worker_args: Vec<String>,
    pub working_dir: PathBuf,
    pub execution_lock: PathBuf,
    pub log_path: PathBuf,
    pub expected_worker: WorkerIdentity,
    #[serde(default = "generation_protocol_version")]
    pub protocol_version: u16,
    #[serde(default = "expected_capabilities")]
    pub expected_capabilities: Vec<String>,
    #[serde(default)]
    pub timeouts: Timeouts,
}

fn worker_args() -> Vec<String> {
    vec!["-I".into(), "-m".into(), "studio_mflux_worker".into()]
}
fn generation_protocol_version() -> u16 {
    generation::PROTOCOL_VERSION
}
fn expected_capabilities() -> Vec<String> {
    vec![generation::CAPABILITY.into()]
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timeouts {
    pub handshake_ms: u64,
    pub loading_ms: u64,
    pub generation_ms: u64,
    pub cancel_grace_ms: u64,
    pub cleanup_deadline_ms: u64,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            handshake_ms: 10_000,
            loading_ms: 600_000,
            generation_ms: 900_000,
            cancel_grace_ms: 3_000,
            cleanup_deadline_ms: 5_000,
        }
    }
}

impl SupervisorConfig {
    fn validate(&self) -> io::Result<()> {
        for path in [
            &self.python_program,
            &self.working_dir,
            &self.execution_lock,
            &self.log_path,
        ] {
            if !path.is_absolute() {
                return Err(invalid("supervisor paths must be absolute"));
            }
        }
        self.expected_worker.validate().map_err(invalid)?;
        if !matches!(
            self.protocol_version,
            generation::PROTOCOL_VERSION | generation::PROTOCOL_VERSION_V2
        ) || self.expected_capabilities.is_empty()
            || self.expected_capabilities.len() > 2
            || self
                .expected_capabilities
                .iter()
                .any(|value| value.is_empty() || value.len() > 256)
        {
            return Err(invalid("invalid generation protocol configuration"));
        }
        if self.worker_args.is_empty()
            || self.worker_args.len() > 16
            || self
                .worker_args
                .iter()
                .any(|arg| arg.len() > 4096 || arg.contains('\0'))
        {
            return Err(invalid("invalid worker arguments"));
        }
        let t = &self.timeouts;
        if t.handshake_ms == 0
            || t.loading_ms == 0
            || t.generation_ms == 0
            || t.cancel_grace_ms == 0
            || t.cleanup_deadline_ms < t.cancel_grace_ms
        {
            return Err(invalid("invalid supervisor timeouts"));
        }
        Ok(())
    }
}

pub fn main_entry(args: impl IntoIterator<Item = OsString>) -> io::Result<i32> {
    let mut args = args.into_iter();
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--config")) {
        return Err(invalid("expected --config <trusted JSON path>"));
    }
    let path = args.next().ok_or_else(|| invalid("missing config path"))?;
    if args.next().is_some() {
        return Err(invalid("unexpected command argument"));
    }
    let file = File::open(path)?;
    if file.metadata()?.len() > protocol::MAX_FRAME_BYTES as u64 {
        return Err(invalid("config exceeds 64 KiB"));
    }
    let config: SupervisorConfig = serde_json::from_reader(file).map_err(invalid)?;
    run(config)
}

#[cfg(not(unix))]
pub fn run(_: SupervisorConfig) -> io::Result<i32> {
    Err(io::Error::other(
        "native generation supervision is currently supported on Unix only",
    ))
}

#[cfg(unix)]
pub fn run(config: SupervisorConfig) -> io::Result<i32> {
    use std::os::{
        fd::{AsRawFd, FromRawFd},
        unix::process::CommandExt,
    };
    config.validate()?;
    let lease = ExecutionLease::acquire(&config.execution_lock)?;
    let mut command = Command::new(&config.python_program);
    command
        .args(&config.worker_args)
        .current_dir(&config.working_dir)
        .env("STUDIO_EXECUTION_LEASE_FD", lease.0.as_raw_fd().to_string())
        .env(
            "STUDIO_RUNTIME_LOCK_ID",
            &config.expected_worker.runtime_lock_id,
        )
        .env(
            "STUDIO_MODEL_SNAPSHOT_ID",
            &config.expected_worker.model_snapshot_id,
        )
        .env("HF_HUB_OFFLINE", "1")
        .env("TRANSFORMERS_OFFLINE", "1")
        .env("HF_HUB_DISABLE_TELEMETRY", "1")
        .env("DO_NOT_TRACK", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = command.spawn()?;
    let mut guard = GroupGuard {
        pgid: child.id() as i32,
        armed: true,
    };
    // Publish before accepting any Hello/Execute, so compute cannot begin without ownership evidence.
    publish_lifecycle(&config.working_dir, child.id())?;
    let mut child_in = child
        .stdin
        .take()
        .ok_or_else(|| invalid("missing worker stdin"))?;
    let mut child_out = child
        .stdout
        .take()
        .ok_or_else(|| invalid("missing worker stdout"))?;
    let mut child_err = child
        .stderr
        .take()
        .ok_or_else(|| invalid("missing worker stderr"))?;
    for fd in [
        libc::STDIN_FILENO,
        libc::STDOUT_FILENO,
        child_in.as_raw_fd(),
        child_out.as_raw_fd(),
        child_err.as_raw_fd(),
    ] {
        nonblocking(fd)?;
    }
    // Use raw files, avoiding stdio line buffering around nonblocking partial writes.
    let duplicate = |fd| -> io::Result<File> {
        // SAFETY: dup returns a fresh descriptor, transferred to exactly one File owner.
        let duplicated = unsafe { libc::dup(fd) };
        if duplicated < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { File::from_raw_fd(duplicated) })
    };
    let mut host_in = duplicate(libc::STDIN_FILENO)?;
    let mut host_out = duplicate(libc::STDOUT_FILENO)?;
    let mut host_frames = Frames::default();
    let mut worker_frames = Frames::default();
    let mut to_worker = PendingWrites::default();
    let mut to_host = PendingWrites::default();
    let mut logs = BoundedLog::new(&config.log_path)?;
    let mut session: Option<HostSession> = None;
    let mut execute_envelope: Option<Envelope> = None;
    let mut wire_sequence = 0;
    let mut candidates: Vec<Envelope> = Vec::new();
    let mut result_error: Option<(String, String)> = None;
    let mut cancel_started: Option<Instant> = None;
    let mut cleanup_reported = false;
    let mut host_connected = true;
    let mut worker_eof = false;
    let mut stderr_eof = false;
    let mut worker_eof_since: Option<Instant> = None;
    let mut child_status: Option<ExitStatus> = None;
    let started = Instant::now();
    let mut execution_started: Option<Instant> = None;
    let mut generation_started: Option<Instant> = None;
    let mut terminal_queued = false;
    let mut terminal_failed = false;
    loop {
        // Each read has a per-tick budget. Flooding cannot starve EOF, cancellation or deadlines.
        if host_connected {
            match host_frames.read(&mut host_in) {
                Ok((frames, eof)) => {
                    if eof {
                        host_connected = false;
                        cancel_started.get_or_insert_with(Instant::now);
                    }
                    for frame in frames {
                        let accepted = (|| -> Result<(), protocol::Error> {
                            let envelope = Envelope::decode_frame(&frame)?;
                            if session.is_none() {
                                session = Some(
                                    if config.protocol_version == generation::PROTOCOL_VERSION_V2 {
                                        HostSession::new_v2(
                                            envelope.session_id.clone(),
                                            config.expected_worker.clone(),
                                            config.expected_capabilities.clone(),
                                        )?
                                    } else {
                                        HostSession::new(
                                            envelope.session_id.clone(),
                                            config.expected_worker.clone(),
                                        )?
                                    },
                                );
                            }
                            session
                                .as_mut()
                                .expect("session created")
                                .observe_outbound(&envelope)?;
                            match &envelope.message {
                                Message::Execute { .. } | Message::ExecuteV2 { .. } => {
                                    execution_started = Some(Instant::now());
                                    execute_envelope = Some(envelope.clone());
                                }
                                Message::Cancel => {
                                    cancel_started.get_or_insert_with(Instant::now);
                                }
                                _ => {}
                            }
                            to_worker
                                .key(envelope.encode_line()?)
                                .map_err(protocol::Error::Io)?;
                            Ok(())
                        })();
                        if let Err(error) = accepted {
                            if error.is_ignorable_rejection() {
                                logs.append(
                                    format!("rejected host message: {error}\n").as_bytes(),
                                )?;
                            } else {
                                result_error
                                    .get_or_insert(("HOST_PROTOCOL".into(), error.to_string()));
                                cancel_started.get_or_insert_with(Instant::now);
                            }
                        }
                    }
                }
                Err(error) => {
                    result_error.get_or_insert(("HOST_PROTOCOL".into(), error.to_string()));
                    host_connected = false;
                    cancel_started.get_or_insert_with(Instant::now);
                }
            }
        }
        match worker_frames.read(&mut child_out) {
            Ok((frames, eof)) => {
                worker_eof |= eof;
                if eof {
                    worker_eof_since.get_or_insert_with(Instant::now);
                }
                for frame in frames {
                    let accepted = (|| -> Result<(), protocol::Error> {
                        let envelope = Envelope::decode_frame(&frame)?;
                        let event = session
                            .as_mut()
                            .ok_or(protocol::Error::UnexpectedMessage)?
                            .accept(&envelope)?;
                        match event {
                            WorkerEvent::OutputCandidate => candidates.push(envelope),
                            WorkerEvent::Cancelled => {
                                cancel_started.get_or_insert_with(Instant::now);
                            }
                            WorkerEvent::Error => {
                                if let Message::Error { code, summary } = &envelope.message {
                                    result_error.get_or_insert((code.clone(), summary.clone()));
                                }
                                cancel_started.get_or_insert_with(Instant::now);
                            }
                            WorkerEvent::CleanupIncomplete => {
                                return Err(protocol::Error::Invalid(
                                    "cleanup status is reserved for native supervisor",
                                ));
                            }
                            WorkerEvent::Progress => {
                                if !matches!(
                                    envelope.message,
                                    Message::Progress {
                                        stage: ProgressStage::Loading,
                                        ..
                                    }
                                ) {
                                    generation_started.get_or_insert_with(Instant::now);
                                }
                                to_host.progress(envelope);
                            }
                            WorkerEvent::HandshakeAccepted => {
                                queue_host(&mut to_host, envelope, &mut wire_sequence)
                                    .map_err(protocol::Error::Io)?
                            }
                        }
                        Ok(())
                    })();
                    if let Err(error) = accepted {
                        if error.is_ignorable_rejection() {
                            logs.append(format!("rejected worker message: {error}\n").as_bytes())?;
                        } else {
                            result_error
                                .get_or_insert(("WORKER_PROTOCOL".into(), error.to_string()));
                            cancel_started.get_or_insert_with(Instant::now);
                        }
                    }
                }
            }
            Err(error) => {
                result_error.get_or_insert(("WORKER_PROTOCOL".into(), error.to_string()));
                cancel_started.get_or_insert_with(Instant::now);
                // The control stream is abandoned after a framing error. A truncated/oversize
                // buffer cannot later turn into a valid EOF; cleanup still waits for the group.
                worker_eof = true;
            }
        }
        stderr_eof |= drain_log(&mut child_err, &mut logs)?;
        if child_status.is_none() {
            child_status = child.try_wait()?;
        }
        if !terminal_queued && cancel_started.is_none() {
            let timeout = if session.as_ref().is_none_or(|s| !s.is_handshaken()) {
                (started.elapsed() >= Duration::from_millis(config.timeouts.handshake_ms))
                    .then_some("HANDSHAKE_TIMEOUT")
            } else if let Some(generation) = generation_started {
                (generation.elapsed() >= Duration::from_millis(config.timeouts.generation_ms))
                    .then_some("GENERATION_TIMEOUT")
            } else {
                execution_started
                    .filter(|since| {
                        since.elapsed() >= Duration::from_millis(config.timeouts.loading_ms)
                    })
                    .map(|_| "LOADING_TIMEOUT")
            };
            if let Some(code) = timeout {
                result_error = Some((code.into(), "native execution deadline exceeded".into()));
                cancel_started = Some(Instant::now());
            }
            if child_status.is_some_and(|s| !s.success()) {
                result_error
                    .get_or_insert(("WORKER_EXIT".into(), "worker exited abnormally".into()));
                cancel_started = Some(Instant::now());
            }
            // Pipe EOF can precede waitpid observing a completely normal exit. Allow a bounded
            // exit handshake, while never trusting a still-running process's candidate.
            if child_status.is_none()
                && worker_eof_since.is_some_and(|since| {
                    since.elapsed() >= Duration::from_millis(config.timeouts.cancel_grace_ms)
                })
            {
                result_error.get_or_insert((
                    "WORKER_DISCONNECTED".into(),
                    "worker closed its control channel while still running".into(),
                ));
                cancel_started = Some(Instant::now());
            }
        }
        // A direct child exiting never proves its descendants are gone. Always clear the group.
        if child_status.is_some() && group_alive(guard.pgid) {
            signal_group(guard.pgid, libc::SIGKILL)?;
            cancel_started.get_or_insert_with(Instant::now);
        }
        if let Some(since) = cancel_started {
            if since.elapsed() >= Duration::from_millis(config.timeouts.cancel_grace_ms) {
                signal_group(guard.pgid, libc::SIGKILL)?;
            }
            if since.elapsed() >= Duration::from_millis(config.timeouts.cleanup_deadline_ms)
                && group_alive(guard.pgid)
                && !cleanup_reported
            {
                logs.append(b"CLEANUP_INCOMPLETE: holding execution lease until the process group is gone\n")?;
                if let Some(envelope) = &execute_envelope {
                    queue_host(
                        &mut to_host,
                        response(
                            envelope,
                            Message::Error {
                                code: "CLEANUP_INCOMPLETE".into(),
                                summary:
                                    "execution cleanup deadline exceeded; resources remain busy"
                                        .into(),
                            },
                        ),
                        &mut wire_sequence,
                    )?;
                }
                cleanup_reported = true;
            }
        }
        if child_status.is_some() && !group_alive(guard.pgid) && worker_eof && !terminal_queued {
            guard.armed = false;
            if host_connected && let Some(execute) = &execute_envelope {
                let cancelled = session.as_ref().is_some_and(HostSession::cancel_requested);
                let messages = if cancelled {
                    vec![Message::Cancelled]
                } else if let Some((code, summary)) = &result_error {
                    vec![Message::Error {
                        code: code.clone(),
                        summary: summary.clone(),
                    }]
                } else if child_status.is_some_and(|s| s.success()) {
                    if candidates.is_empty() {
                        vec![Message::Error {
                            code: "MISSING_OUTPUT".into(),
                            summary: "worker exited without an output candidate".into(),
                        }]
                    } else {
                        candidates
                            .drain(..)
                            .map(|envelope| envelope.message)
                            .collect()
                    }
                } else {
                    vec![Message::Error {
                        code: "WORKER_EXIT".into(),
                        summary: "worker exited abnormally".into(),
                    }]
                };
                terminal_failed = messages
                    .iter()
                    .any(|message| matches!(message, Message::Error { .. }));
                // Terminal/candidate events outrank the last coalesced progress snapshot.
                to_host.progress = None;
                for message in messages {
                    queue_host(&mut to_host, response(execute, message), &mut wire_sequence)?;
                }
            }
            terminal_queued = true;
        }
        if host_connected {
            to_host.promote_progress(&mut wire_sequence)?;
            if let Err(error) = to_host.flush(&mut host_out) {
                logs.append(format!("host output disconnected: {error}\n").as_bytes())?;
                host_connected = false;
                cancel_started.get_or_insert_with(Instant::now);
            }
        }
        if let Err(error) = to_worker.flush(&mut child_in) {
            result_error.get_or_insert(("WORKER_DISCONNECTED".into(), error.to_string()));
            cancel_started.get_or_insert_with(Instant::now);
        }
        if terminal_queued && (!host_connected || to_host.is_empty()) {
            logs.sync()?;
            return Ok(
                if terminal_failed
                    || !host_connected && result_error.is_some()
                    || execute_envelope.is_none()
                {
                    1
                } else {
                    0
                },
            );
        }
        // poll is bounded and reacts to host EOF even if Python is in a non-cooperative native call.
        let mut pollfds = [
            libc::pollfd {
                fd: if host_connected {
                    libc::STDIN_FILENO
                } else {
                    -1
                },
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: if worker_eof {
                    -1
                } else {
                    child_out.as_raw_fd()
                },
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: if stderr_eof {
                    -1
                } else {
                    child_err.as_raw_fd()
                },
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: pollfds is a valid mutable array; timeout prevents an unbounded wait.
        unsafe {
            libc::poll(pollfds.as_mut_ptr(), pollfds.len() as libc::nfds_t, 20);
        }
        // Keep the lease alive across all cleanup paths and while blocked on host output.
        std::hint::black_box(&lease);
    }
}

fn response(execute: &Envelope, message: Message) -> Envelope {
    Envelope {
        family: protocol::FAMILY.into(),
        version: execute.version,
        session_id: execute.session_id.clone(),
        request_id: execute.request_id.clone(),
        sequence: 0,
        execution: execute.execution.clone(),
        message,
    }
}

fn queue_host(
    queue: &mut PendingWrites,
    mut envelope: Envelope,
    sequence: &mut u64,
) -> io::Result<()> {
    // Progress events can have been merged. Assign a single native outgoing sequence to actual frames.
    envelope.sequence = *sequence;
    *sequence += 1;
    queue.key(envelope.encode_line().map_err(invalid)?)
}

#[derive(Default)]
struct PendingWrites {
    queue: VecDeque<Vec<u8>>,
    offset: usize,
    progress: Option<Envelope>,
}
impl PendingWrites {
    fn key(&mut self, bytes: Vec<u8>) -> io::Result<()> {
        if self.queue.len() >= KEY_QUEUE_LIMIT {
            return Err(io::Error::other("bounded control queue exhausted"));
        }
        self.queue.push_back(bytes);
        Ok(())
    }
    fn progress(&mut self, envelope: Envelope) {
        self.progress = Some(envelope);
    }
    fn promote_progress(&mut self, sequence: &mut u64) -> io::Result<()> {
        if self.queue.is_empty()
            && let Some(progress) = self.progress.take()
        {
            queue_host(self, progress, sequence)?;
        }
        Ok(())
    }
    fn is_empty(&self) -> bool {
        self.queue.is_empty() && self.progress.is_none()
    }
    fn flush(&mut self, writer: &mut impl Write) -> io::Result<()> {
        while let Some(bytes) = self.queue.front() {
            match writer.write(&bytes[self.offset..]) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "closed control channel",
                    ));
                }
                Ok(n) => {
                    self.offset += n;
                    if self.offset == bytes.len() {
                        self.queue.pop_front();
                        self.offset = 0;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct Frames {
    bytes: Vec<u8>,
}
impl Frames {
    fn read(&mut self, reader: &mut impl Read) -> io::Result<(Vec<Vec<u8>>, bool)> {
        let mut frames = Vec::new();
        let mut chunk = [0; 4096];
        // 16 KiB/tick per channel keeps memory and scheduler work bounded during a flood.
        for _ in 0..4 {
            match reader.read(&mut chunk) {
                Ok(0) if self.bytes.is_empty() => return Ok((frames, true)),
                Ok(0) => return Err(invalid("truncated control frame")),
                Ok(n) => {
                    for byte in &chunk[..n] {
                        if *byte == b'\n' {
                            frames.push(std::mem::take(&mut self.bytes));
                        } else {
                            if self.bytes.len() == protocol::MAX_FRAME_BYTES {
                                return Err(invalid("control frame exceeds 64 KiB"));
                            }
                            self.bytes.push(*byte);
                        }
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    return Ok((frames, false));
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        Ok((frames, false))
    }
}

struct BoundedLog {
    memory: VecDeque<u8>,
    file: File,
    written: usize,
}
impl BoundedLog {
    fn new(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().write(true).create_new(true).open(path)?;
        Ok(Self {
            memory: VecDeque::new(),
            file,
            written: 0,
        })
    }
    fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
        let bytes_for_disk = bytes.len().min(DISK_LOG_BYTES.saturating_sub(self.written));
        self.file.write_all(&bytes[..bytes_for_disk])?;
        self.written += bytes_for_disk;
        for byte in bytes {
            if self.memory.len() == MEMORY_LOG_BYTES {
                self.memory.pop_front();
            }
            self.memory.push_back(*byte);
        }
        Ok(())
    }
    fn sync(&self) -> io::Result<()> {
        self.file.sync_all()
    }
}

fn drain_log(reader: &mut impl Read, log: &mut BoundedLog) -> io::Result<bool> {
    let mut bytes = [0; 8192];
    for _ in 0..4 {
        match reader.read(&mut bytes) {
            Ok(0) => return Ok(true),
            Ok(n) => log.append(&bytes[..n])?,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(false)
}

#[cfg(unix)]
struct ExecutionLease(File);
#[cfg(unix)]
impl ExecutionLease {
    fn acquire(path: &Path) -> io::Result<Self> {
        use std::os::fd::AsRawFd;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        // SAFETY: valid owned descriptor. Nonblocking flock ensures old execution never queues a new one.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(io::Error::other("execution lease is busy"));
        }
        // Python inherits the same open file description, retaining the lease if this supervisor dies.
        // SAFETY: fcntl modifies flags on an owned, valid descriptor.
        if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, 0) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(file))
    }
}

#[cfg(unix)]
struct GroupGuard {
    pgid: i32,
    armed: bool,
}
#[cfg(unix)]
impl Drop for GroupGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = signal_group(self.pgid, libc::SIGKILL);
        }
    }
}

#[cfg(unix)]
fn nonblocking(fd: std::os::fd::RawFd) -> io::Result<()> {
    // SAFETY: descriptors remain owned by the surrounding loop throughout this call.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
#[cfg(unix)]
fn group_alive(pgid: i32) -> bool {
    // SAFETY: signal 0 only observes the fresh process group created by this supervisor.
    unsafe {
        libc::kill(-pgid, 0) == 0 || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
}
#[cfg(unix)]
fn signal_group(pgid: i32, signal: i32) -> io::Result<()> {
    // SAFETY: pgid belongs to this invocation, never a PID restored from disk.
    if unsafe { libc::kill(-pgid, signal) } == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    // Darwin can report EPERM for a group whose last member is exiting. This is not proof
    // of cleanup: keep the lease and poll group liveness until confirmed or report busy.
    if matches!(error.raw_os_error(), Some(libc::ESRCH) | Some(libc::EPERM)) {
        Ok(())
    } else {
        Err(error)
    }
}
fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn logs_and_control_queues_have_hard_limits() {
        let root = tempfile::tempdir().unwrap();
        let mut logs = BoundedLog::new(&root.path().join("log")).unwrap();
        for _ in 0..1025 {
            logs.append(&[b'x'; 8192]).unwrap();
        }
        assert_eq!(logs.memory.len(), MEMORY_LOG_BYTES);
        assert_eq!(logs.file.metadata().unwrap().len(), DISK_LOG_BYTES as u64);
        let mut queue = PendingWrites::default();
        for _ in 0..KEY_QUEUE_LIMIT {
            queue.key(vec![0]).unwrap();
        }
        assert!(queue.key(vec![0]).is_err());
    }
    #[test]
    fn incomplete_and_oversize_frames_are_rejected() {
        assert!(Frames::default().read(&mut &b"partial"[..]).is_err());
        let mut frames = Frames::default();
        let data = vec![b'x'; protocol::MAX_FRAME_BYTES + 1];
        let mut reader = &data[..];
        while frames.bytes.len() < protocol::MAX_FRAME_BYTES {
            frames.read(&mut reader).unwrap();
        }
        assert!(frames.read(&mut reader).is_err());
    }
}
