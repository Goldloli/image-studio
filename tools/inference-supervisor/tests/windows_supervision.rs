#![cfg(windows)]

use inference_supervisor::{SupervisorConfig, Timeouts};
use std::{
    fs,
    io::{BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, WAIT_TIMEOUT},
    System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
};
use worker_protocol::generation::{
    self as protocol, Envelope, ExecuteRequest, ExecutionContext, HostSession, Message,
    WorkerEvent, WorkerIdentity,
};

struct Harness {
    child: Child,
    input: Option<ChildStdin>,
    output: Receiver<Envelope>,
    session: HostSession,
    root: tempfile::TempDir,
    config: SupervisorConfig,
}

fn python() -> PathBuf {
    let output = Command::new("python")
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .expect("Python is required only for this process fixture");
    assert!(output.status.success());
    PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
}

impl Harness {
    fn new(mode: &str) -> Self {
        Self::with_mode(mode, None)
    }

    fn with_mode(mode: &str, execution_lock: Option<PathBuf>) -> Self {
        let root = tempfile::Builder::new()
            .prefix("生成 supervisor job ; ")
            .tempdir()
            .unwrap();
        let config = config_for(mode, root.path(), execution_lock);
        Self::from_config(config, root)
    }

    fn from_config(config: SupervisorConfig, root: tempfile::TempDir) -> Self {
        let config_path = root.path().join("config.json");
        fs::write(&config_path, serde_json::to_vec(&config).unwrap()).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_inference-supervisor"))
            .arg("--config")
            .arg(config_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let (tx, output) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            while let Ok(Some(frame)) = protocol::read_frame(&mut reader) {
                if let Ok(envelope) = Envelope::decode_frame(&frame)
                    && tx.send(envelope).is_err()
                {
                    break;
                }
            }
        });
        let session =
            HostSession::new("session:windows-job", config.expected_worker.clone()).unwrap();
        Self {
            child,
            input,
            output,
            session,
            root,
            config,
        }
    }

    fn send(&mut self, envelope: &Envelope) {
        self.input
            .as_mut()
            .unwrap()
            .write_all(&envelope.encode_line().unwrap())
            .unwrap();
    }

    fn start(&mut self) {
        let hello = self.session.hello("request:hello", "fixture-host").unwrap();
        self.send(&hello);
        assert_eq!(
            self.session.accept(&self.next()).unwrap(),
            WorkerEvent::HandshakeAccepted
        );
        let execute = self
            .session
            .execute(
                "request:execute",
                ExecutionContext {
                    job_id: "job:windows".into(),
                    attempt_id: "attempt:windows".into(),
                    execution_token: "token:windows".into(),
                },
                ExecuteRequest {
                    prompt: "Windows Job Object fixture / not model inference".into(),
                    width: 1024,
                    height: 768,
                    seed: 9,
                    num_inference_steps: 9,
                    quantization_bits: 4,
                    model_path: self
                        .root
                        .path()
                        .join("model")
                        .to_string_lossy()
                        .into_owned(),
                    output_dir: self.root.path().to_string_lossy().into_owned(),
                },
            )
            .unwrap();
        self.send(&execute);
    }

    fn next(&self) -> Envelope {
        self.output
            .recv_timeout(Duration::from_secs(8))
            .expect("supervisor did not produce bounded response")
    }

    fn terminal(&mut self) -> Envelope {
        loop {
            let envelope = self.next();
            let event = self.session.accept(&envelope).unwrap();
            if event != WorkerEvent::Progress {
                return envelope;
            }
        }
    }

    fn wait(&mut self) -> std::process::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < deadline, "supervisor did not exit");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn pids(&self) -> serde_json::Value {
        let path = self.root.path().join("pids.json");
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            if let Ok(bytes) = fs::read(&path)
                && let Ok(value) = serde_json::from_slice(&bytes)
            {
                return value;
            }
            assert!(Instant::now() < deadline, "worker did not start");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

fn config_for(
    mode: &str,
    root: &std::path::Path,
    execution_lock: Option<PathBuf>,
) -> SupervisorConfig {
    // Timeout fixtures keep the small budgets they assert against. Every other
    // fixture gets generous budgets: this test binary spawns several Python
    // workers and runs in parallel with the rest of the workspace suite, and the
    // old 1.5s/2.5s budgets occasionally turned CPU contention into a spurious
    // Error event (progress_flood_is_coalesced_and_logs_remain_bounded).
    let (handshake_ms, loading_ms, generation_ms) = match mode {
        "hang" | "generation-timeout" => (1500, 2500, 2500),
        _ => (5000, 10000, 10000),
    };
    SupervisorConfig {
        python_program: python(),
        worker_args: vec![
            format!("{}/tests/fixtures/worker.py", env!("CARGO_MANIFEST_DIR")),
            mode.into(),
            root.join("pids.json").to_string_lossy().into_owned(),
        ],
        working_dir: root.into(),
        execution_lock: execution_lock.unwrap_or_else(|| root.join("execution.lock")),
        log_path: root.join("worker.log"),
        expected_worker: WorkerIdentity {
            build_id: "fixture-worker".into(),
            runtime_lock_id: "fixture-runtime".into(),
            model_snapshot_id: "fixture-model".into(),
        },
        protocol_version: 1,
        expected_capabilities: vec!["image.text-to-image.z-image-turbo.v1".into()],
        timeouts: Timeouts {
            handshake_ms,
            loading_ms,
            generation_ms,
            cancel_grace_ms: 100,
            cleanup_deadline_ms: 1500,
        },
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn process_alive(process_id: u32) -> bool {
    // SAFETY: read-only synchronization query for a live fixture PID.
    let raw: HANDLE = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, process_id) };
    if raw.is_null() {
        return false;
    }
    // SAFETY: raw is an owned process handle with PROCESS_SYNCHRONIZE.
    let result = unsafe { WaitForSingleObject(raw, 0) };
    // SAFETY: this fixture helper owns raw.
    unsafe {
        CloseHandle(raw);
    }
    result == WAIT_TIMEOUT
}

fn gone(process_id: u32) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while process_alive(process_id) {
        assert!(
            Instant::now() < deadline,
            "process {process_id} survived Job cleanup"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn candidate_is_forwarded_only_after_the_job_is_empty() {
    let mut harness = Harness::new("normal");
    harness.start();
    assert!(matches!(
        harness.terminal().message,
        Message::OutputCandidate { .. }
    ));
    assert!(harness.wait().success());
    gone(harness.pids()["worker"].as_u64().unwrap() as u32);
}

#[test]
fn lifecycle_evidence_is_published_before_execute_and_tracks_the_job_tree() {
    let mut harness = Harness::new("descendant");
    let deadline = Instant::now() + Duration::from_secs(3);
    let record_path = harness
        .config
        .working_dir
        .join(inference_supervisor::LIFECYCLE_RECORD_NAME);
    while !record_path.exists() {
        assert!(
            Instant::now() < deadline,
            "lifecycle evidence must exist before the worker runs"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let lifecycle =
        inference_supervisor::read_current_lifecycle(&harness.config.working_dir, &harness.child)
            .unwrap();
    harness.start();
    let pids = harness.pids();
    assert_eq!(
        lifecycle.compute_pgid(),
        pids["worker"].as_u64().unwrap() as u32
    );
    assert!(lifecycle.group_alive());
    drop(harness.input.take());
    harness.wait();
    gone(pids["worker"].as_u64().unwrap() as u32);
    gone(pids["descendant"].as_u64().unwrap() as u32);
    assert!(!lifecycle.group_alive());
}

#[test]
fn lifecycle_publication_failure_kills_the_still_suspended_worker() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path()
            .join(inference_supervisor::LIFECYCLE_RECORD_NAME),
        b"occupied",
    )
    .unwrap();
    let config = config_for("normal", root.path(), None);
    let mut harness = Harness::from_config(config, root);
    assert!(!harness.wait().success());
    assert!(!harness.root.path().join("pids.json").exists());
}

#[test]
fn identity_mismatch_stops_before_execute_without_spawning_model_work() {
    let mut harness = Harness::new("bad-identity");
    let hello = harness
        .session
        .hello("request:hello", "fixture-host")
        .unwrap();
    harness.send(&hello);
    assert!(!harness.wait().success());
    assert!(!harness.root.path().join("pids.json").exists());
}

#[test]
fn host_eof_and_uncooperative_cancel_reap_the_entire_job() {
    let mut eof = Harness::new("descendant");
    eof.start();
    let eof_pids = eof.pids();
    drop(eof.input.take());
    eof.wait();
    gone(eof_pids["worker"].as_u64().unwrap() as u32);
    gone(eof_pids["descendant"].as_u64().unwrap() as u32);

    let mut cancel = Harness::new("hang");
    cancel.start();
    let cancel_pids = cancel.pids();
    let message = cancel.session.cancel().unwrap();
    cancel.send(&message);
    assert!(matches!(cancel.terminal().message, Message::Cancelled));
    assert!(cancel.wait().success());
    gone(cancel_pids["worker"].as_u64().unwrap() as u32);
}

#[test]
fn protocol_failures_and_timeouts_cannot_look_successful() {
    for mode in ["crash", "malformed", "oversize", "truncated"] {
        let mut harness = Harness::new(mode);
        harness.start();
        assert!(
            matches!(harness.terminal().message, Message::Error { .. }),
            "{mode}"
        );
        assert!(!harness.wait().success(), "{mode}");
    }
    for (mode, expected) in [
        ("hang", "LOADING_TIMEOUT"),
        ("generation-timeout", "GENERATION_TIMEOUT"),
    ] {
        let mut harness = Harness::new(mode);
        harness.start();
        assert!(matches!(
            harness.terminal().message,
            Message::Error { code, .. } if code == expected
        ));
        assert!(!harness.wait().success(), "{mode}");
        gone(harness.pids()["worker"].as_u64().unwrap() as u32);
    }
}

#[test]
fn progress_flood_is_coalesced_and_logs_remain_bounded() {
    let mut harness = Harness::new("flood");
    harness.start();
    let mut progress = 0;
    loop {
        let envelope = harness.next();
        match harness.session.accept(&envelope).unwrap() {
            WorkerEvent::Progress => progress += 1,
            WorkerEvent::OutputCandidate => break,
            event => panic!("unexpected flood result {event:?}"),
        }
    }
    assert!(harness.wait().success());
    assert!(
        progress < 4000,
        "Windows supervisor did not coalesce progress"
    );
    assert_eq!(
        fs::metadata(&harness.config.log_path).unwrap().len(),
        inference_supervisor::DISK_LOG_BYTES as u64
    );
}

#[test]
fn killing_the_supervisor_closes_the_job_and_reclaims_descendants() {
    let mut harness = Harness::new("descendant");
    harness.start();
    let pids = harness.pids();
    harness.child.kill().unwrap();
    harness.child.wait().unwrap();
    gone(pids["worker"].as_u64().unwrap() as u32);
    gone(pids["descendant"].as_u64().unwrap() as u32);
}

#[test]
fn execution_lease_blocks_a_second_job_until_the_first_tree_is_gone() {
    let mut first = Harness::new("descendant");
    first.start();
    let pids = first.pids();
    let shared = first.config.execution_lock.clone();

    let mut second = Harness::with_mode("normal", Some(shared.clone()));
    assert!(!second.wait().success());
    assert!(!second.root.path().join("pids.json").exists());

    first.child.kill().unwrap();
    first.child.wait().unwrap();
    gone(pids["worker"].as_u64().unwrap() as u32);
    gone(pids["descendant"].as_u64().unwrap() as u32);

    let mut third = Harness::with_mode("normal", Some(shared));
    third.start();
    assert!(matches!(
        third.terminal().message,
        Message::OutputCandidate { .. }
    ));
    assert!(third.wait().success());
}
