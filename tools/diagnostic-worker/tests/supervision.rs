use image_studio_desktop::adapters::diagnostics::{
    DiagnosticConfig, DiagnosticController, DiagnosticPhase, DiagnosticSnapshot,
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

fn start(mode: &str) -> DiagnosticController {
    let mut config = DiagnosticConfig::new(PathBuf::from(env!("CARGO_BIN_EXE_diagnostic-worker")));
    config.mode = mode.into();
    config.handshake_timeout = Duration::from_secs(2);
    config.probe_timeout = Duration::from_millis(500);
    config.shutdown_grace = Duration::from_millis(150);
    DiagnosticController::start(config, Arc::new(|| {}))
}

fn until(
    controller: &DiagnosticController,
    condition: impl Fn(&DiagnosticSnapshot) -> bool,
) -> DiagnosticSnapshot {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let snapshot = controller.snapshot();
        if condition(&snapshot) {
            return snapshot;
        }
        assert!(
            Instant::now() < deadline,
            "diagnostic did not settle: {snapshot:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn immediate_stop_always_reports_a_terminal_state() {
    for _ in 0..8 {
        let controller = start("normal");
        controller.stop();
        let done = until(&controller, |s| !s.is_running);
        assert_eq!(done.phase, DiagnosticPhase::Stopped);
        assert!(!done.probe_passed);
    }
}

#[test]
fn normal_probe_and_orderly_stop_use_real_process() {
    let controller = start("normal");
    let ready = until(&controller, |s| s.probe_passed);
    assert!(ready.is_running);
    assert_eq!(ready.phase, DiagnosticPhase::Ready);
    controller.stop();
    let done = until(&controller, |s| !s.is_running);
    assert_eq!(done.phase, DiagnosticPhase::Stopped);
    assert_eq!(done.error_code, None);
}

#[test]
fn malformed_versions_frames_crashes_and_timeouts_never_pass() {
    for mode in [
        "bad-version",
        "malformed",
        "oversize",
        "crash",
        "hang",
        "handshake-hang",
    ] {
        let controller = start(mode);
        let result = until(&controller, |s| !s.is_running);
        assert_eq!(result.phase, DiagnosticPhase::Failed, "{mode}: {result:?}");
        assert!(!result.probe_passed, "{mode}");
        assert!(result.error_code.is_some(), "{mode}");
    }
}

#[test]
fn rejected_events_do_not_prevent_later_correct_response() {
    for mode in ["duplicate", "wrong-session", "wrong-request", "wrong-token"] {
        let controller = start(mode);
        let ready = until(&controller, |s| s.probe_passed || !s.is_running);
        assert!(ready.probe_passed, "{mode}: {ready:?}");
        assert!(ready.logs.contains("拒绝消息"), "{mode}: {ready:?}");
        controller.stop();
        until(&controller, |s| !s.is_running);
    }
}

#[test]
fn ignore_shutdown_has_distinct_forced_termination_result() {
    let controller = start("ignore-shutdown");
    until(&controller, |s| s.probe_passed);
    controller.stop();
    let result = until(&controller, |s| !s.is_running);
    assert_eq!(result.phase, DiagnosticPhase::Stopped);
    assert_eq!(result.error_code.as_deref(), Some("FORCED_TERMINATION"));
}

#[test]
fn stop_interrupts_a_pending_probe() {
    let controller = start("hang");
    until(&controller, |s| s.phase == DiagnosticPhase::Probing);
    controller.stop();
    let result = until(&controller, |s| !s.is_running);
    assert_eq!(result.phase, DiagnosticPhase::Stopped);
    assert!(!result.probe_passed);
}

#[test]
fn progress_and_stderr_flood_remain_bounded_and_reach_terminal_state() {
    let controller = start("flood");
    let ready = until(&controller, |s| s.probe_passed || !s.is_running);
    assert!(ready.probe_passed, "{ready:?}");
    assert!(ready.progress_events > 64);
    assert!(ready.logs.len() <= 128 * 1024);
    controller.stop();
    let result = until(&controller, |s| !s.is_running);
    assert!(result.logs.len() <= 128 * 1024);
}

#[test]
fn unicode_space_and_shell_symbols_in_executable_and_workdir_are_literal() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("中文 空格 ; $(no-command)");
    std::fs::create_dir_all(&dir).unwrap();
    let program = dir.join("诊断 worker ; $name");
    std::fs::copy(env!("CARGO_BIN_EXE_diagnostic-worker"), &program).unwrap();
    let mut config = DiagnosticConfig::new(program);
    config.work_dir = Some(dir);
    let controller = DiagnosticController::start(config, Arc::new(|| {}));
    assert!(until(&controller, |s| s.probe_passed || !s.is_running).probe_passed);
    controller.stop();
    until(&controller, |s| !s.is_running);
}

#[cfg(unix)]
fn alive(pid: u32) -> bool {
    std::process::Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(unix)]
#[test]
fn graceful_parent_and_controller_drop_clean_descendants() {
    for explicit_stop in [true, false] {
        let controller = start("descendant");
        let ready = until(&controller, |s| s.probe_passed);
        let parent = ready.pid.unwrap();
        let descendant = ready.child_pid.unwrap();
        assert!(alive(parent));
        assert!(alive(descendant));
        if explicit_stop {
            controller.stop();
            until(&controller, |s| !s.is_running);
        }
        drop(controller);
        let deadline = Instant::now() + Duration::from_secs(3);
        while alive(parent) || alive(descendant) {
            assert!(
                Instant::now() < deadline,
                "managed processes survived cleanup"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
