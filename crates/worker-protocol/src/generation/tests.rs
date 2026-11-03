use super::*;

fn managed_path(relative: &str) -> String {
    #[cfg(windows)]
    {
        format!(r"C:\managed\{}", relative.replace('/', r"\"))
    }
    #[cfg(not(windows))]
    {
        format!("/managed/{relative}")
    }
}

fn localize_fixture_paths(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(path) => {
            if let Some(relative) = path.strip_prefix("/managed/") {
                *path = managed_path(relative);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                localize_fixture_paths(item);
            }
        }
        serde_json::Value::Object(fields) => {
            for value in fields.values_mut() {
                localize_fixture_paths(value);
            }
        }
        _ => {}
    }
}

fn decode_fixture(line: &str) -> Envelope {
    let mut value: serde_json::Value = serde_json::from_str(line).unwrap();
    localize_fixture_paths(&mut value);
    Envelope::decode_frame(serde_json::to_string(&value).unwrap().as_bytes()).unwrap()
}

fn host_frames() -> Vec<Envelope> {
    include_str!("../../fixtures/generation/host.jsonl")
        .lines()
        .map(decode_fixture)
        .collect()
}
fn worker_frames() -> Vec<Envelope> {
    include_str!("../../fixtures/generation/worker.jsonl")
        .lines()
        .map(decode_fixture)
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
            path: managed_path("runs/v2/input.png"),
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
        model_path: managed_path("models/birefnet"),
        output_dir: managed_path("runs/v2"),
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
        model_path: managed_path(&format!("models/{snapshot_id}")),
        output_dir: managed_path("runs/text-v3"),
    }
}

fn windows_v2_request() -> ExecuteRequestV2 {
    ExecuteRequestV2 {
        model_id: Z_IMAGE_WINDOWS_MODEL_ID.into(),
        capability_id: Z_IMAGE_WINDOWS_CAPABILITY_ID.into(),
        recipe_id: Z_IMAGE_WINDOWS_RECIPE_ID.into(),
        recipe_version: 1,
        runtime_lock_id: Z_IMAGE_WINDOWS_RUNTIME_ID.into(),
        model_snapshot_id: Z_IMAGE_WINDOWS_MODEL_ID.into(),
        instruction: "Windows 中文 prompt".into(),
        seed: Some(42),
        sampling_steps: Some(9),
        parameters: windows_z_image_parameters(42),
        inputs: vec![],
        outputs: vec![OutputSpec {
            role: "primary".into(),
            data_type: "image_rgb".into(),
            width: 1024,
            height: 1024,
            max_bytes: MAX_OUTPUT_BYTES,
        }],
        model_path: managed_path("models/windows-z-image"),
        output_dir: managed_path("runs/windows-z-image"),
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
    request.inputs[0].path = managed_path("runs/other/input.png");
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
        path: managed_path("runs/text-v3/input.png"),
        width: 1024,
        height: 1024,
        sha256: "a".repeat(64),
    });
    assert!(wrong.validate().is_err());
}

#[test]
fn windows_z_image_v2_payload_requires_frozen_identity_and_parameters() {
    let valid = windows_v2_request();
    valid.validate().unwrap();

    for mutation in 0..7 {
        let mut request = windows_v2_request();
        match mutation {
            0 => request.capability_id = CAPABILITY.into(),
            1 => request.recipe_id = "z-image-turbo-q4-v1".into(),
            2 => request.runtime_lock_id = "mflux-0.24.0".into(),
            3 => request.model_snapshot_id = "z-image-turbo-q4".into(),
            4 => {
                request
                    .parameters
                    .insert("truncation".into(), serde_json::json!(true));
            }
            5 => request.outputs[0].width = 768,
            _ => request.seed = None,
        }
        assert!(request.validate().is_err(), "mutation {mutation}");
    }
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

fn windows_map(entries: &[(&str, serde_json::Value)]) -> BTreeMap<String, serde_json::Value> {
    entries
        .iter()
        .map(|(key, value)| ((*key).into(), value.clone()))
        .collect()
}

fn windows_klein_v2_request(edit: bool) -> ExecuteRequestV2 {
    let seed = 42;
    let (capability_id, recipe_id, parameters) = if edit {
        (
            KLEIN_WINDOWS_EDIT_CAPABILITY_ID,
            KLEIN_WINDOWS_EDIT_RECIPE_ID,
            windows_klein_edit_parameters(seed),
        )
    } else {
        (
            KLEIN_WINDOWS_TEXT_CAPABILITY_ID,
            KLEIN_WINDOWS_TEXT_RECIPE_ID,
            windows_klein_parameters(seed),
        )
    };
    ExecuteRequestV2 {
        model_id: KLEIN_WINDOWS_MODEL_ID.into(),
        capability_id: capability_id.into(),
        recipe_id: recipe_id.into(),
        recipe_version: 1,
        runtime_lock_id: WINDOWS_V2_RUNTIME_ID.into(),
        model_snapshot_id: KLEIN_WINDOWS_SNAPSHOT_ID.into(),
        instruction: "a cobalt cube".into(),
        seed: Some(seed),
        sampling_steps: Some(4),
        parameters,
        inputs: if edit {
            vec![InputAssetDescriptor {
                role: "source".into(),
                path: managed_path("runs/windows-klein/input.png"),
                width: 512,
                height: 512,
                sha256: "a".repeat(64),
            }]
        } else {
            vec![]
        },
        outputs: vec![OutputSpec {
            role: "primary".into(),
            data_type: "image_rgb".into(),
            width: 1024,
            height: 1024,
            max_bytes: MAX_OUTPUT_BYTES,
        }],
        model_path: managed_path("models/windows-klein"),
        output_dir: managed_path("runs/windows-klein"),
    }
}

fn windows_birefnet_v2_request() -> ExecuteRequestV2 {
    ExecuteRequestV2 {
        model_id: BIREFNET_WINDOWS_MODEL_ID.into(),
        capability_id: BIREFNET_WINDOWS_CAPABILITY_ID.into(),
        recipe_id: BIREFNET_WINDOWS_RECIPE_ID.into(),
        recipe_version: 1,
        runtime_lock_id: WINDOWS_V2_RUNTIME_ID.into(),
        model_snapshot_id: BIREFNET_WINDOWS_SNAPSHOT_ID.into(),
        instruction: String::new(),
        seed: None,
        sampling_steps: None,
        parameters: windows_birefnet_parameters(),
        inputs: vec![InputAssetDescriptor {
            role: "source".into(),
            path: managed_path("runs/windows-birefnet/input.png"),
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
        model_path: managed_path("models/windows-birefnet"),
        output_dir: managed_path("runs/windows-birefnet"),
    }
}

fn windows_realesrgan_v2_request() -> ExecuteRequestV2 {
    ExecuteRequestV2 {
        model_id: REALESRGAN_WINDOWS_MODEL_ID.into(),
        capability_id: REALESRGAN_WINDOWS_CAPABILITY_ID.into(),
        recipe_id: REALESRGAN_WINDOWS_RECIPE_ID.into(),
        recipe_version: 1,
        runtime_lock_id: REALESRGAN_WINDOWS_RUNTIME_ID.into(),
        model_snapshot_id: REALESRGAN_WINDOWS_SNAPSHOT_ID.into(),
        instruction: String::new(),
        seed: None,
        sampling_steps: None,
        parameters: windows_realesrgan_parameters(),
        inputs: vec![InputAssetDescriptor {
            role: "source".into(),
            path: managed_path("runs/windows-realesrgan/input.png"),
            width: 10,
            height: 12,
            sha256: "a".repeat(64),
        }],
        outputs: vec![OutputSpec {
            role: "primary".into(),
            data_type: "image_rgb".into(),
            width: 40,
            height: 48,
            max_bytes: MAX_UPSCALE_OUTPUT_BYTES,
        }],
        model_path: managed_path("models/windows-realesrgan"),
        output_dir: managed_path("runs/windows-realesrgan"),
    }
}

fn windows_text_v2_request(
    model_id: &str,
    capability_id: &str,
    recipe_id: &str,
    snapshot_id: &str,
    parameters: BTreeMap<String, serde_json::Value>,
) -> ExecuteRequestV2 {
    let steps = parameters["steps"].as_u64().map(|value| value as u32);
    ExecuteRequestV2 {
        model_id: model_id.into(),
        capability_id: capability_id.into(),
        recipe_id: recipe_id.into(),
        recipe_version: 1,
        runtime_lock_id: WINDOWS_V2_RUNTIME_ID.into(),
        model_snapshot_id: snapshot_id.into(),
        instruction: "a cobalt cube".into(),
        seed: Some(42),
        sampling_steps: steps,
        parameters,
        inputs: vec![],
        outputs: vec![OutputSpec {
            role: "primary".into(),
            data_type: "image_rgb".into(),
            width: 1024,
            height: 1024,
            max_bytes: MAX_OUTPUT_BYTES,
        }],
        model_path: managed_path(&format!("models/{snapshot_id}")),
        output_dir: managed_path("runs/windows-text"),
    }
}

#[test]
fn windows_recipe_parameter_helpers_match_the_frozen_maps() {
    let seed = 42;
    assert_eq!(
        windows_klein_parameters(seed),
        windows_map(&[
            ("recipe", serde_json::json!("klein4b-text-windows-v1")),
            ("recipe_version", serde_json::json!(1)),
            ("seed", serde_json::json!(seed)),
            ("steps", serde_json::json!(4)),
            ("guidance", serde_json::json!(1.0)),
            ("scheduler", serde_json::json!("flow_match_euler_discrete")),
            ("token_budget", serde_json::json!(512)),
            ("thinking", serde_json::json!(false)),
            ("truncation", serde_json::json!(false)),
            ("quantization", serde_json::json!("nf4")),
            (
                "load_strategy",
                serde_json::json!("nf4-component-offload-v1")
            ),
        ])
    );
    assert_eq!(
        windows_klein_edit_parameters(seed)["recipe"],
        serde_json::json!("klein4b-edit-windows-v1")
    );
    assert_eq!(
        windows_birefnet_parameters()["postprocessing"],
        serde_json::json!("birefnet-logits-sigmoid-v1")
    );
    assert_eq!(
        windows_realesrgan_parameters(),
        windows_map(&[
            (
                "recipe",
                serde_json::json!("realesrgan-x4plus-windows-t128-v1")
            ),
            ("recipe_version", serde_json::json!(1)),
            ("model", serde_json::json!("realesrgan-x4plus")),
            ("scale", serde_json::json!(4)),
            ("tile", serde_json::json!(128)),
            ("threads", serde_json::json!("1:1:1")),
            ("tta", serde_json::json!(false)),
        ])
    );
    assert_eq!(
        windows_qwen21_parameters(seed),
        windows_map(&[
            (
                "recipe",
                serde_json::json!("qwen21-windows-txt2img-nf4-40-v1")
            ),
            ("recipe_version", serde_json::json!(1)),
            ("seed", serde_json::json!(seed)),
            ("steps", serde_json::json!(40)),
            ("guidance", serde_json::json!(1.0)),
            ("sampling", serde_json::json!("linear")),
            ("token_budget", serde_json::json!(2048)),
            ("quantization", serde_json::json!("nf4")),
            ("load_strategy", serde_json::json!("staged-nf4-v1")),
        ])
    );
    assert_eq!(
        windows_qwen21_turbo_parameters(seed)["sampling_schedule"],
        serde_json::json!("qwen-turbo-model-index-v1")
    );
    assert_eq!(windows_qwen21_turbo_parameters(seed)["steps"], 8);
    assert_eq!(
        windows_moss_parameters(seed),
        windows_map(&[
            (
                "recipe",
                serde_json::json!("moss-image-sft-windows-txt2img-nf4-50-v1")
            ),
            ("recipe_version", serde_json::json!(1)),
            ("seed", serde_json::json!(seed)),
            ("steps", serde_json::json!(50)),
            ("guidance", serde_json::json!(3.0)),
            ("sampling", serde_json::json!("moss_image")),
            ("token_budget", serde_json::json!(4096)),
            ("thinking", serde_json::json!(true)),
            ("negative_prompt", serde_json::json!("")),
            ("uncond_skip_layers", serde_json::json!([22])),
            (
                "quantized_components",
                serde_json::json!(["transformer", "text_encoder"])
            ),
            ("text_encoder_precision", serde_json::json!("nf4")),
            ("vae_precision", serde_json::json!("float32")),
            ("load_strategy", serde_json::json!("staged-nf4-v1")),
        ])
    );
}

#[test]
fn windows_v2_payloads_accept_the_frozen_identities() {
    for request in [
        windows_klein_v2_request(false),
        windows_klein_v2_request(true),
        windows_birefnet_v2_request(),
        windows_realesrgan_v2_request(),
        windows_text_v2_request(
            QWEN_IMAGE_21_WINDOWS_MODEL_ID,
            QWEN_IMAGE_21_WINDOWS_CAPABILITY_ID,
            QWEN_IMAGE_21_WINDOWS_RECIPE_ID,
            QWEN_IMAGE_21_WINDOWS_SNAPSHOT_ID,
            windows_qwen21_parameters(42),
        ),
        windows_text_v2_request(
            QWEN_IMAGE_21_TURBO_WINDOWS_MODEL_ID,
            QWEN_IMAGE_21_TURBO_WINDOWS_CAPABILITY_ID,
            QWEN_IMAGE_21_TURBO_WINDOWS_RECIPE_ID,
            QWEN_IMAGE_21_TURBO_WINDOWS_SNAPSHOT_ID,
            windows_qwen21_turbo_parameters(42),
        ),
        windows_text_v2_request(
            MOSS_IMAGE_SFT_WINDOWS_MODEL_ID,
            MOSS_IMAGE_SFT_WINDOWS_CAPABILITY_ID,
            MOSS_IMAGE_SFT_WINDOWS_RECIPE_ID,
            MOSS_IMAGE_SFT_WINDOWS_SNAPSHOT_ID,
            windows_moss_parameters(42),
        ),
    ] {
        request.validate().unwrap();
    }
}

#[test]
fn windows_klein_payloads_reject_identity_parameter_and_shape_drift() {
    for edit in [false, true] {
        for mutation in 0..12 {
            let mut request = windows_klein_v2_request(edit);
            match mutation {
                0 => request.capability_id = "image.text-to-image.flux2-klein-4b.v1".into(),
                1 => request.recipe_id = "klein4b-text-v1".into(),
                2 => request.runtime_lock_id = "mflux-0.24.0".into(),
                3 => request.model_snapshot_id = "flux2-klein-4b-windows-v1".into(),
                4 => request.seed = None,
                5 => request.sampling_steps = Some(5),
                6 => request.instruction.clear(),
                7 => {
                    request
                        .parameters
                        .insert("truncation".into(), serde_json::json!(true));
                }
                8 => {
                    request
                        .parameters
                        .insert("quantization".into(), serde_json::json!("q8"));
                }
                9 => request.outputs[0].width = 768,
                10 => request.outputs[0].data_type = "image_rgba".into(),
                _ => {
                    if edit {
                        request.inputs.clear();
                    } else {
                        request.inputs.push(InputAssetDescriptor {
                            role: "source".into(),
                            path: managed_path("runs/windows-klein/input.png"),
                            width: 512,
                            height: 512,
                            sha256: "a".repeat(64),
                        });
                    }
                }
            }
            assert!(
                request.validate().is_err(),
                "klein edit={edit} mutation {mutation}"
            );
        }
    }
}

#[test]
fn windows_tool_payloads_reject_identity_parameter_and_shape_drift() {
    for mutation in 0..10 {
        let mut request = windows_birefnet_v2_request();
        match mutation {
            0 => request.instruction = "remove background".into(),
            1 => request.seed = Some(7),
            2 => request.sampling_steps = Some(1),
            3 => request.runtime_lock_id = "onnxruntime-1.31.1-cpu".into(),
            4 => request.model_snapshot_id = "birefnet-epoch-244".into(),
            5 => request.inputs.clear(),
            6 => {
                request.outputs.pop();
            }
            7 => request.outputs[1].width = 11,
            8 => request.outputs[0].data_type = "image_rgb".into(),
            _ => {
                request.parameters.insert(
                    "provider".into(),
                    serde_json::json!("CUDAExecutionProvider"),
                );
            }
        }
        assert!(request.validate().is_err(), "birefnet mutation {mutation}");
    }

    for mutation in 0..9 {
        let mut request = windows_realesrgan_v2_request();
        match mutation {
            0 => request.instruction = "upscale".into(),
            1 => request.seed = Some(7),
            2 => request.sampling_steps = Some(1),
            3 => request.runtime_lock_id = "realesrgan-x4plus-ncnn-macos-v0.2.0-v1".into(),
            4 => request.model_snapshot_id = "realesrgan-x4plus".into(),
            5 => request.inputs.clear(),
            6 => request.outputs[0].width = 39,
            7 => request.outputs[0].data_type = "image_rgba".into(),
            _ => {
                request
                    .parameters
                    .insert("tile".into(), serde_json::json!(64));
            }
        }
        assert!(
            request.validate().is_err(),
            "realesrgan mutation {mutation}"
        );
    }
}

#[test]
fn windows_staged_text_payloads_require_exact_identities_and_parameter_maps() {
    let cases = [
        (
            QWEN_IMAGE_21_WINDOWS_MODEL_ID,
            QWEN_IMAGE_21_WINDOWS_CAPABILITY_ID,
            QWEN_IMAGE_21_WINDOWS_RECIPE_ID,
            QWEN_IMAGE_21_WINDOWS_SNAPSHOT_ID,
            windows_qwen21_parameters(42),
        ),
        (
            QWEN_IMAGE_21_TURBO_WINDOWS_MODEL_ID,
            QWEN_IMAGE_21_TURBO_WINDOWS_CAPABILITY_ID,
            QWEN_IMAGE_21_TURBO_WINDOWS_RECIPE_ID,
            QWEN_IMAGE_21_TURBO_WINDOWS_SNAPSHOT_ID,
            windows_qwen21_turbo_parameters(42),
        ),
        (
            MOSS_IMAGE_SFT_WINDOWS_MODEL_ID,
            MOSS_IMAGE_SFT_WINDOWS_CAPABILITY_ID,
            MOSS_IMAGE_SFT_WINDOWS_RECIPE_ID,
            MOSS_IMAGE_SFT_WINDOWS_SNAPSHOT_ID,
            windows_moss_parameters(42),
        ),
    ];
    for (model_id, capability_id, recipe_id, snapshot_id, parameters) in cases {
        for mutation in 0..9 {
            let mut request = windows_text_v2_request(
                model_id,
                capability_id,
                recipe_id,
                snapshot_id,
                parameters.clone(),
            );
            match mutation {
                0 => request.instruction.clear(),
                1 => request.seed = None,
                2 => request.sampling_steps = Some(3),
                3 => request.runtime_lock_id = "recipes-v3".into(),
                4 => request.model_snapshot_id = "other-snapshot".into(),
                5 => request.inputs.push(InputAssetDescriptor {
                    role: "source".into(),
                    path: managed_path("runs/windows-text/input.png"),
                    width: 1024,
                    height: 1024,
                    sha256: "a".repeat(64),
                }),
                6 => request.outputs[0].width = 768,
                7 => {
                    request
                        .parameters
                        .insert("truncation".into(), serde_json::json!(false));
                }
                _ => {
                    request
                        .parameters
                        .insert("steps".into(), serde_json::json!(9));
                }
            }
            assert!(
                request.validate().is_err(),
                "{model_id} mutation {mutation}"
            );
        }
    }
}

#[test]
fn macos_identities_with_windows_runtime_are_rejected() {
    let mut request = text_v2_request(
        "qwen-image-2.1-mflux-v1",
        "image.text-to-image.qwen-image-2.1.v1",
        "qwen21-txt2img-q8-40-v1",
        "qwen-image-2.1-d26bb61231c349cf6b7896fa83353113880e1ba3",
        40,
        1.0,
        "linear",
        2048,
    );
    request.runtime_lock_id = WINDOWS_V2_RUNTIME_ID.into();
    assert!(matches!(
        request.validate(),
        Err(Error::Invalid(
            "Windows runtime cannot serve a macOS recipe"
        ))
    ));

    let mut request = windows_klein_v2_request(false);
    request.model_id = "flux2-klein-4b-distilled".into();
    request.capability_id = "image.text-to-image.flux2-klein-4b.v1".into();
    request.recipe_id = "klein4b-text-v1".into();
    request.runtime_lock_id = WINDOWS_V2_RUNTIME_ID.into();
    assert!(request.validate().is_err());

    let mut unknown = windows_klein_v2_request(false);
    unknown.model_id = "unknown-model-v1".into();
    assert!(matches!(
        unknown.validate(),
        Err(Error::Invalid("unsupported v2 model recipe"))
    ));
    let mut unknown = windows_birefnet_v2_request();
    unknown.capability_id = "image.remove-background.unknown.v1".into();
    assert!(matches!(
        unknown.validate(),
        Err(Error::Invalid("unsupported v2 model recipe"))
    ));
}

#[test]
fn windows_z_image_payload_behaviour_is_unchanged() {
    assert!(windows_v2_request().validate().is_ok());
    assert_ne!(WINDOWS_V2_RUNTIME_ID, Z_IMAGE_WINDOWS_RUNTIME_ID);
    assert_ne!(KLEIN_WINDOWS_MODEL_ID, Z_IMAGE_WINDOWS_MODEL_ID);
    assert_ne!(KLEIN_WINDOWS_TEXT_RECIPE_ID, Z_IMAGE_WINDOWS_RECIPE_ID);
    assert!(is_windows_runtime_id(Z_IMAGE_WINDOWS_RUNTIME_ID));
    assert!(is_windows_runtime_id(WINDOWS_V2_RUNTIME_ID));
    assert!(!is_windows_runtime_id(
        "mflux-0.24.0-py3.13.12-macos-arm64-v1"
    ));
}
