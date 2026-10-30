#![cfg(unix)]
use inference_supervisor::{SupervisorConfig, Timeouts};
use std::{
    fs,
    io::{BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
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
    let output = Command::new("python3")
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .expect("Python is required only for this process fixture");
    PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
}
impl Harness {
    fn new(mode: &str) -> Self {
        Self::with_mode(mode, None)
    }
    fn with_mode(mode: &str, execution_lock: Option<PathBuf>) -> Self {
        let root = tempfile::Builder::new()
            .prefix("生成 监督 ; ")
            .tempdir()
            .unwrap();
        let config = SupervisorConfig {
            python_program: python(),
            worker_args: vec![
                format!("{}/tests/fixtures/worker.py", env!("CARGO_MANIFEST_DIR")),
                mode.into(),
                root.path().join("pids.json").to_str().unwrap().into(),
            ],
            working_dir: root.path().into(),
            execution_lock: execution_lock.unwrap_or_else(|| root.path().join("execution.lock")),
            log_path: root.path().join("worker.log"),
            expected_worker: WorkerIdentity {
                build_id: "fixture-worker".into(),
                runtime_lock_id: "fixture-runtime".into(),
                model_snapshot_id: "fixture-model".into(),
            },
            timeouts: Timeouts {
                handshake_ms: 1000,
                loading_ms: 2000,
                generation_ms: 2000,
                cancel_grace_ms: 100,
                cleanup_deadline_ms: 1000,
            },
        };
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
            HostSession::new("session:integration", config.expected_worker.clone()).unwrap();
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
                    job_id: "job:integration".into(),
                    attempt_id: "attempt:integration".into(),
                    execution_token: "token:integration".into(),
                },
                ExecuteRequest {
                    prompt: "真实协议故障测试 / not model inference".into(),
                    width: 1024,
                    height: 768,
                    seed: 7,
                    num_inference_steps: 9,
                    quantization_bits: 4,
                    model_path: self.root.path().join("model").to_str().unwrap().into(),
                    output_dir: self.root.path().to_str().unwrap().into(),
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
        let deadline = Instant::now() + Duration::from_secs(3);
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
impl Drop for Harness {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}
fn gone(pid: u32) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while alive(pid) {
        assert!(
            Instant::now() < deadline,
            "process {pid} survived native cleanup"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn candidate_is_delivered_after_normal_exit_and_control_sequence_is_valid() {
    let mut h = Harness::new("normal");
    h.start();
    let terminal = h.terminal();
    assert!(
        matches!(terminal.message, Message::OutputCandidate { .. }),
        "{terminal:?}"
    );
    assert!(h.wait().success());
    gone(h.pids()["worker"].as_u64().unwrap() as u32);
}

#[test]
fn lifecycle_record_is_published_before_execute_and_bound_to_the_live_child_handle() {
    let mut h = Harness::new("hang");
    let deadline = Instant::now() + Duration::from_secs(3);
    while !h
        .config
        .working_dir
        .join(inference_supervisor::LIFECYCLE_RECORD_NAME)
        .exists()
    {
        assert!(
            Instant::now() < deadline,
            "ownership evidence must be published before any execute request"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let lifecycle =
        inference_supervisor::read_current_lifecycle(&h.config.working_dir, &h.child).unwrap();
    h.start();
    let pids = h.pids();
    assert_eq!(
        lifecycle.compute_pgid(),
        pids["worker"].as_u64().unwrap() as u32
    );
    assert!(lifecycle.group_alive());
    let mut other = Command::new(python())
        .args(["-c", "import time;time.sleep(60)"])
        .spawn()
        .unwrap();
    let rejected =
        inference_supervisor::read_current_lifecycle(&h.config.working_dir, &other).is_err();
    let _ = other.kill();
    let _ = other.wait();
    assert!(
        rejected,
        "another invocation cannot reuse this lifecycle evidence"
    );
    drop(h.input.take());
    h.wait();
    assert!(!lifecycle.group_alive());
}

#[test]
fn cooperative_and_noncooperative_cancel_reap_before_cancelled() {
    for mode in ["cooperate", "hang", "descendant"] {
        let mut h = Harness::new(mode);
        h.start();
        let pids = h.pids();
        let cancel = h.session.cancel().unwrap();
        h.send(&cancel);
        let terminal = h.terminal();
        assert!(
            matches!(terminal.message, Message::Cancelled),
            "{mode}: {terminal:?}"
        );
        assert!(h.wait().success());
        gone(pids["worker"].as_u64().unwrap() as u32);
        if let Some(pid) = pids["descendant"].as_u64() {
            gone(pid as u32);
        }
    }
}

#[test]
fn host_eof_reclaims_worker_and_descendant_without_host_cooperation() {
    let mut h = Harness::new("descendant");
    h.start();
    let pids = h.pids();
    drop(h.input.take());
    h.wait();
    gone(pids["worker"].as_u64().unwrap() as u32);
    gone(pids["descendant"].as_u64().unwrap() as u32);
}

#[test]
fn candidates_wait_for_descendants_and_abnormal_workers_do_not_succeed() {
    let mut orphan = Harness::new("orphan");
    orphan.start();
    let pids = orphan.pids();
    assert!(matches!(
        orphan.terminal().message,
        Message::OutputCandidate { .. }
    ));
    orphan.wait();
    gone(pids["descendant"].as_u64().unwrap() as u32);
    for mode in ["crash", "malformed", "oversize", "truncated"] {
        let mut h = Harness::new(mode);
        h.start();
        let terminal = h.terminal();
        assert!(
            matches!(terminal.message, Message::Error { .. }),
            "{mode}: {terminal:?}"
        );
        assert!(!h.wait().success());
    }
}

#[test]
fn native_loading_and_generation_timeouts_are_distinct_and_cleanup() {
    for (mode, code) in [
        ("hang", "LOADING_TIMEOUT"),
        ("generation-timeout", "GENERATION_TIMEOUT"),
    ] {
        let mut h = Harness::new(mode);
        h.start();
        let terminal = h.terminal();
        assert!(matches!(terminal.message,Message::Error {code:ref actual,..} if actual==code));
        assert!(!h.wait().success());
        gone(h.pids()["worker"].as_u64().unwrap() as u32);
    }
}

#[test]
fn flood_is_bounded_and_terminal_cannot_be_starved_by_progress() {
    let mut h = Harness::new("flood");
    h.config.timeouts.loading_ms = 8000;
    h.start();
    let mut progress = 0;
    loop {
        let envelope = h.next();
        match h.session.accept(&envelope).unwrap() {
            WorkerEvent::Progress => progress += 1,
            WorkerEvent::OutputCandidate => break,
            event => panic!("unexpected flood result {event:?}"),
        }
    }
    assert!(h.wait().success());
    assert!(progress < 4000, "native proxy did not coalesce progress");
    assert_eq!(
        fs::metadata(&h.config.log_path).unwrap().len(),
        inference_supervisor::DISK_LOG_BYTES as u64
    );
}

#[test]
fn existing_execution_and_inherited_lease_prevent_new_compute() {
    let mut first = Harness::new("lease");
    first.start();
    let pids = first.pids();
    let mut second = Harness::with_mode("normal", Some(first.config.execution_lock.clone()));
    assert!(!second.wait().success());
    assert!(!second.root.path().join("pids.json").exists());
    first.child.kill().unwrap();
    first.child.wait().unwrap();
    let mut third = Harness::with_mode("normal", Some(first.config.execution_lock.clone()));
    assert!(!third.wait().success());
    // These PIDs came from this live fixture invocation, never from persisted recovery metadata.
    unsafe {
        libc::kill(-(pids["worker"].as_u64().unwrap() as i32), libc::SIGKILL);
    }
    gone(pids["worker"].as_u64().unwrap() as u32);
    gone(pids["descendant"].as_u64().unwrap() as u32);
    let mut next = Harness::with_mode("normal", Some(first.config.execution_lock.clone()));
    next.start();
    assert!(matches!(
        next.terminal().message,
        Message::OutputCandidate { .. }
    ));
    assert!(next.wait().success());
}
