//! Explicit same-process Qwen/MOSS text-model acceptance helper.

use image_studio_desktop::{
    adapters::AppPaths,
    studio::{ConversationSendInput, DraftInput, StudioService},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskSpec {
    id: String,
    prompt: String,
    seed: String,
}

fn capability_for_model(model_id: &str) -> Result<&'static str, String> {
    match model_id {
        studio_core::recipes::QWEN_IMAGE_21_MODEL => {
            Ok(studio_core::recipes::QWEN_IMAGE_21_CAPABILITY)
        }
        studio_core::recipes::QWEN_IMAGE_21_TURBO_MODEL => {
            Ok(studio_core::recipes::QWEN_IMAGE_21_TURBO_CAPABILITY)
        }
        studio_core::recipes::MOSS_IMAGE_SFT_MODEL => {
            Ok(studio_core::recipes::MOSS_IMAGE_SFT_CAPABILITY)
        }
        _ => Err("--model-id must be an exact Qwen/MOSS text model".into()),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut root = None;
    let mut model_id = None;
    let mut specs_path = None;
    let mut export_directory = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--data-dir" => root = Some(PathBuf::from(args.next().ok_or("missing data root")?)),
            "--model-id" => model_id = Some(args.next().ok_or("missing model ID")?),
            "--specs" => {
                specs_path = Some(PathBuf::from(args.next().ok_or("missing specs path")?))
            }
            "--export-dir" => {
                export_directory = Some(PathBuf::from(
                    args.next().ok_or("missing export directory")?,
                ))
            }
            _ => return Err(format!("unknown option: {flag}").into()),
        }
    }
    let root = root.ok_or("--data-dir is required")?;
    let model_id = model_id.ok_or("--model-id is required")?;
    let specs_path = specs_path.ok_or("--specs is required")?;
    let export_directory = export_directory.ok_or("--export-dir is required")?;
    let capability = capability_for_model(&model_id)?;
    std::fs::create_dir_all(&export_directory)?;
    let specs: Vec<TaskSpec> = serde_json::from_slice(&std::fs::read(specs_path)?)?;
    let expected_tasks = if model_id == studio_core::recipes::MOSS_IMAGE_SFT_MODEL {
        4
    } else {
        3
    };
    if specs.len() != expected_tasks {
        return Err(format!("--specs must contain exactly {expected_tasks} tasks").into());
    }

    let executable = std::env::current_exe()?;
    let supervisor = executable
        .parent()
        .and_then(Path::parent)
        .ok_or("invalid target layout")?
        .join("inference-supervisor");
    let service = Arc::new(StudioService::new(
        AppPaths::resolve(Some(&root))?,
        supervisor,
    ));
    if let Some(error) = &service.startup_error {
        return Err(error.clone().into());
    }
    let model = service
        .model_views()
        .into_iter()
        .find(|value| value["id"] == model_id.as_str())
        .ok_or("registered model view is missing")?;
    if model["ready"] != true || model["deviceVerified"] != true {
        return Err("model must be fully installed and device-verified before this helper".into());
    }

    let batch_started = Instant::now();
    let mut tasks = Vec::new();
    for spec in &specs {
        let accepted = submit_task(&service, &model_id, capability, spec)?;
        let turn_id = accepted["turn"]["id"]
            .as_str()
            .ok_or("accepted turn has no ID")?
            .to_owned();
        let observation = wait_for_turn(&service, &turn_id)?;
        let asset_id = observation["turn"]["assetId"]
            .as_str()
            .ok_or("successful turn has no asset ID")?;
        let bytes = service.read_asset(asset_id)?;
        let export = export_directory.join(format!("{}.png", spec.id));
        service.export(asset_id, &export, false)?;
        if std::fs::read(&export)? != bytes {
            return Err("exported PNG differs from the committed asset".into());
        }
        tasks.push(json!({
            "id": spec.id,
            "prompt": spec.prompt,
            "requestedSeed": spec.seed,
            "turnId": turn_id,
            "jobId": observation["turn"]["jobId"],
            "attemptId": observation["turn"]["attemptId"],
            "assetId": asset_id,
            "assetSha256": format!("{:x}", Sha256::digest(&bytes)),
            "exportPath": export,
            "elapsedMilliseconds": observation["elapsedMilliseconds"],
            "maxSamplingStep": observation["maxSamplingStep"],
            "stageSequence": observation["stageSequence"],
            "hostResidentBytesAfterCompletion": resident_bytes()?,
            "backendPeakBytes": Value::Null,
            "workerProcessesAfterCompletion": worker_processes(),
            "executionAfterCompletion": observation["execution"]
        }));
        println!(
            "{}",
            json!({"continuousTask": tasks.last(), "completed": tasks.len()})
        );
    }
    println!(
        "{}",
        json!({
            "modelId": model_id,
            "capabilityId": capability,
            "tasks": tasks,
            "processElapsedMilliseconds": batch_started.elapsed().as_millis()
        })
    );
    Ok(())
}

fn submit_task(
    service: &Arc<StudioService>,
    model_id: &str,
    capability: &str,
    spec: &TaskSpec,
) -> Result<Value, String> {
    let snapshot = service.snapshot()?;
    let conversation_id = snapshot["activeConversationId"]
        .as_str()
        .ok_or("missing active conversation")?
        .to_owned();
    let draft_id = snapshot["activeId"]
        .as_u64()
        .ok_or("missing active draft")?;
    let expected_version = snapshot["drafts"]
        .as_array()
        .and_then(|drafts| {
            drafts
                .iter()
                .find(|draft| draft["id"].as_u64() == Some(draft_id))
        })
        .and_then(|draft| draft["version"].as_u64())
        .ok_or("missing draft version")?;
    let saved = service
        .save_draft(
            DraftInput {
                id: draft_id,
                name: format!("{model_id} acceptance"),
                prompt: spec.prompt.clone(),
                params: json!({
                    "modelId": model_id,
                    "capabilityId": capability,
                    "sourceAssetId": null,
                    "aspect": "1:1",
                    "count": 1,
                    "seed": spec.seed
                }),
                collapsed: false,
                prompt_source_id: None,
            },
            expected_version,
        )
        .map_err(|error| error.to_string())?;
    let accepted = service.conversation_send(ConversationSendInput {
        conversation_id,
        draft_id,
        expected_version: saved.version,
        prompt: spec.prompt.clone(),
        model_id: model_id.into(),
        capability_id: Some(capability.into()),
        source_asset_id: None,
        aspect: "1:1".into(),
        count: 1,
        seed: spec.seed.clone(),
        submission_key: format!("{}-{}", spec.id, uuid::Uuid::new_v4()),
    })?;
    if accepted["created"] != true {
        return Err(format!("{} was not newly created", spec.id));
    }
    Ok(accepted)
}

fn wait_for_turn(service: &StudioService, turn_id: &str) -> Result<Value, String> {
    let started = Instant::now();
    let mut max_sampling_step = 0_u64;
    let mut stage_sequence = Vec::<String>::new();
    loop {
        let snapshot = service.snapshot()?;
        let turn = snapshot["turns"]
            .as_array()
            .and_then(|turns| turns.iter().find(|turn| turn["id"] == turn_id))
            .cloned()
            .ok_or("accepted turn is not visible")?;
        if turn["jobId"].as_str().is_some()
            && turn["jobId"].as_str() == snapshot["execution"]["jobId"].as_str()
        {
            let stage = snapshot["execution"]["stage"]
                .as_str()
                .unwrap_or("")
                .to_owned();
            if stage_sequence.last() != Some(&stage) {
                stage_sequence.push(stage);
            }
            if stage_sequence.last().is_some_and(|stage| stage == "sampling") {
                max_sampling_step = max_sampling_step
                    .max(snapshot["execution"]["step"].as_u64().unwrap_or_default());
            }
        }
        match turn["state"].as_str() {
            Some("Succeeded") => {
                if snapshot["execution"]["busy"] == true {
                    std::thread::sleep(Duration::from_secs(1));
                    continue;
                }
                if max_sampling_step < 1 {
                    return Err("successful task never reported a sampling step".into());
                }
                return Ok(json!({
                    "turn": turn,
                    "execution": snapshot["execution"],
                    "elapsedMilliseconds": started.elapsed().as_millis(),
                    "maxSamplingStep": max_sampling_step,
                    "stageSequence": stage_sequence
                }));
            }
            Some("Failed" | "Rejected" | "Cancelled" | "Interrupted") => {
                return Err(format!(
                    "turn ended as {}: {}",
                    turn["state"],
                    turn["error"].as_str().unwrap_or("no detail")
                ));
            }
            _ => {}
        }
        if started.elapsed() > Duration::from_secs(2400) {
            return Err("text-model acceptance task exceeded the deadline".into());
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn resident_bytes() -> Result<u64, String> {
    let output = Command::new("/bin/ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .map_err(|error| error.to_string())?;
    let kilobytes = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u64>()
        .map_err(|error| error.to_string())?;
    Ok(kilobytes * 1024)
}

fn worker_processes() -> Vec<String> {
    let Ok(output) = Command::new("/bin/ps")
        .args(["-axo", "pid=,ppid=,command="])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| {
            line.contains("studio_mflux_worker_v3")
                || line.contains("studio-mflux-worker-v3")
                || line.contains("/inference-supervisor")
        })
        .map(str::to_owned)
        .collect()
}
