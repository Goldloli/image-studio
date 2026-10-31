//! Explicit same-process Klein continuity and duplicate-submission acceptance aid.

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut root = None;
    let mut mode = "text".to_string();
    let mut source = None;
    let mut specs_path = None;
    let mut export_directory = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--data-dir" => root = Some(PathBuf::from(args.next().ok_or("missing data root")?)),
            "--mode" => mode = args.next().ok_or("missing mode")?,
            "--source" => source = Some(PathBuf::from(args.next().ok_or("missing source path")?)),
            "--specs" => {
                specs_path = Some(PathBuf::from(args.next().ok_or("missing task specs path")?))
            }
            "--export-dir" => {
                export_directory = Some(PathBuf::from(
                    args.next().ok_or("missing export directory")?,
                ))
            }
            _ => return Err(format!("unknown option: {flag}").into()),
        }
    }
    if !matches!(mode.as_str(), "text" | "edit") {
        return Err("--mode must be text or edit".into());
    }
    let root = root.ok_or("--data-dir is required")?;
    let specs_path = specs_path.ok_or("--specs is required")?;
    let export_directory = export_directory.ok_or("--export-dir is required")?;
    std::fs::create_dir_all(&export_directory)?;
    let specs: Vec<TaskSpec> = serde_json::from_slice(&std::fs::read(specs_path)?)?;
    if specs.len() != 3 {
        return Err("--specs must contain exactly three tasks".into());
    }
    if mode == "edit" && source.is_none() {
        return Err("edit mode requires --source".into());
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

    let source_id = source
        .map(|path| {
            let imported = service.import_source(&path)?;
            imported["assetId"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "source import did not return an asset ID".to_string())
        })
        .transpose()?;
    let capability = if source_id.is_some() {
        "image.edit.flux2-klein-4b.v1"
    } else {
        "image.text-to-image.flux2-klein-4b.v1"
    };

    let batch_started = Instant::now();
    let mut tasks = Vec::new();
    for (index, spec) in specs.iter().enumerate() {
        let accepted = submit_task(
            &service,
            index,
            capability,
            source_id.as_deref(),
            &spec.prompt,
            &spec.seed,
            &format!("continuous-{}", uuid::Uuid::new_v4()),
        )?;
        let turn_id = accepted["turn"]["id"]
            .as_str()
            .ok_or("accepted turn has no ID")?
            .to_owned();
        let observation = wait_for_turn(&service, &turn_id)?;
        let asset_id = observation["turn"]["assetId"]
            .as_str()
            .ok_or("successful turn has no asset ID")?;
        let bytes = service.read_asset(asset_id)?;
        let export = export_directory.join(format!("continuous-{:02}.png", index + 1));
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
            "workerProcessesAfterCompletion": worker_processes(),
            "executionAfterCompletion": observation["execution"]
        }));
        println!(
            "{}",
            json!({"continuousTask": tasks.last(), "completed": index + 1})
        );
    }

    let duplicate_spec = TaskSpec {
        id: "p08-duplicate-and-busy".into(),
        prompt: "A pale blue ceramic cup on a bamboo tray beside a green leaf, soft morning light, simple still life photograph".into(),
        seed: "107".into(),
    };
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
    let saved = save_task_draft(
        &service,
        draft_id,
        expected_version,
        capability,
        source_id.as_deref(),
        &duplicate_spec.prompt,
        &duplicate_spec.seed,
    )?;
    let submission_key = format!("p08-duplicate-{}", uuid::Uuid::new_v4());
    let first = service.conversation_send(conversation_input(
        &conversation_id,
        draft_id,
        saved,
        capability,
        source_id.as_deref(),
        &duplicate_spec.prompt,
        &duplicate_spec.seed,
        &submission_key,
    ))?;
    let duplicate = service.conversation_send(conversation_input(
        &conversation_id,
        draft_id,
        saved,
        capability,
        source_id.as_deref(),
        &duplicate_spec.prompt,
        &duplicate_spec.seed,
        &submission_key,
    ))?;
    let same_turn = duplicate["created"] == false
        && duplicate["turn"]["id"].as_str().is_some()
        && duplicate["turn"]["id"] == first["turn"]["id"];
    if !same_turn {
        return Err("duplicate submission key created or returned a different turn".into());
    }

    let after_duplicate = service.snapshot()?;
    let busy_draft_version = after_duplicate["drafts"]
        .as_array()
        .and_then(|drafts| {
            drafts
                .iter()
                .find(|draft| draft["id"].as_u64() == Some(draft_id))
        })
        .and_then(|draft| draft["version"].as_u64())
        .ok_or("missing post-submit draft version")?;
    let busy_prompt = "PRESERVE THIS UNSENT PROMPT WHILE THE ACTIVE TASK COMPLETES";
    let busy_saved = save_task_draft(
        &service,
        draft_id,
        busy_draft_version,
        capability,
        source_id.as_deref(),
        busy_prompt,
        "108",
    )?;
    let turns_before_busy = service.snapshot()?["turns"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    let busy_error = service
        .conversation_send(conversation_input(
            &conversation_id,
            draft_id,
            busy_saved,
            capability,
            source_id.as_deref(),
            busy_prompt,
            "108",
            &format!("p08-busy-{}", uuid::Uuid::new_v4()),
        ))
        .expect_err("a second distinct submission must be rejected while generation is active");
    let busy_snapshot = service.snapshot()?;
    let turns_after_busy = busy_snapshot["turns"].as_array().map(Vec::len).unwrap_or(0);
    let preserved_busy_prompt = busy_snapshot["drafts"]
        .as_array()
        .and_then(|drafts| {
            drafts
                .iter()
                .find(|draft| draft["id"].as_u64() == Some(draft_id))
        })
        .and_then(|draft| draft["prompt"].as_str())
        == Some(busy_prompt);
    if turns_before_busy != turns_after_busy || !preserved_busy_prompt {
        return Err("busy rejection created a turn or cleared the unsent draft".into());
    }

    let turn_id = first["turn"]["id"]
        .as_str()
        .ok_or("duplicate turn has no ID")?
        .to_owned();
    let observation = wait_for_turn(&service, &turn_id)?;
    let asset_id = observation["turn"]["assetId"]
        .as_str()
        .ok_or("duplicate task has no asset ID")?;
    let bytes = service.read_asset(asset_id)?;
    let export = export_directory.join("p08-duplicate.png");
    service.export(asset_id, &export, false)?;
    if std::fs::read(&export)? != bytes {
        return Err("duplicate-check export differs from the committed asset".into());
    }
    let duplicate_record = json!({
        "sameTurnReturnedOnDuplicate": same_turn,
        "firstCreated": first["created"],
        "duplicateCreated": duplicate["created"],
        "turnId": turn_id,
        "jobId": observation["turn"]["jobId"],
        "attemptId": observation["turn"]["attemptId"],
        "assetId": asset_id,
        "assetSha256": format!("{:x}", Sha256::digest(&bytes)),
        "exportPath": export,
        "busyError": busy_error,
        "busyRejectionCreatedNoTurn": turns_before_busy == turns_after_busy,
        "busyDraftPreserved": preserved_busy_prompt,
        "elapsedMilliseconds": observation["elapsedMilliseconds"],
        "maxSamplingStep": observation["maxSamplingStep"],
        "stageSequence": observation["stageSequence"],
        "hostResidentBytesAfterCompletion": resident_bytes()?,
        "workerProcessesAfterCompletion": worker_processes(),
        "executionAfterCompletion": observation["execution"]
    });

    println!(
        "{}",
        json!({
            "schemaVersion": 1,
            "mode": mode,
            "sameStudioServiceForAllTasks": true,
            "capabilityId": capability,
            "sourceAssetId": source_id,
            "continuousTasks": tasks,
            "duplicateAndBusy": duplicate_record,
            "processElapsedMilliseconds": batch_started.elapsed().as_millis()
        })
    );
    Ok(())
}

fn submit_task(
    service: &Arc<StudioService>,
    index: usize,
    capability: &str,
    source_id: Option<&str>,
    prompt: &str,
    seed: &str,
    submission_key: &str,
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
    let saved = save_task_draft(
        service,
        draft_id,
        expected_version,
        capability,
        source_id,
        prompt,
        seed,
    )?;
    let accepted = service.conversation_send(conversation_input(
        &conversation_id,
        draft_id,
        saved,
        capability,
        source_id,
        prompt,
        seed,
        submission_key,
    ))?;
    if accepted["created"] != true {
        return Err(format!(
            "continuous task {} was not newly created",
            index + 1
        ));
    }
    Ok(accepted)
}

fn save_task_draft(
    service: &StudioService,
    draft_id: u64,
    expected_version: u64,
    capability: &str,
    source_id: Option<&str>,
    prompt: &str,
    seed: &str,
) -> Result<u64, String> {
    service
        .save_draft(
            DraftInput {
                id: draft_id,
                name: "Klein continuous acceptance check".into(),
                prompt: prompt.into(),
                params: json!({
                    "modelId": "flux2-klein-4b-distilled",
                    "capabilityId": capability,
                    "sourceAssetId": source_id,
                    "aspect": "1:1",
                    "count": 1,
                    "seed": seed
                }),
                collapsed: false,
                prompt_source_id: None,
            },
            expected_version,
        )
        .map(|draft| draft.version)
        .map_err(|error| error.to_string())
}

fn conversation_input(
    conversation_id: &str,
    draft_id: u64,
    expected_version: u64,
    capability: &str,
    source_id: Option<&str>,
    prompt: &str,
    seed: &str,
    submission_key: &str,
) -> ConversationSendInput {
    ConversationSendInput {
        conversation_id: conversation_id.into(),
        draft_id,
        expected_version,
        prompt: prompt.into(),
        model_id: "flux2-klein-4b-distilled".into(),
        capability_id: Some(capability.into()),
        source_asset_id: source_id.map(str::to_owned),
        aspect: "1:1".into(),
        count: 1,
        seed: seed.into(),
        submission_key: submission_key.into(),
    }
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
        let stage = snapshot["execution"]["stage"]
            .as_str()
            .unwrap_or("")
            .to_owned();
        if stage_sequence.last() != Some(&stage) {
            stage_sequence.push(stage.clone());
        }
        if stage == "sampling" {
            max_sampling_step =
                max_sampling_step.max(snapshot["execution"]["step"].as_u64().unwrap_or_default());
        }
        match turn["state"].as_str() {
            Some("Succeeded") => {
                if max_sampling_step < 1 {
                    return Err("successful Klein task never reported a sampling step".into());
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
                    "Klein turn ended as {}: {}",
                    turn["state"],
                    turn["error"].as_str().unwrap_or("no detail")
                ));
            }
            _ => {}
        }
        if started.elapsed() > Duration::from_secs(900) {
            return Err("Klein batch task exceeded the acceptance deadline".into());
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
            line.contains("studio_mflux_worker_v2")
                || line.contains("studio-mflux-worker-v2")
                || line.contains("/inference-supervisor")
        })
        .map(str::to_owned)
        .collect()
}
