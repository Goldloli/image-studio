//! Real subprocess checks for the diagnostic fixture itself.
//! Supervisor fault/cleanup coverage lives in supervision.rs.

use std::{
    io::Write,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use worker_protocol::{
    DIAGNOSTIC_SCOPE, Envelope, HostEvent, HostSession, Message, VersionRange, read_frame,
};

fn exchange(mode: &str, input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_diagnostic-worker"))
        .args(["--mode", mode])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("diagnostic fixture did not finish bounded exchange");
        }
        thread::sleep(Duration::from_millis(10));
    }
    child.wait_with_output().unwrap()
}

fn input_messages() -> Vec<u8> {
    let mut host = HostSession::new("fixture-session", VersionRange::CURRENT).unwrap();
    let mut bytes = host
        .hello("hello-1", "fixture-host")
        .unwrap()
        .encode_line()
        .unwrap();
    bytes.extend(
        Envelope::new(
            Some(1),
            "fixture-session",
            "probe-1",
            1,
            Message::Probe {
                challenge: "fixture-challenge".into(),
            },
        )
        .encode_line()
        .unwrap(),
    );
    bytes.extend(
        Envelope::new(
            Some(1),
            "fixture-session",
            "shutdown-1",
            2,
            Message::Shutdown,
        )
        .encode_line()
        .unwrap(),
    );
    bytes
}

#[test]
fn normal_process_completes_only_diagnostic_exchange() {
    let output = exchange("normal", &input_messages());
    assert!(output.status.success(), "{:?}", output);
    let mut reader = output.stdout.as_slice();
    let mut host = HostSession::new("fixture-session", VersionRange::CURRENT).unwrap();
    host.hello("hello-1", "fixture-host").unwrap();
    let accepted = Envelope::decode_frame(&read_frame(&mut reader).unwrap().unwrap()).unwrap();
    assert_eq!(
        host.accept(&accepted).unwrap(),
        HostEvent::HandshakeAccepted
    );
    host.probe("probe-1", "fixture-challenge").unwrap();
    let progress = Envelope::decode_frame(&read_frame(&mut reader).unwrap().unwrap()).unwrap();
    assert_eq!(host.accept(&progress).unwrap(), HostEvent::Progress);
    let result = Envelope::decode_frame(&read_frame(&mut reader).unwrap().unwrap()).unwrap();
    assert_eq!(host.accept(&result).unwrap(), HostEvent::ProbeCompleted);
    let Message::ProbeResult {
        scope, child_pid, ..
    } = result.message
    else {
        panic!("expected diagnostic result");
    };
    assert_eq!(scope, DIAGNOSTIC_SCOPE);
    assert_eq!(child_pid, None);
    host.shutdown("shutdown-1").unwrap();
    let ack = Envelope::decode_frame(&read_frame(&mut reader).unwrap().unwrap()).unwrap();
    assert_eq!(host.accept(&ack).unwrap(), HostEvent::ShutdownAcknowledged);
    assert!(read_frame(&mut reader).unwrap().is_none());
}

#[test]
fn unsupported_range_returns_bootstrap_error_without_selected_version() {
    let mut host = HostSession::new("fixture-session", VersionRange::new(2, 3).unwrap()).unwrap();
    let input = host
        .hello("hello-1", "fixture-host")
        .unwrap()
        .encode_line()
        .unwrap();
    let output = exchange("normal", &input);
    assert!(output.status.success());
    let mut reader = output.stdout.as_slice();
    let response = Envelope::decode_frame(&read_frame(&mut reader).unwrap().unwrap()).unwrap();
    assert_eq!(response.version, None);
    assert_eq!(response.sequence, 0);
    assert!(matches!(response.message, Message::ProtocolError { .. }));
}

#[test]
fn a_request_before_hello_is_rejected_without_success_output() {
    let input = Envelope::new(
        Some(1),
        "fixture-session",
        "probe-1",
        0,
        Message::Probe {
            challenge: "fixture".into(),
        },
    )
    .encode_line()
    .unwrap();
    let output = exchange("normal", &input);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("UNEXPECTED_MESSAGE")
    );
}

#[test]
fn eof_inside_a_control_frame_is_reported_on_stderr() {
    let output = exchange("normal", b"{");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("TRUNCATED_FRAME")
    );
}

#[test]
fn unknown_mode_fails_fast_without_starting_a_probe() {
    let output = exchange("not-a-mode", b"");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("unknown diagnostic mode")
    );
}
