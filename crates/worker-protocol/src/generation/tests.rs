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

fn v2_expected() -> WorkerIdentity {
    WorkerIdentity {
        build_id: "studio-onnx-worker-0.1.0".into(),
        runtime_lock_id: "onnxruntime-1.31.1-cpu".into(),
        model_snapshot_id: "birefnet-epoch-244".into(),
    }
}

fn v2_context() -> ExecutionContext {
    ExecutionContext {
        job_id: "job:v2".into(),
        attempt_id: "attempt:v2".into(),
        execution_token: "token:v2".into(),
    }
}

fn v2_request() -> ExecuteRequestV2 {
    ExecuteRequestV2 {
        model_id: "birefnet-general-epoch-244".into(),
        capability_id: "image.remove-background.birefnet-general-epoch-244.v1".into(),
        recipe_id: "birefnet-general-cpu-v1".into(),
        recipe_version: 1,
        runtime_lock_id: "onnxruntime-1.31.1-cpu".into(),
        model_snapshot_id: "birefnet-epoch-244".into(),
        instruction: String::new(),
        seed: None,
        sampling_steps: None,
        parameters: BTreeMap::from([
            ("provider".into(), serde_json::json!("CPUExecutionProvider")),
            ("precision".into(), serde_json::json!("float32")),
        ]),
        inputs: vec![InputAssetDescriptor {
            role: "source".into(),
            path: "/managed/runs/v2/input.png".into(),
            width: 10,
            height: 12,
            sha256: "a".repeat(64),
        }],
        outputs: vec![
            OutputSpec {
                role: "primary".into(),
                data_type: "image_rgba".into(),
                width: 10,
                height: 12,
                max_bytes: MAX_OUTPUT_BYTES,
            },
            OutputSpec {
                role: "mask".into(),
                data_type: "mask".into(),
                width: 10,
                height: 12,
                max_bytes: MAX_OUTPUT_BYTES,
            },
        ],
        model_path: "/managed/models/birefnet".into(),
        output_dir: "/managed/runs/v2".into(),
    }
}

#[allow(clippy::too_many_arguments)]
fn text_v2_request(
    model_id: &str,
    capability_id: &str,
    recipe_id: &str,
    snapshot_id: &str,
    steps: u32,
    guidance: f64,
    scheduler: &str,
    token_budget: u32,
) -> ExecuteRequestV2 {
    let mut parameters = BTreeMap::from([
        ("seed".into(), serde_json::json!(42)),
        ("steps".into(), serde_json::json!(steps)),
        ("quantization".into(), serde_json::json!(8)),
        ("guidance".into(), serde_json::json!(guidance)),
        ("scheduler".into(), serde_json::json!(scheduler)),
        ("recipe".into(), serde_json::json!(recipe_id)),
        ("recipe_version".into(), serde_json::json!(1)),
        ("token_budget".into(), serde_json::json!(token_budget)),
        ("truncation".into(), serde_json::json!(false)),
        (
            "quantized_components".into(),
            serde_json::json!(["transformer"]),
        ),
        (
            "text_encoder_precision".into(),
            serde_json::json!("bfloat16"),
        ),
        ("vae_precision".into(), serde_json::json!("float32")),
    ]);
    if recipe_id == "qwen21-turbo-txt2img-q8-8-v1" {
        parameters.insert(
            "sampling_schedule".into(),
            serde_json::json!("qwen-turbo-model-index-v1"),
        );
    }
    if recipe_id == "moss-image-sft-txt2img-q8-50-v1" {
        parameters.insert("thinking".into(), serde_json::json!(true));
        parameters.insert("negative_prompt".into(), serde_json::json!(""));
        parameters.insert("uncond_skip_layers".into(), serde_json::json!([22]));
        parameters.insert(
            "quantized_components".into(),
            serde_json::json!(["transformer", "text_encoder"]),
        );
        parameters.insert("text_encoder_precision".into(), serde_json::json!("q8"));
        parameters.insert("vae_precision".into(), serde_json::json!("bfloat16"));
    }
    ExecuteRequestV2 {
        model_id: model_id.into(),
        capability_id: capability_id.into(),
        recipe_id: recipe_id.into(),
        recipe_version: 1,
        runtime_lock_id: "recipes-v3".into(),
        model_snapshot_id: snapshot_id.into(),
        instruction: "a cobalt cube".into(),
        seed: Some(42),
        sampling_steps: Some(steps),
        parameters,
        inputs: vec![],
        outputs: vec![OutputSpec {
            role: "primary".into(),
            data_type: "image_rgb".into(),
            width: 1024,
            height: 1024,
            max_bytes: MAX_OUTPUT_BYTES,
        }],
        model_path: format!("/managed/models/{snapshot_id}"),
        output_dir: "/managed/runs/text-v3".into(),
    }
}

fn v2_envelope(sequence: u64, request_id: &str, message: Message) -> Envelope {
    Envelope {
        family: FAMILY.into(),
        version: Some(PROTOCOL_VERSION_V2),
        session_id: "session:v2".into(),
        request_id: request_id.into(),
        sequence,
        execution: Some(v2_context()),
        message,
    }
}

#[test]
fn v2_handshake_and_two_role_candidates_are_strict() {
    let expected = v2_expected();
    let capability = "image.remove-background.birefnet-general-epoch-244.v1";
    let mut session =
        HostSession::new_v2("session:v2", expected.clone(), vec![capability.into()]).unwrap();
    let hello = session.hello("request:hello", "studio-host").unwrap();
    let accepted = Envelope {
        family: FAMILY.into(),
        version: Some(PROTOCOL_VERSION_V2),
        session_id: "session:v2".into(),
        request_id: "request:hello".into(),
        sequence: 0,
        execution: None,
        message: Message::HelloAccepted {
            versions: VERSIONS_V2,
            selected_version: PROTOCOL_VERSION_V2,
            identity: expected,
            capabilities: vec![capability.into()],
        },
    };
    assert_eq!(hello.sequence, 0);
    assert_eq!(
        session.accept(&accepted).unwrap(),
        WorkerEvent::HandshakeAccepted
    );
    let Message::Execute {
        request: v1_request,
    } = host_frames()[1].message.clone()
    else {
        unreachable!()
    };
    assert!(
        session
            .execute("request:v1", v2_context(), v1_request)
            .is_err()
    );
    session
        .execute_v2("request:execute", v2_context(), v2_request())
        .unwrap();

    let primary = v2_envelope(
        1,
        "request:execute",
        Message::OutputCandidateV2 {
            candidate: OutputCandidateV2 {
                role: "primary".into(),
                data_type: "image_rgba".into(),
                relative_path: "primary.png".into(),
                width: 10,
                height: 12,
                size_bytes: 128,
                sha256: "b".repeat(64),
            },
        },
    );
    assert_eq!(
        session.accept(&primary).unwrap(),
        WorkerEvent::OutputCandidate
    );
    let mask = v2_envelope(
        2,
        "request:execute",
        Message::OutputCandidateV2 {
            candidate: OutputCandidateV2 {
                role: "mask".into(),
                data_type: "mask".into(),
                relative_path: "mask.png".into(),
                width: 10,
                height: 12,
                size_bytes: 64,
                sha256: "c".repeat(64),
            },
        },
    );
    assert_eq!(session.accept(&mask).unwrap(), WorkerEvent::OutputCandidate);
    let mut duplicate = mask.clone();
    duplicate.sequence = 3;
    assert!(session.accept(&duplicate).is_err());
}

#[test]
fn v2_rejects_fake_tool_fields_path_escape_and_bad_roles() {
    let mut request = v2_request();
    request.instruction = "pretend prompt".into();
    assert!(request.validate().is_err());
    request = v2_request();
    request.seed = Some(7);
    assert!(request.validate().is_err());
    request = v2_request();
    request.inputs[0].path = "/managed/runs/other/input.png".into();
    assert!(request.validate().is_err());
    request = v2_request();
    request.outputs[1].width = 11;
    assert!(request.validate().is_err());
    request = v2_request();
    request.outputs.pop();
    assert!(request.validate().is_err());
}

#[test]
fn qwen_and_moss_v2_payloads_require_exact_recipe_identity_and_parameters() {
    let mut requests = vec![
        text_v2_request(
            "qwen-image-2.1-mflux-v1",
            "image.text-to-image.qwen-image-2.1.v1",
            "qwen21-txt2img-q8-40-v1",
            "qwen-image-2.1-d26bb61231c349cf6b7896fa83353113880e1ba3",
            40,
            1.0,
            "linear",
            2048,
        ),
        text_v2_request(
            "qwen-image-2.1-turbo-mflux-v1",
            "image.text-to-image.qwen-image-2.1-turbo.v1",
            "qwen21-turbo-txt2img-q8-8-v1",
            "qwen-image-2.1-turbo-fc13ba1986fcb905854dc8de2f52eee395f49b5b",
            8,
            1.0,
            "linear",
            2048,
        ),
        text_v2_request(
            "moss-image-sft-mflux-v1",
            "image.text-to-image.moss-image-sft.v1",
            "moss-image-sft-txt2img-q8-50-v1",
            "moss-image-sft-f972bd4b4b9e3852133cc5da954d87bc32cd8fb9",
            50,
            3.0,
            "moss_image",
            4096,
        ),
    ];
    for request in &requests {
        request.validate().unwrap();
    }

    let mut wrong = requests.remove(0);
    wrong.capability_id = "image.text-to-image.qwen-image-2.1-turbo.v1".into();
    assert!(wrong.validate().is_err());
    wrong = requests.remove(0);
    wrong
        .parameters
        .insert("steps".into(), serde_json::json!(9));
    assert!(wrong.validate().is_err());
    wrong = requests.remove(0);
    wrong.inputs.push(InputAssetDescriptor {
        role: "source".into(),
        path: "/managed/runs/text-v3/input.png".into(),
        width: 1024,
        height: 1024,
        sha256: "a".repeat(64),
    });
    assert!(wrong.validate().is_err());
}

#[test]
fn v1_and_v2_messages_cannot_cross_protocol_boundaries() {
    let mut envelope = host_frames()[1].clone();
    envelope.version = Some(PROTOCOL_VERSION_V2);
    assert!(envelope.validate().is_err());
    let mut v2 = v2_envelope(
        1,
        "request:execute",
        Message::ExecuteV2 {
            request: v2_request(),
        },
    );
    assert!(v2.validate().is_ok());
    v2.version = Some(PROTOCOL_VERSION);
    assert!(v2.validate().is_err());
}
