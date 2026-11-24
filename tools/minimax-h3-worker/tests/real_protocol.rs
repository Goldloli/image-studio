#![cfg(not(windows))]

use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use worker_protocol::generation::{
    self, Envelope, ExecuteRequestV2, ExecutionContext, Message, OutputSpec, ProgressStage,
    VERSIONS_V2, WorkerIdentity, minimax_h3_parameters, read_frame, write_message,
};

const BUILD_ID: &str = "studio-minimax-h3-worker-macos-0.1.0";
const RUNTIME_ID: &str = "minimax-h3-sdcpp-3b12781-macos-arm64-metal-v1";
const SNAPSHOT_ID: &str = "minimax-h3-image-q3ks-sdcpp-macos-v1";
const MODEL_ID: &str = "minimax-h3-image-q3ks-sdcpp-macos-v1";
const CAPABILITY: &str = "image.text-to-image.minimax-h3-image.gguf.macos-v1";
const RECIPE: &str = "minimax-h3-image-txt2img-q3-q2te-6-macos-v1";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn identity() -> WorkerIdentity {
    WorkerIdentity {
        build_id: BUILD_ID.into(),
        runtime_lock_id: RUNTIME_ID.into(),
        model_snapshot_id: SNAPSHOT_ID.into(),
    }
}

fn send(stdin: &mut impl Write, envelope: &Envelope) {
    write_message(stdin, envelope).unwrap();
    stdin.flush().unwrap();
}

#[test]
#[ignore = "explicit acceptance: runs the real 23 GB MiniMax-H3-Image native protocol generation"]
fn actual_native_protocol_generates_square() {
    let repo = repo_root();
    let poc = std::env::var_os("STUDIO_MINIMAX_MODEL_SOURCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("target/minimax-h3-poc/models-segmented"));
    let runtime = std::env::var_os("STUDIO_MINIMAX_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("target/minimax-h3-poc/runtime/sdcpp-3b12781-macos-arm64"));
    let evidence_suffix = std::env::var("STUDIO_MINIMAX_EVIDENCE_SUFFIX").unwrap_or_default();
    let output_dir = repo.join(format!(
        "target/minimax-h3-worker-real-protocol{evidence_suffix}"
    ));
    let resolution = std::env::var("STUDIO_MINIMAX_TEST_RESOLUTION")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(1024);
    assert!(matches!(resolution, 1024 | 2048));
    if output_dir.exists() {
        fs::remove_dir_all(&output_dir).unwrap();
    }
    fs::create_dir_all(&output_dir).unwrap();
    let stem = format!("macos-product-worker-{resolution}{evidence_suffix}");
    let log_path = repo.join(format!("verification/minimax-h3-image-20261124/{stem}.log"));
    let stderr = File::create(&log_path).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_minimax-h3-worker"))
        .args([
            "--sd-cli",
            runtime.join("sd-cli").to_str().unwrap(),
            "--diffusion-model",
            poc.join("minimax_h3_image_pruned-Q3_K_S.gguf")
                .to_str()
                .unwrap(),
            "--llm",
            poc.join("qwen3vl_32b_minimax_h3-Q2_K_M.gguf")
                .to_str()
                .unwrap(),
            "--vae",
            poc.join("minimax_h3_image_vae_fp16.safetensors")
                .to_str()
                .unwrap(),
            "--tokenizer",
            poc.join("tokenizer.json").to_str().unwrap(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(stderr))
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let session_id = "session-minimax-real".to_owned();

    send(
        &mut stdin,
        &Envelope {
            family: generation::FAMILY.into(),
            version: None,
            session_id: session_id.clone(),
            request_id: "hello".into(),
            sequence: 0,
            execution: None,
            message: Message::Hello {
                versions: VERSIONS_V2,
                build_id: BUILD_ID.into(),
                expected_worker: identity(),
                capabilities: vec![CAPABILITY.into()],
            },
        },
    );
    let response = Envelope::decode_frame(&read_frame(&mut stdout).unwrap().unwrap()).unwrap();
    assert!(matches!(response.message, Message::HelloAccepted { .. }));

    let execution = ExecutionContext {
        job_id: "job-minimax-real".into(),
        attempt_id: "attempt-minimax-real".into(),
        execution_token: "token-minimax-real".into(),
    };
    let request = ExecuteRequestV2 {
        model_id: MODEL_ID.into(),
        capability_id: CAPABILITY.into(),
        recipe_id: RECIPE.into(),
        recipe_version: 1,
        runtime_lock_id: RUNTIME_ID.into(),
        model_snapshot_id: SNAPSHOT_ID.into(),
        instruction: "An editorial product photograph of a brushed copper espresso machine on a walnut counter, with a white ceramic cup and a small card reading \"IMAGE STUDIO\". Soft morning window light, realistic reflections, precise typography, natural materials, shallow depth of field, high detail.".into(),
        seed: Some(42),
        sampling_steps: Some(6),
        parameters: minimax_h3_parameters(42, resolution),
        inputs: vec![],
        outputs: vec![OutputSpec {
            role: "primary".into(),
            data_type: "image_rgb".into(),
            width: resolution,
            height: resolution,
            max_bytes: generation::MAX_UPSCALE_OUTPUT_BYTES,
        }],
        model_path: poc.to_str().unwrap().into(),
        output_dir: output_dir.to_str().unwrap().into(),
    };
    send(
        &mut stdin,
        &Envelope {
            family: generation::FAMILY.into(),
            version: Some(generation::PROTOCOL_VERSION_V2),
            session_id: session_id.clone(),
            request_id: "execute".into(),
            sequence: 1,
            execution: Some(execution.clone()),
            message: Message::ExecuteV2 { request },
        },
    );

    let started = Instant::now();
    let mut stages = Vec::new();
    let candidate = loop {
        let frame = read_frame(&mut stdout)
            .unwrap()
            .expect("worker closed stdout before a terminal message");
        let envelope = Envelope::decode_frame(&frame).unwrap();
        assert_eq!(envelope.execution.as_ref(), Some(&execution));
        match envelope.message {
            Message::Progress { stage, .. } => stages.push(match stage {
                ProgressStage::Loading => "loading",
                ProgressStage::Encoding => "encoding",
                ProgressStage::Sampling => "sampling",
                ProgressStage::Inference => "inference",
                ProgressStage::Decoding => "decoding",
                ProgressStage::Writing => "writing",
            }),
            Message::OutputCandidateV2 { candidate } => break candidate,
            Message::Error { code, summary } => panic!("{code}: {summary}"),
            other => panic!("unexpected worker message: {other:?}"),
        }
    };
    let status = child.wait().unwrap();
    assert!(status.success());
    assert_eq!(candidate.role, "primary");
    assert_eq!(candidate.data_type, "image_rgb");
    assert_eq!(
        (candidate.width, candidate.height),
        (resolution, resolution)
    );
    let output = output_dir.join(&candidate.relative_path);
    let bytes = fs::read(&output).unwrap();
    assert_eq!(bytes.len() as u64, candidate.size_bytes);
    let digest = format!("{:x}", Sha256::digest(&bytes));
    assert_eq!(digest, candidate.sha256);
    let evidence_png = repo.join(format!(
        "verification/minimax-h3-image-20261124/{stem}-seed42.png"
    ));
    fs::copy(&output, &evidence_png).unwrap();
    let evidence = serde_json::json!({
        "kind": "minimax-h3-image-product-worker-protocol-acceptance",
        "status": "passed",
        "scope": "real generation through the product minimax-h3-worker and pinned stable-diffusion.cpp sidecar on this macOS arm64 host",
        "elapsed_seconds": started.elapsed().as_secs_f64(),
        "width": candidate.width,
        "height": candidate.height,
        "size_bytes": candidate.size_bytes,
        "sha256": candidate.sha256,
        "stages": stages,
        "process_exit_after_candidate": status.success(),
        "runtime_lock_id": RUNTIME_ID,
        "model_snapshot_id": SNAPSHOT_ID,
        "worker_build_id": BUILD_ID,
        "execution_environment": std::env::var("STUDIO_MINIMAX_EXECUTION_ENVIRONMENT")
            .unwrap_or_else(|_| "normal".into()),
        "network_denied": std::env::var("STUDIO_MINIMAX_NETWORK_DENIED").as_deref() == Ok("1"),
        "log": format!("verification/minimax-h3-image-20261124/{stem}.log"),
        "png": format!("verification/minimax-h3-image-20261124/{stem}-seed42.png"),
        "evidence_boundary": format!("This proves the dedicated worker protocol and native generation path at {resolution} square on this host; other sizes, cancel, reopen, UI send and export are separate gates.")
    });
    fs::write(
        repo.join(format!(
            "verification/minimax-h3-image-20261124/{stem}.json"
        )),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    assert!(started.elapsed() < Duration::from_secs(900));
}

#[test]
#[ignore = "explicit acceptance: starts the real native inference and verifies cancellation"]
fn actual_native_protocol_cancel_removes_output_and_exits_cleanly() {
    let repo = repo_root();
    let poc = std::env::var_os("STUDIO_MINIMAX_MODEL_SOURCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("target/minimax-h3-poc/models-segmented"));
    let runtime = std::env::var_os("STUDIO_MINIMAX_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("target/minimax-h3-poc/runtime/sdcpp-3b12781-macos-arm64"));
    let output_dir = repo.join("target/minimax-h3-worker-real-cancel");
    if output_dir.exists() {
        fs::remove_dir_all(&output_dir).unwrap();
    }
    fs::create_dir_all(&output_dir).unwrap();
    let log_path =
        repo.join("verification/minimax-h3-image-20261124/macos-product-worker-cancel.log");
    let stderr = File::create(&log_path).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_minimax-h3-worker"))
        .args([
            "--sd-cli",
            runtime.join("sd-cli").to_str().unwrap(),
            "--diffusion-model",
            poc.join("minimax_h3_image_pruned-Q3_K_S.gguf")
                .to_str()
                .unwrap(),
            "--llm",
            poc.join("qwen3vl_32b_minimax_h3-Q2_K_M.gguf")
                .to_str()
                .unwrap(),
            "--vae",
            poc.join("minimax_h3_image_vae_fp16.safetensors")
                .to_str()
                .unwrap(),
            "--tokenizer",
            poc.join("tokenizer.json").to_str().unwrap(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(stderr))
        .spawn()
        .unwrap();
    let wrapper_pid = child.id();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let session_id = "session-minimax-cancel".to_owned();
    send(
        &mut stdin,
        &Envelope {
            family: generation::FAMILY.into(),
            version: None,
            session_id: session_id.clone(),
            request_id: "hello".into(),
            sequence: 0,
            execution: None,
            message: Message::Hello {
                versions: VERSIONS_V2,
                build_id: BUILD_ID.into(),
                expected_worker: identity(),
                capabilities: vec![CAPABILITY.into()],
            },
        },
    );
    let response = Envelope::decode_frame(&read_frame(&mut stdout).unwrap().unwrap()).unwrap();
    assert!(matches!(response.message, Message::HelloAccepted { .. }));
    let execution = ExecutionContext {
        job_id: "job-minimax-cancel".into(),
        attempt_id: "attempt-minimax-cancel".into(),
        execution_token: "token-minimax-cancel".into(),
    };
    send(
        &mut stdin,
        &Envelope {
            family: generation::FAMILY.into(),
            version: Some(generation::PROTOCOL_VERSION_V2),
            session_id: session_id.clone(),
            request_id: "execute".into(),
            sequence: 1,
            execution: Some(execution.clone()),
            message: Message::ExecuteV2 {
                request: ExecuteRequestV2 {
                    model_id: MODEL_ID.into(),
                    capability_id: CAPABILITY.into(),
                    recipe_id: RECIPE.into(),
                    recipe_version: 1,
                    runtime_lock_id: RUNTIME_ID.into(),
                    model_snapshot_id: SNAPSHOT_ID.into(),
                    instruction: "A detailed studio photograph of a ceramic cup and a card reading IMAGE STUDIO".into(),
                    seed: Some(42),
                    sampling_steps: Some(6),
                    parameters: minimax_h3_parameters(42, 1024),
                    inputs: vec![],
                    outputs: vec![OutputSpec {
                        role: "primary".into(),
                        data_type: "image_rgb".into(),
                        width: 1024,
                        height: 1024,
                        max_bytes: generation::MAX_UPSCALE_OUTPUT_BYTES,
                    }],
                    model_path: poc.to_str().unwrap().into(),
                    output_dir: output_dir.to_str().unwrap().into(),
                },
            },
        },
    );
    loop {
        let frame = read_frame(&mut stdout).unwrap().unwrap();
        let envelope = Envelope::decode_frame(&frame).unwrap();
        if matches!(
            envelope.message,
            Message::Progress {
                stage: ProgressStage::Inference,
                ..
            }
        ) {
            break;
        }
    }
    std::thread::sleep(Duration::from_secs(2));
    send(
        &mut stdin,
        &Envelope {
            family: generation::FAMILY.into(),
            version: Some(generation::PROTOCOL_VERSION_V2),
            session_id,
            request_id: "execute".into(),
            sequence: 2,
            execution: Some(execution),
            message: Message::Cancel,
        },
    );
    let started = Instant::now();
    loop {
        let frame = read_frame(&mut stdout).unwrap().unwrap();
        let envelope = Envelope::decode_frame(&frame).unwrap();
        match envelope.message {
            Message::Progress { .. } => {}
            Message::Cancelled => break,
            Message::Error { code, summary } => panic!("{code}: {summary}"),
            other => panic!("unexpected cancellation response: {other:?}"),
        }
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            panic!("worker did not exit after cancellation");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(status.success());
    assert!(!output_dir.join("primary.png").exists());
    let evidence = serde_json::json!({
        "kind": "minimax-h3-image-product-worker-cancel-acceptance",
        "status": "passed",
        "scope": "real inference-stage cancel through the product worker",
        "elapsed_seconds": started.elapsed().as_secs_f64(),
        "worker_pid": wrapper_pid,
        "worker_exit_success": status.success(),
        "candidate_written": false,
        "runtime_lock_id": RUNTIME_ID,
        "model_snapshot_id": SNAPSHOT_ID,
        "worker_build_id": BUILD_ID,
        "log": "verification/minimax-h3-image-20261124/macos-product-worker-cancel.log",
        "evidence_boundary": "This proves the worker terminates its direct native child after a valid cancel frame. Supervisor whole-tree enforcement and host restart recovery remain separate gates."
    });
    fs::write(
        repo.join("verification/minimax-h3-image-20261124/macos-product-worker-cancel.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
}
