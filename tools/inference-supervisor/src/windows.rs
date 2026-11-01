use super::{BoundedLog, KEY_QUEUE_LIMIT, SupervisorConfig, invalid, publish_lifecycle};
use fs2::FileExt;
use std::{
    collections::VecDeque,
    ffi::c_void,
    fs::{File, OpenOptions},
    io::{self, BufReader, Read, Write},
    mem::{size_of, zeroed},
    os::windows::{io::AsRawHandle, process::CommandExt},
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc::{self, Receiver, Sender, SyncSender, TryRecvError, TrySendError},
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE, WAIT_TIMEOUT},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
        },
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
            QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
        },
        Threading::{
            CREATE_SUSPENDED, OpenProcess, OpenThread, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
            ResumeThread, THREAD_SUSPEND_RESUME, TerminateProcess, WaitForSingleObject,
        },
    },
};
use worker_protocol::generation::{
    self as protocol, Envelope, HostSession, Message, ProgressStage, WorkerEvent,
};

const WRITER_QUEUE_LIMIT: usize = 8;
const IO_TICK: Duration = Duration::from_millis(10);

pub(super) fn run(config: SupervisorConfig) -> io::Result<i32> {
    config.validate()?;
    let lease = ExecutionLease::acquire(&config.execution_lock)?;
    let logs = BoundedLog::new(&config.log_path)?;
    let job = WindowsJob::create()?;
    let mut command = Command::new(&config.python_program);
    command
        .args(&config.worker_args)
        .current_dir(&config.working_dir)
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
        .creation_flags(CREATE_SUSPENDED);
    let mut child = command.spawn()?;
    let startup = (|| -> io::Result<()> {
        job.assign(&child)?;
        publish_lifecycle(&config.working_dir, child.id())?;
        resume_suspended(child.id())
    })();
    if let Err(error) = startup {
        let _ = job.terminate();
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }

    let child_in = child
        .stdin
        .take()
        .ok_or_else(|| invalid("missing worker stdin"))?;
    let child_out = child
        .stdout
        .take()
        .ok_or_else(|| invalid("missing worker stdout"))?;
    let child_err = child
        .stderr
        .take()
        .ok_or_else(|| invalid("missing worker stderr"))?;

    let (host_tx, host_rx) = mpsc::sync_channel(KEY_QUEUE_LIMIT);
    let (worker_tx, worker_rx) = mpsc::sync_channel(KEY_QUEUE_LIMIT);
    let (stderr_tx, stderr_rx) = mpsc::sync_channel(KEY_QUEUE_LIMIT);
    start_frame_reader(std::io::stdin(), host_tx);
    start_frame_reader(child_out, worker_tx);
    start_chunk_reader(child_err, stderr_tx);

    let (host_writer_tx, host_writer_rx) = mpsc::sync_channel(WRITER_QUEUE_LIMIT);
    let (worker_writer_tx, worker_writer_rx) = mpsc::sync_channel(WRITER_QUEUE_LIMIT);
    let (writer_done_tx, writer_done_rx) = mpsc::channel();
    start_writer(
        std::io::stdout(),
        host_writer_rx,
        writer_done_tx.clone(),
        WriterTarget::Host,
    );
    start_writer(
        child_in,
        worker_writer_rx,
        writer_done_tx,
        WriterTarget::Worker,
    );

    let mut logs = logs;
    let mut session: Option<HostSession> = None;
    let mut execute_envelope: Option<Envelope> = None;
    let mut wire_sequence = 0u64;
    let mut candidates: Vec<Envelope> = Vec::new();
    let mut result_error: Option<(String, String)> = None;
    let mut cancel_started: Option<Instant> = None;
    let mut cleanup_reported = false;
    let mut host_connected = true;
    let mut worker_eof = false;
    let mut worker_eof_since: Option<Instant> = None;
    let mut child_status: Option<ExitStatus> = None;
    let started = Instant::now();
    let mut execution_started: Option<Instant> = None;
    let mut generation_started: Option<Instant> = None;
    let mut terminal_queued = false;
    let mut terminal_failed = false;
    let mut terminal_since: Option<Instant> = None;
    let mut to_worker = WriteQueue::default();
    let mut to_host = WriteQueue::default();
    let mut host_acks = 0usize;
    let mut worker_acks = 0usize;

    loop {
        let mut activity = false;
        'host: for _ in 0..KEY_QUEUE_LIMIT {
            let input = match host_rx.try_recv() {
                Ok(input) => input,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if host_connected {
                        host_connected = false;
                        cancel_started.get_or_insert_with(Instant::now);
                        activity = true;
                    }
                    break 'host;
                }
            };
            activity = true;
            match input {
                FrameInput::Envelope(envelope) => {
                    let accepted = (|| -> Result<(), protocol::Error> {
                        if session.is_none() {
                            session = Some(
                                if config.protocol_version == protocol::PROTOCOL_VERSION_V2 {
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
                                execute_envelope = Some((*envelope).clone());
                            }
                            Message::Cancel => {
                                cancel_started.get_or_insert_with(Instant::now);
                            }
                            _ => {}
                        }
                        to_worker
                            .frame(envelope.encode_line()?)
                            .map_err(protocol::Error::Io)?;
                        Ok(())
                    })();
                    if let Err(error) = accepted {
                        if error.is_ignorable_rejection() {
                            logs.append(format!("rejected host message: {error}\n").as_bytes())?;
                        } else {
                            result_error.get_or_insert(("HOST_PROTOCOL".into(), error.to_string()));
                            cancel_started.get_or_insert_with(Instant::now);
                        }
                    }
                }
                FrameInput::Eof => {
                    host_connected = false;
                    cancel_started.get_or_insert_with(Instant::now);
                    break;
                }
                FrameInput::Error(error) => {
                    result_error.get_or_insert(("HOST_PROTOCOL".into(), error));
                    host_connected = false;
                    cancel_started.get_or_insert_with(Instant::now);
                    break;
                }
            }
        }

        'worker: for _ in 0..KEY_QUEUE_LIMIT {
            let input = match worker_rx.try_recv() {
                Ok(input) => input,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    worker_eof = true;
                    worker_eof_since.get_or_insert_with(Instant::now);
                    break 'worker;
                }
            };
            activity = true;
            match input {
                FrameInput::Envelope(envelope) => {
                    worker_eof_since = None;
                    let accepted = (|| -> Result<(), protocol::Error> {
                        let event = session
                            .as_mut()
                            .ok_or(protocol::Error::UnexpectedMessage)?
                            .accept(&envelope)?;
                        match event {
                            WorkerEvent::OutputCandidate => candidates.push(*envelope),
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
                                to_host.progress(*envelope);
                            }
                            WorkerEvent::HandshakeAccepted => {
                                queue_host(&mut to_host, *envelope, &mut wire_sequence)
                                    .map_err(protocol::Error::Io)?;
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
                FrameInput::Eof => {
                    worker_eof = true;
                    worker_eof_since.get_or_insert_with(Instant::now);
                    break;
                }
                FrameInput::Error(error) => {
                    result_error.get_or_insert(("WORKER_PROTOCOL".into(), error));
                    cancel_started.get_or_insert_with(Instant::now);
                    worker_eof = true;
                    worker_eof_since.get_or_insert_with(Instant::now);
                    break;
                }
            }
        }

        for _ in 0..4 {
            match stderr_rx.try_recv() {
                Ok(ChunkInput::Data(bytes)) => {
                    logs.append(&bytes)?;
                    activity = true;
                }
                Ok(ChunkInput::Eof) => {
                    activity = true;
                    break;
                }
                Ok(ChunkInput::Error(error)) => {
                    logs.append(format!("worker log read failed: {error}\n").as_bytes())?;
                    activity = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    break;
                }
            }
        }

        let mut writer_disconnected = false;
        while let Ok(done) = writer_done_rx.try_recv() {
            activity = true;
            match done.target {
                WriterTarget::Host => {
                    host_acks = host_acks.saturating_sub(1);
                    if let Err(error) = done.result {
                        logs.append(format!("host output disconnected: {error}\n").as_bytes())?;
                        host_connected = false;
                        cancel_started.get_or_insert_with(Instant::now);
                    }
                }
                WriterTarget::Worker => {
                    worker_acks = worker_acks.saturating_sub(1);
                    if let Err(error) = done.result {
                        result_error
                            .get_or_insert(("WORKER_DISCONNECTED".into(), error.to_string()));
                        cancel_started.get_or_insert_with(Instant::now);
                    }
                }
            }
        }

        if child_status.is_none() {
            child_status = child.try_wait()?;
            activity |= child_status.is_some();
        }

        if !terminal_queued && cancel_started.is_none() {
            let timeout = if session
                .as_ref()
                .is_none_or(|session| !session.is_handshaken())
            {
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
            if child_status.is_some_and(|status| !status.success()) {
                result_error
                    .get_or_insert(("WORKER_EXIT".into(), "worker exited abnormally".into()));
                cancel_started.get_or_insert_with(Instant::now);
            }
            if child_status.is_none()
                && worker_eof_since.is_some_and(|since| {
                    since.elapsed() >= Duration::from_millis(config.timeouts.cancel_grace_ms)
                })
            {
                result_error.get_or_insert((
                    "WORKER_DISCONNECTED".into(),
                    "worker closed its control channel while still running".into(),
                ));
                cancel_started.get_or_insert_with(Instant::now);
            }
        }

        let active_processes = job.active_processes()?;
        if child_status.is_some() && active_processes > 0 {
            job.terminate()?;
            cancel_started.get_or_insert_with(Instant::now);
        }
        if let Some(since) = cancel_started {
            if since.elapsed() >= Duration::from_millis(config.timeouts.cancel_grace_ms)
                && job.active_processes()? > 0
            {
                job.terminate()?;
            }
            if since.elapsed() >= Duration::from_millis(config.timeouts.cleanup_deadline_ms)
                && job.active_processes()? > 0
                && !cleanup_reported
            {
                logs.append(
                    b"CLEANUP_INCOMPLETE: holding execution lease until the Job Object is empty\n",
                )?;
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

        let active_processes = job.active_processes()?;
        if child_status.is_some() && active_processes == 0 && worker_eof && !terminal_queued {
            let cancelled = session.as_ref().is_some_and(HostSession::cancel_requested);
            let messages = if cancelled {
                vec![Message::Cancelled]
            } else if let Some((code, summary)) = &result_error {
                vec![Message::Error {
                    code: code.clone(),
                    summary: summary.clone(),
                }]
            } else if child_status.is_some_and(|status| status.success()) {
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
            to_host.progress = None;
            if host_connected && let Some(execute) = &execute_envelope {
                for message in messages {
                    queue_host(&mut to_host, response(execute, message), &mut wire_sequence)?;
                }
            }
            terminal_queued = true;
            terminal_since = Some(Instant::now());
        }

        if host_connected {
            match to_host.flush_to(&host_writer_tx, &mut host_acks, &mut wire_sequence) {
                Ok(progress) => activity |= progress,
                Err(error) => {
                    logs.append(format!("host output disconnected: {error}\n").as_bytes())?;
                    host_connected = false;
                    cancel_started.get_or_insert_with(Instant::now);
                    writer_disconnected = true;
                }
            }
        }
        if !writer_disconnected {
            match to_worker.flush_to(&worker_writer_tx, &mut worker_acks, &mut wire_sequence) {
                Ok(progress) => activity |= progress,
                Err(error) => {
                    result_error.get_or_insert(("WORKER_DISCONNECTED".into(), error.to_string()));
                    cancel_started.get_or_insert_with(Instant::now);
                }
            }
        }

        if terminal_queued && (!host_connected || (to_host.is_empty() && host_acks == 0)) {
            logs.sync()?;
            std::hint::black_box(&lease);
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
        if terminal_since.is_some_and(|since| since.elapsed() >= Duration::from_secs(1)) {
            logs.append(b"host output did not drain after worker cleanup\n")?;
            logs.sync()?;
            std::hint::black_box(&lease);
            return Ok(1);
        }

        std::hint::black_box(&lease);
        if !activity {
            thread::sleep(IO_TICK);
        }
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
    queue: &mut WriteQueue,
    mut envelope: Envelope,
    sequence: &mut u64,
) -> io::Result<()> {
    envelope.sequence = *sequence;
    *sequence += 1;
    queue.frame(envelope.encode_line().map_err(invalid)?)
}

#[derive(Default)]
struct WriteQueue {
    queue: VecDeque<Vec<u8>>,
    progress: Option<Envelope>,
}

impl WriteQueue {
    fn frame(&mut self, bytes: Vec<u8>) -> io::Result<()> {
        if self.queue.len() >= KEY_QUEUE_LIMIT {
            return Err(io::Error::other("bounded control queue exhausted"));
        }
        self.queue.push_back(bytes);
        Ok(())
    }

    fn progress(&mut self, envelope: Envelope) {
        self.progress = Some(envelope);
    }

    fn is_empty(&self) -> bool {
        self.queue.is_empty() && self.progress.is_none()
    }

    fn flush_to(
        &mut self,
        writer: &SyncSender<WriterRequest>,
        awaiting: &mut usize,
        sequence: &mut u64,
    ) -> io::Result<bool> {
        if self.queue.is_empty()
            && let Some(mut progress) = self.progress.take()
        {
            progress.sequence = *sequence;
            *sequence += 1;
            self.queue
                .push_back(progress.encode_line().map_err(invalid)?);
        }
        let mut progress = false;
        while let Some(bytes) = self.queue.front() {
            match writer.try_send(WriterRequest {
                bytes: bytes.clone(),
            }) {
                Ok(()) => {
                    self.queue.pop_front();
                    *awaiting += 1;
                    progress = true;
                }
                Err(TrySendError::Full(_)) => break,
                Err(TrySendError::Disconnected(_)) => {
                    return Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "control writer stopped",
                    ));
                }
            }
        }
        Ok(progress)
    }
}

enum FrameInput {
    Envelope(Box<Envelope>),
    Eof,
    Error(String),
}

fn start_frame_reader(reader: impl Read + Send + 'static, sender: SyncSender<FrameInput>) {
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        loop {
            let input = match protocol::read_frame(&mut reader) {
                Ok(Some(frame)) => match Envelope::decode_frame(&frame) {
                    Ok(envelope) => FrameInput::Envelope(Box::new(envelope)),
                    Err(error) => FrameInput::Error(error.to_string()),
                },
                Ok(None) => FrameInput::Eof,
                Err(error) => FrameInput::Error(error.to_string()),
            };
            let terminal = !matches!(input, FrameInput::Envelope(_));
            if sender.send(input).is_err() || terminal {
                break;
            }
        }
    });
}

enum ChunkInput {
    Data(Vec<u8>),
    Eof,
    Error(String),
}

fn start_chunk_reader(reader: impl Read + Send + 'static, sender: SyncSender<ChunkInput>) {
    thread::spawn(move || {
        let mut reader = reader;
        let mut bytes = [0u8; 8192];
        loop {
            let input = match reader.read(&mut bytes) {
                Ok(0) => ChunkInput::Eof,
                Ok(length) => ChunkInput::Data(bytes[..length].to_vec()),
                Err(error) => ChunkInput::Error(error.to_string()),
            };
            let terminal = !matches!(input, ChunkInput::Data(_));
            if sender.send(input).is_err() || terminal {
                break;
            }
        }
    });
}

struct WriterRequest {
    bytes: Vec<u8>,
}

#[derive(Clone, Copy)]
enum WriterTarget {
    Host,
    Worker,
}

struct WriterDone {
    target: WriterTarget,
    result: io::Result<()>,
}

fn start_writer(
    mut writer: impl Write + Send + 'static,
    receiver: Receiver<WriterRequest>,
    done: Sender<WriterDone>,
    target: WriterTarget,
) {
    thread::spawn(move || {
        while let Ok(request) = receiver.recv() {
            let result = writer.write_all(&request.bytes);
            let terminal = result.is_err();
            if done.send(WriterDone { target, result }).is_err() || terminal {
                break;
            }
        }
    });
}

struct OwnedHandle(HANDLE);

impl OwnedHandle {
    fn new(raw: HANDLE) -> io::Result<Self> {
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(raw))
        }
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: the wrapper owns exactly one valid kernel handle.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

struct WindowsJob {
    handle: OwnedHandle,
}

impl WindowsJob {
    fn create() -> io::Result<Self> {
        // SAFETY: null security/name requests an unnamed, non-inheritable Job handle.
        let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        let handle = OwnedHandle::new(raw)?;
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION =
            // SAFETY: all-zero is the documented initial state for this plain C structure.
            unsafe { zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: the buffer and declared size match JOBOBJECT_EXTENDED_LIMIT_INFORMATION.
        let configured = unsafe {
            SetInformationJobObject(
                handle.0,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast::<c_void>(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { handle })
    }

    fn assign(&self, child: &Child) -> io::Result<()> {
        // SAFETY: both handles are live for the duration of the call; the process is suspended.
        let assigned = unsafe { AssignProcessToJobObject(self.handle.0, child.as_raw_handle()) };
        if assigned == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn active_processes(&self) -> io::Result<u32> {
        let mut accounting: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION =
            // SAFETY: all-zero is the documented initial state for this plain C structure.
            unsafe { zeroed() };
        let mut returned = 0u32;
        // SAFETY: the output buffer and declared size match the requested information class.
        let queried = unsafe {
            QueryInformationJobObject(
                self.handle.0,
                JobObjectBasicAccountingInformation,
                (&raw mut accounting).cast::<c_void>(),
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                &mut returned,
            )
        };
        if queried == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(accounting.ActiveProcesses)
    }

    fn terminate(&self) -> io::Result<()> {
        // SAFETY: the Job handle is owned by this supervisor invocation.
        if unsafe { TerminateJobObject(self.handle.0, 1) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl Drop for WindowsJob {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

fn resume_suspended(process_id: u32) -> io::Result<()> {
    // SAFETY: a system thread snapshot requires no borrowed pointers and is closed by OwnedHandle.
    let snapshot = OwnedHandle::new(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) })?;
    let mut entry = THREADENTRY32 {
        dwSize: size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    // SAFETY: entry points to writable storage with dwSize initialized as required.
    let mut present = unsafe { Thread32First(snapshot.0, &mut entry) } != 0;
    while present {
        if entry.th32OwnerProcessID == process_id {
            // SAFETY: the suspended process still owns this thread; open requests only resume access.
            let thread = OwnedHandle::new(unsafe {
                OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID)
            })?;
            // SAFETY: the thread handle grants THREAD_SUSPEND_RESUME.
            let previous = unsafe { ResumeThread(thread.0) };
            if previous == u32::MAX {
                return Err(io::Error::last_os_error());
            }
            if previous != 1 {
                return Err(invalid(format!(
                    "suspended worker had unexpected suspend count {previous}"
                )));
            }
            return Ok(());
        }
        // SAFETY: snapshot and entry remain valid across the enumeration call.
        present = unsafe { Thread32Next(snapshot.0, &mut entry) } != 0;
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "suspended worker primary thread was not found",
    ))
}

pub(super) fn process_alive(process_id: u32) -> bool {
    // SAFETY: opening for synchronization does not mutate the target.
    let raw = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, process_id) };
    let Ok(handle) = OwnedHandle::new(raw) else {
        return false;
    };
    // SAFETY: the handle grants PROCESS_SYNCHRONIZE.
    let result = unsafe { WaitForSingleObject(handle.0, 0) };
    result == WAIT_TIMEOUT
}

pub(super) fn terminate_process(process_id: u32) -> io::Result<()> {
    // SAFETY: request only terminate and synchronization rights for this current lifecycle PID.
    let raw = unsafe { OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, 0, process_id) };
    if raw.is_null() {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(87) {
            return Ok(());
        }
        return Err(error);
    }
    let handle = OwnedHandle::new(raw)?;
    // SAFETY: the handle grants PROCESS_TERMINATE.
    if unsafe { TerminateProcess(handle.0, 1) } == 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(5) {
            return Err(error);
        }
    }
    Ok(())
}

struct ExecutionLease {
    _file: File,
}

impl ExecutionLease {
    fn acquire(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        file.try_lock_exclusive()
            .map_err(|_| io::Error::other("execution lease is busy"))?;
        Ok(Self { _file: file })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn suspended_process_cannot_run_before_job_assignment_and_resume() {
        let root = tempfile::tempdir().unwrap();
        let marker = root.path().join("marker.txt");
        let script = root.path().join("write-marker.cmd");
        fs::write(
            &script,
            format!("@echo started>\"{}\"\r\n", marker.display()),
        )
        .unwrap();
        let job = WindowsJob::create().unwrap();
        let mut child = Command::new(std::env::var_os("COMSPEC").unwrap())
            .args(["/D", "/C"])
            .arg(&script)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_SUSPENDED)
            .spawn()
            .unwrap();
        job.assign(&child).unwrap();
        assert!(job.active_processes().unwrap() >= 1);
        thread::sleep(Duration::from_millis(200));
        assert!(
            !marker.exists(),
            "the worker executed before joining the Job Object"
        );
        resume_suspended(child.id()).unwrap();
        assert!(child.wait().unwrap().success());
        assert!(marker.exists());
        assert_eq!(job.active_processes().unwrap(), 0);
    }
}
