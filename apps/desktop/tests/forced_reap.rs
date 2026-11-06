//! Windows-only: a supervisor that ignores cancellation must be force-reaped.
//!
//! The Unix unit fixture cannot express this (it uses shell scripts), so a tiny `stub-supervisor`
//! binary is spawned through the real controller: it never speaks the protocol, never reads stdin
//! and never exits, so only the cancel deadline can end the exchange.

#![cfg(windows)]

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use image_studio_desktop::adapters::{
    generation::GenerationController, installation::ReadyPaths, library::Library, paths::AppPaths,
};
use studio_core::{
    contracts::{ComponentSnapshot, DeviceSnapshot, RuntimeSnapshot, SourceRevision},
    domain::{ContentHash, IdempotencyKey, PlanId, ProjectId, RequestId},
    generation::{AttemptState, GenerationInput, PlanEnvironment, freeze_z_image},
};

fn fixture_environment() -> PlanEnvironment {
    PlanEnvironment {
        components: vec![ComponentSnapshot {
            role: "transformer".into(),
            source: SourceRevision {
                source: "fixture".into(),
                revision: "fixture".into(),
            },
            content_hash: ContentHash::new("a".repeat(64)).unwrap(),
        }],
        runtime: RuntimeSnapshot {
            package_id: "fixture".into(),
            engine: "fixture".into(),
            source: SourceRevision {
                source: "fixture".into(),
                revision: "fixture".into(),
            },
        },
        device: DeviceSnapshot {
            device_id: "fixture".into(),
            precision: "fixture".into(),
        },
        scheduler: "fixture".into(),
    }
}

fn ready_paths() -> ReadyPaths {
    ReadyPaths {
        worker_program: PathBuf::from(r"C:\fixture\python.exe"),
        worker_args: vec!["-I".into(), "-m".into(), "studio_mflux_worker".into()],
        model_path: PathBuf::from(r"C:\fixture\verified-model"),
        worker_module: "studio_mflux_worker".into(),
        worker_build_id: "fixture-worker".into(),
        runtime_lock_id: "fixture-runtime".into(),
        model_snapshot_id: "fixture-model".into(),
    }
}

#[test]
fn a_supervisor_that_ignores_cancel_is_force_reaped() {
    let root = tempfile::Builder::new()
        .prefix("forced reap ")
        .tempdir()
        .unwrap();
    let paths = AppPaths::resolve(Some(root.path())).unwrap();
    let mut library =
        Library::open(&paths.data, vec![paths.cache.clone(), paths.logs.clone()]).unwrap();
    let frozen = freeze_z_image(
        GenerationInput {
            request_id: RequestId::new("request-forced-reap").unwrap(),
            project_id: ProjectId::new("default").unwrap(),
            idempotency_key: IdempotencyKey::new("intent-forced-reap").unwrap(),
            plan_id: PlanId::new("plan-forced-reap").unwrap(),
            prompt: "强制回收夹具，不是真实模型生成".into(),
            width: 1024,
            height: 768,
            count: 1,
            seed: Some(7),
        },
        7,
        fixture_environment(),
    )
    .unwrap();
    let job = library.submit(frozen.request, frozen.plan).unwrap().job;
    let library = Arc::new(Mutex::new(library));
    let controller = GenerationController::new(
        paths,
        library.clone(),
        PathBuf::from(env!("CARGO_BIN_EXE_stub-supervisor")),
    );

    let marker = root.path().join("stub-started");
    // SAFETY: this test owns the process for its duration and no other thread reads the environment.
    unsafe {
        std::env::set_var("STUB_SUPERVISOR_MARKER", &marker);
    }
    controller.start(job.clone(), ready_paths()).unwrap();
    let spawned = Instant::now();
    while !marker.exists() && spawned.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        marker.exists(),
        "the stub supervisor was never spawned, so the deadline path was not exercised"
    );

    // The stub ignores stdin closure and the cancel message; only the deadline can end this.
    let cancelled = Instant::now();
    controller.cancel(&job.job_id).unwrap();
    let deadline = Instant::now() + Duration::from_secs(25);
    while controller.is_busy() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    let settle = cancelled.elapsed();
    assert!(
        !controller.is_busy(),
        "the controller stayed busy long after the cancel deadline"
    );
    // Z-Image's default timeouts are a 3 s grace plus a 5 s cleanup deadline; settling sooner would
    // mean the stub exited by itself rather than being force-reaped.
    assert!(
        settle >= Duration::from_secs(6),
        "settled in {settle:?}, which is too early to be the forced path"
    );

    let record = library.lock().unwrap().job(&job.job_id).unwrap();
    assert!(
        matches!(record.state, AttemptState::Cancelled | AttemptState::Failed),
        "unexpected terminal state: {:?}",
        record.state
    );
    // The marker process must be gone: a live stub would still hold the execution lease.
    let child_still_running = std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq stub-supervisor.exe", "/NH"])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).contains("stub-supervisor.exe"))
        .unwrap_or(false);
    assert!(
        !child_still_running,
        "the stub supervisor outlived the reap"
    );
}
