use std::io::{BufReader, Cursor};

use super::*;

fn accepted(sequence: u64) -> Envelope {
    Envelope::new(
        Some(1),
        "session-1",
        "hello-1",
        sequence,
        Message::HelloAccepted {
            versions: VersionRange::CURRENT,
            selected_version: 1,
            build_id: "fixture-worker".into(),
            capabilities: diagnostic_capabilities(),
        },
    )
}

fn ready() -> HostSession {
    let mut host = HostSession::new("session-1", VersionRange::CURRENT).unwrap();
    let hello = host.hello("hello-1", "fixture-host").unwrap();
    assert_eq!(hello.sequence, 0);
    assert_eq!(hello.version, None);
    assert_eq!(
        host.accept(&accepted(0)).unwrap(),
        HostEvent::HandshakeAccepted
    );
    host
}

fn result(sequence: u64) -> Envelope {
    Envelope::new(
        Some(1),
        "session-1",
        "probe-1",
        sequence,
        Message::ProbeResult {
            challenge: "challenge-1".into(),
            build_id: "fixture-worker".into(),
            scope: DIAGNOSTIC_SCOPE.into(),
            child_pid: None,
        },
    )
}

#[test]
fn selects_highest_common_and_rejects_invalid_ranges() {
    let a = VersionRange::new(1, 3).unwrap();
    let b = VersionRange::new(2, 4).unwrap();
    assert_eq!(a.highest_common(b), Some(3));
    assert_eq!(a.highest_common(VersionRange::new(4, 5).unwrap()), None);
    assert!(VersionRange::new(0, 1).is_err());
    assert!(VersionRange::new(2, 1).is_err());
}

#[test]
fn initial_handshake_and_sequences_continue_without_reset() {
    let mut host = ready();
    let probe = host.probe("probe-1", "challenge-1").unwrap();
    assert_eq!(probe.version, Some(1));
    assert_eq!(probe.sequence, 1);
    assert_eq!(host.accept(&result(1)).unwrap(), HostEvent::ProbeCompleted);
    assert_eq!(host.shutdown("shutdown-1").unwrap().sequence, 2);
    let ack = Envelope::new(Some(1), "session-1", "shutdown-1", 2, Message::ShutdownAck);
    assert_eq!(host.accept(&ack).unwrap(), HostEvent::ShutdownAcknowledged);
    assert!(!host.is_handshaken());
}

#[test]
fn cannot_send_probe_before_handshake() {
    let mut host = HostSession::new("session-1", VersionRange::CURRENT).unwrap();
    assert!(matches!(
        host.probe("probe-1", "challenge-1"),
        Err(ProtocolError::UnexpectedMessage)
    ));
    assert!(matches!(
        host.accept(&accepted(0)),
        Err(ProtocolError::WrongRequest)
    ));
}

#[test]
fn first_worker_sequence_must_be_zero() {
    let mut host = HostSession::new("session-1", VersionRange::CURRENT).unwrap();
    host.hello("hello-1", "host").unwrap();
    assert!(matches!(
        host.accept(&accepted(1)),
        Err(ProtocolError::Sequence { .. })
    ));
    assert!(host.accept(&accepted(0)).is_ok());
}

#[test]
fn handshake_must_select_highest_common_version() {
    let mut host = HostSession::new("session-1", VersionRange::new(1, 2).unwrap()).unwrap();
    host.hello("hello-1", "host").unwrap();
    let mut reply = accepted(0);
    if let Message::HelloAccepted { versions, .. } = &mut reply.message {
        *versions = VersionRange::new(1, 2).unwrap();
    }
    assert!(matches!(
        host.accept(&reply),
        Err(ProtocolError::VersionMismatch)
    ));
}

#[test]
fn incompatible_version_is_distinct_from_malformed_json() {
    let mut host = HostSession::new("session-1", VersionRange::CURRENT).unwrap();
    host.hello("hello-1", "host").unwrap();
    let mut reply = accepted(0);
    reply.version = Some(2);
    if let Message::HelloAccepted {
        versions,
        selected_version,
        ..
    } = &mut reply.message
    {
        *versions = VersionRange::new(2, 2).unwrap();
        *selected_version = 2;
    }
    let error = host.accept(&reply).unwrap_err();
    assert_eq!(error.code(), "PROTOCOL_VERSION_MISMATCH");
    assert!(!error.is_ignorable_rejection());
    assert_eq!(
        Envelope::decode_frame(b"ordinary log").unwrap_err().code(),
        "MALFORMED_JSON"
    );
}

#[test]
fn worker_cannot_declare_generation_capability() {
    let mut host = HostSession::new("session-1", VersionRange::CURRENT).unwrap();
    host.hello("hello-1", "host").unwrap();
    let mut reply = accepted(0);
    if let Message::HelloAccepted { capabilities, .. } = &mut reply.message {
        capabilities.push("image.generate.v1".into());
    }
    assert!(matches!(
        host.accept(&reply),
        Err(ProtocolError::CapabilityMismatch)
    ));
    assert!(!host.is_handshaken());
}

#[test]
fn rejected_old_session_does_not_advance_sequence() {
    let mut host = ready();
    host.probe("probe-1", "challenge-1").unwrap();
    let mut old = result(1);
    old.session_id = "old-session".into();
    let error = host.accept(&old).unwrap_err();
    assert!(error.is_ignorable_rejection());
    assert_eq!(error.code(), "WRONG_SESSION");
    assert_eq!(host.accept(&result(1)).unwrap(), HostEvent::ProbeCompleted);
}

#[test]
fn response_to_wrong_or_completed_request_is_rejected() {
    let mut host = ready();
    host.probe("probe-1", "challenge-1").unwrap();
    let mut wrong = result(1);
    wrong.request_id = "another-request".into();
    assert!(matches!(
        host.accept(&wrong),
        Err(ProtocolError::WrongRequest)
    ));
    host.accept(&result(1)).unwrap();
    assert!(matches!(
        host.accept(&result(2)),
        Err(ProtocolError::WrongRequest)
    ));
}

#[test]
fn duplicate_progress_does_not_prevent_valid_result() {
    let mut host = ready();
    host.probe("probe-1", "challenge-1").unwrap();
    let progress = Envelope::new(
        Some(1),
        "session-1",
        "probe-1",
        1,
        Message::Progress {
            completed: 1,
            total: 2,
        },
    );
    assert_eq!(host.accept(&progress).unwrap(), HostEvent::Progress);
    assert!(matches!(
        host.accept(&progress),
        Err(ProtocolError::Sequence { .. })
    ));
    assert_eq!(host.accept(&result(2)).unwrap(), HostEvent::ProbeCompleted);
}

#[test]
fn shutdown_invalidates_in_flight_probe_and_accepts_ack() {
    let mut host = ready();
    host.probe("probe-1", "challenge-1").unwrap();
    host.shutdown("shutdown-1").unwrap();
    assert!(matches!(
        host.accept(&result(1)),
        Err(ProtocolError::WrongRequest)
    ));
    let ack = Envelope::new(Some(1), "session-1", "shutdown-1", 1, Message::ShutdownAck);
    assert_eq!(host.accept(&ack).unwrap(), HostEvent::ShutdownAcknowledged);
}

#[test]
fn challenge_scope_and_execution_are_checked_before_state_changes() {
    let mut host = ready();
    host.probe("probe-1", "challenge-1").unwrap();
    let mut wrong = result(1);
    if let Message::ProbeResult { challenge, .. } = &mut wrong.message {
        *challenge = "another-challenge".into();
    }
    assert!(matches!(
        host.accept(&wrong),
        Err(ProtocolError::ChallengeMismatch)
    ));
    let mut wrong = result(1);
    if let Message::ProbeResult { scope, .. } = &mut wrong.message {
        *scope = "image_generation".into();
    }
    assert!(matches!(
        host.accept(&wrong),
        Err(ProtocolError::ScopeMismatch)
    ));
    let mut wrong = result(1);
    wrong.execution = Some(ExecutionContext {
        attempt_id: "old-attempt".into(),
        fencing_token: "old-token".into(),
    });
    assert!(host.accept(&wrong).unwrap_err().is_ignorable_rejection());
    assert_eq!(host.accept(&result(1)).unwrap(), HostEvent::ProbeCompleted);
}

#[test]
fn later_message_must_use_selected_version() {
    let mut host = ready();
    host.probe("probe-1", "challenge-1").unwrap();
    let mut reply = result(1);
    reply.version = None;
    assert!(matches!(
        host.accept(&reply),
        Err(ProtocolError::VersionMismatch)
    ));
}

#[test]
fn bootstrap_error_does_not_require_a_selected_version() {
    let mut host = HostSession::new("session-1", VersionRange::CURRENT).unwrap();
    host.hello("hello-1", "host").unwrap();
    let error = Envelope::new(
        None,
        "session-1",
        "hello-1",
        0,
        Message::ProtocolError {
            code: "NO_COMMON_VERSION".into(),
            summary: "no common diagnostic protocol".into(),
            versions: Some(VersionRange { min: 2, max: 2 }),
        },
    );
    assert!(matches!(
        host.accept(&error),
        Err(ProtocolError::Remote { .. })
    ));
    assert_eq!(host.selected_version(), None);
}

#[test]
fn remote_error_for_unknown_request_does_not_terminate_current_probe() {
    let mut host = ready();
    host.probe("probe-1", "challenge-1").unwrap();
    let error = Envelope::new(
        Some(1),
        "session-1",
        "old-request",
        1,
        Message::ProtocolError {
            code: "OLD_ERROR".into(),
            summary: "an old request failed".into(),
            versions: None,
        },
    );
    assert!(matches!(
        host.accept(&error),
        Err(ProtocolError::WrongRequest)
    ));
    assert_eq!(host.accept(&result(1)).unwrap(), HostEvent::ProbeCompleted);
}

#[test]
fn changed_build_identity_cannot_become_a_successful_probe() {
    let mut host = ready();
    host.probe("probe-1", "challenge-1").unwrap();
    let mut changed = result(1);
    if let Message::ProbeResult { build_id, .. } = &mut changed.message {
        *build_id = "different-worker".into();
    }
    assert!(matches!(
        host.accept(&changed),
        Err(ProtocolError::InvalidEnvelope(_))
    ));
    assert_eq!(host.accept(&result(1)).unwrap(), HostEvent::ProbeCompleted);
}

#[test]
fn invalid_progress_cannot_advance_a_pending_probe() {
    let mut host = ready();
    host.probe("probe-1", "challenge-1").unwrap();
    let progress = Envelope::new(
        Some(1),
        "session-1",
        "probe-1",
        1,
        Message::Progress {
            completed: 1,
            total: 0,
        },
    );
    assert!(matches!(
        host.accept(&progress),
        Err(ProtocolError::InvalidEnvelope(_))
    ));
    assert_eq!(host.accept(&result(1)).unwrap(), HostEvent::ProbeCompleted);
}

#[test]
fn unknown_fields_and_generation_message_types_are_not_accepted() {
    let line = result(1).encode_line().unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&line).unwrap();
    value["unexpected"] = 1.into();
    assert!(Envelope::decode_frame(&serde_json::to_vec(&value).unwrap()).is_err());
    value.as_object_mut().unwrap().remove("unexpected");
    value["message"]["unexpected"] = 1.into();
    assert!(Envelope::decode_frame(&serde_json::to_vec(&value).unwrap()).is_err());
    value["message"] = serde_json::json!({"type":"completed"});
    assert!(Envelope::decode_frame(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn incomplete_execution_context_is_not_silently_accepted() {
    let mut value: serde_json::Value =
        serde_json::from_slice(&result(1).encode_line().unwrap()).unwrap();
    value["execution"] = serde_json::json!({"attempt_id":"old-attempt"});
    assert!(Envelope::decode_frame(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn decoder_preserves_legal_message_and_requires_no_unbounded_line_read() {
    let envelope = result(1);
    let encoded = envelope.encode_line().unwrap();
    let mut reader = BufReader::with_capacity(7, Cursor::new(encoded));
    let frame = read_frame(&mut reader).unwrap().unwrap();
    assert_eq!(Envelope::decode_frame(&frame).unwrap(), envelope);
    assert!(read_frame(&mut reader).unwrap().is_none());
}

#[test]
fn frame_limit_is_checked_before_appending_unterminated_bytes() {
    let bytes = vec![b'x'; MAX_FRAME_BYTES + 1];
    let mut reader = BufReader::with_capacity(31, Cursor::new(bytes));
    assert!(matches!(
        read_frame(&mut reader),
        Err(ProtocolError::FrameTooLarge { .. })
    ));
}

#[test]
fn exact_limit_and_multiple_frames_have_correct_boundaries() {
    let mut bytes = vec![b'x'; MAX_FRAME_BYTES];
    bytes.extend_from_slice(b"\nsecond\n");
    let mut reader = BufReader::with_capacity(37, Cursor::new(bytes));
    assert_eq!(
        read_frame(&mut reader).unwrap().unwrap().len(),
        MAX_FRAME_BYTES
    );
    assert_eq!(read_frame(&mut reader).unwrap().unwrap(), b"second");
    assert!(read_frame(&mut reader).unwrap().is_none());
}

#[test]
fn eof_inside_a_frame_is_not_a_completed_message() {
    let mut reader = Cursor::new(b"{\"message\":");
    assert!(matches!(
        read_frame(&mut reader),
        Err(ProtocolError::TruncatedFrame)
    ));
}

#[test]
fn encoder_and_decoder_both_reject_oversized_frames() {
    let mut envelope = result(1);
    if let Message::ProbeResult { challenge, .. } = &mut envelope.message {
        *challenge = "x".repeat(MAX_FRAME_BYTES);
    }
    assert!(matches!(
        envelope.encode_line(),
        Err(ProtocolError::FrameTooLarge { .. })
    ));
    assert!(matches!(
        Envelope::decode_frame(&vec![b'x'; MAX_FRAME_BYTES + 1]),
        Err(ProtocolError::FrameTooLarge { .. })
    ));
}
