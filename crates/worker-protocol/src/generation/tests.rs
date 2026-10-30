use super::*;

fn host_frames() -> Vec<Envelope> {
    include_str!("../../fixtures/generation/host.jsonl")
        .lines()
        .map(|l| Envelope::decode_frame(l.as_bytes()).unwrap())
        .collect()
}
fn worker_frames() -> Vec<Envelope> {
    include_str!("../../fixtures/generation/worker.jsonl")
        .lines()
        .map(|l| Envelope::decode_frame(l.as_bytes()).unwrap())
        .collect()
}
fn session() -> HostSession {
    let frames = host_frames();
    let Message::Hello {
        expected_worker, ..
    } = &frames[0].message
    else {
        unreachable!()
    };
    HostSession::new(frames[0].session_id.clone(), expected_worker.clone()).unwrap()
}
fn executing() -> HostSession {
    let mut session = session();
    let host = host_frames();
    let worker = worker_frames();
    session.observe_outbound(&host[0]).unwrap();
    session.accept(&worker[0]).unwrap();
    session.observe_outbound(&host[1]).unwrap();
    session
}

#[test]
fn golden_exchange_is_strict_and_round_trips() {
    let mut session = session();
    let host = host_frames();
    let worker = worker_frames();
    for frame in host.iter().chain(&worker) {
        assert_eq!(
            *frame,
            Envelope::decode_frame(&frame.encode_line().unwrap()).unwrap()
        );
    }
    session.observe_outbound(&host[0]).unwrap();
    assert_eq!(
        session.accept(&worker[0]).unwrap(),
        WorkerEvent::HandshakeAccepted
    );
    session.observe_outbound(&host[1]).unwrap();
    assert_eq!(session.accept(&worker[1]).unwrap(), WorkerEvent::Progress);
    assert_eq!(session.accept(&worker[2]).unwrap(), WorkerEvent::Progress);
    assert_eq!(
        session.accept(&worker[3]).unwrap(),
        WorkerEvent::OutputCandidate
    );
    // Cancellation is still legal after a candidate. Durable success has not happened here.
    session.observe_outbound(&host[2]).unwrap();
    assert_eq!(session.accept(&worker[4]).unwrap(), WorkerEvent::Cancelled);
    assert!(session.accept(&worker[4]).is_err());
}

#[test]
fn stale_identity_and_duplicate_rejection_do_not_advance_state() {
    for mutation in 0..5 {
        let mut session = executing();
        let worker = worker_frames();
        let mut stale = worker[1].clone();
        match mutation {
            0 => stale.session_id = "old".into(),
            1 => stale.request_id = "old".into(),
            2 => stale.execution.as_mut().unwrap().attempt_id = "old".into(),
            3 => stale.execution.as_mut().unwrap().execution_token = "old".into(),
            _ => stale.sequence = 0,
        }
        assert!(session.accept(&stale).unwrap_err().is_ignorable_rejection());
        assert_eq!(session.accept(&worker[1]).unwrap(), WorkerEvent::Progress);
        assert!(matches!(session.accept(&worker[1]), Err(Error::Sequence)));
    }
}

#[test]
fn highest_common_version_identity_and_family_are_required() {
    let host = host_frames();
    let worker = worker_frames();
    for mutation in 0..4 {
        let mut session = session();
        session.observe_outbound(&host[0]).unwrap();
        let mut response = worker[0].clone();
        match &mut response.message {
            Message::HelloAccepted {
                identity,
                versions,
                capabilities,
                ..
            } => match mutation {
                0 => identity.runtime_lock_id = "wrong-runtime".into(),
                1 => identity.model_snapshot_id = "wrong-model".into(),
                2 => *versions = VersionRange { min: 2, max: 3 },
                _ => capabilities.push("diagnostic.probe.v1".into()),
            },
            _ => unreachable!(),
        }
        assert!(session.accept(&response).is_err());
        assert!(!session.is_handshaken());
        assert_eq!(
            session.accept(&worker[0]).unwrap(),
            WorkerEvent::HandshakeAccepted
        );
    }
    let mut wrong = worker[0].clone();
    wrong.family = "image-studio.diagnostic".into();
    assert!(matches!(wrong.validate(), Err(Error::WrongFamily)));
    let diagnostic = crate::Envelope::new(None, "session", "request", 0, crate::Message::Heartbeat);
    assert!(Envelope::decode_frame(&diagnostic.encode_line().unwrap()).is_err());
    assert!(crate::Envelope::decode_frame(&worker[0].encode_line().unwrap()).is_err());
}

#[test]
fn codec_rejects_unknown_fields_truncated_and_oversize_before_decode() {
    let mut value = serde_json::to_value(&host_frames()[0]).unwrap();
    value["unexpected"] = true.into();
    assert!(Envelope::decode_frame(&serde_json::to_vec(&value).unwrap()).is_err());
    let mut value = serde_json::to_value(&worker_frames()[1]).unwrap();
    value["message"]["unexpected"] = true.into();
    assert!(Envelope::decode_frame(&serde_json::to_vec(&value).unwrap()).is_err());
    assert!(matches!(
        read_frame(&mut &b"{\"incomplete\":"[..]),
        Err(Error::TruncatedFrame)
    ));
    let huge = vec![b'x'; MAX_FRAME_BYTES + 1];
    assert!(matches!(
        read_frame(&mut &huge[..]),
        Err(Error::FrameTooLarge)
    ));
    assert!(matches!(
        Envelope::decode_frame(&huge),
        Err(Error::FrameTooLarge)
    ));
}

#[test]
fn a_process_executes_only_once_and_candidate_matches_plan() {
    let mut session = executing();
    let host = host_frames();
    let worker = worker_frames();
    assert!(session.observe_outbound(&host[1]).is_err());
    let mut bad = worker[3].clone();
    if let Message::OutputCandidate { candidate } = &mut bad.message {
        candidate.width = 768;
    }
    assert!(session.accept(&bad).is_err());
    assert_eq!(
        session.accept(&worker[3]).unwrap(),
        WorkerEvent::OutputCandidate
    );
    assert!(session.accept(&worker[2]).is_err());
    let mut early_cancel = executing();
    assert!(early_cancel.accept(&worker[4]).is_err());
}

#[test]
fn recipe_and_progress_cannot_silently_downgrade() {
    let Message::Execute { mut request } = host_frames()[1].message.clone() else {
        unreachable!()
    };
    request.num_inference_steps = 4;
    assert!(request.validate().is_err());
    request.num_inference_steps = 9;
    request.width = 1000;
    assert!(request.validate().is_err());
    request.width = 1024;
    request.model_path = "Tongyi-MAI/Z-Image-Turbo".into();
    assert!(request.validate().is_err());
    let mut progress = worker_frames()[2].clone();
    if let Message::Progress { total, .. } = &mut progress.message {
        *total = Some(4);
    }
    assert!(progress.validate().is_err());
    let mut candidate = worker_frames()[3].clone();
    if let Message::OutputCandidate { candidate } = &mut candidate.message {
        candidate.relative_path = "../result.png".into();
    }
    assert!(candidate.validate().is_err());
}
