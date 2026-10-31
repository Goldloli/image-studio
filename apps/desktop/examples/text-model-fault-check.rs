//! Explicit Qwen/MOSS cancellation, kill, and retry acceptance helper.

use image_studio_desktop::{
    adapters::AppPaths,
    studio::{ConversationSendInput, DraftInput, StudioService},
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

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
        _ => Err("unsupported text model".into()),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut root = None;
    let mut model_id = None;
    let mut prompt =
        "一只橙色的小猫坐在窗边，窗外是秋天的树叶，温暖自然光，细腻的水彩插画。".to_string();
    let mut seed = "42".to_string();
    let mut cancel_stage = Some("sampling".to_string());
    let mut kill_stage = None;
    let mut retry_after_cancel = false;
    let mut retry_job = None;
    let mut export_directory = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--data-dir" => root = Some(PathBuf::from(args.next().ok_or("missing data root")?)),
            "--model-id" => model_id = Some(args.next().ok_or("missing model ID")?),
            "--prompt" => prompt = args.next().ok_or("missing prompt")?,
            "--seed" => seed = args.next().ok_or("missing seed")?,
            "--cancel-stage" => cancel_stage = Some(args.next().ok_or("missing cancel stage")?),
            "--kill-at-stage" => kill_stage = Some(args.next().ok_or("missing kill stage")?),
            "--retry-after-cancel" => retry_after_cancel = true,
            "--retry-job" => retry_job = Some(args.next().ok_or("missing job ID")?),
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
    let export_directory = export_directory.ok_or("--export-dir is required")?;
    std::fs::create_dir_all(&export_directory)?;
    for stage in cancel_stage.iter().chain(kill_stage.iter()) {
        if !matches!(
            stage.as_str(),
            "loading" | "encoding" | "sampling" | "writing"
        ) {
            return Err("stage must be loading, encoding, sampling, or writing".into());
        }
    }
    let capability = capability_for_model(&model_id)?;
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

    if let Some(job_id) = retry_job {
        let retried = service.retry(&job_id)?;
        let turn_id = retried["turnId"]
            .as_str()
            .ok_or("retry job has no turn ID")?
            .to_owned();
        let turn = wait_for_turn(&service, &turn_id)?;
        let asset_id = turn["assetId"]
            .as_str()
            .ok_or("retried turn has no asset ID")?;
        let export = export_directory.join("retry.png");
        service.export(asset_id, &export, false)?;
        println!(
            "{}",
            json!({
                "mode": "retry-job",
                "jobId": job_id,
                "turn": turn,
                "exportPath": export,
                "exportSha256": sha256_file(&export)?
            })
        );
        return Ok(());
    }

    let accepted = submit_task(&service, &model_id, capability, &prompt, &seed)?;
    let turn_id = accepted["turn"]["id"]
        .as_str()
        .ok_or("accepted turn has no ID")?
        .to_owned();
    let started = Instant::now();
    let mut cancellation_sent = false;
    let mut cancelled_at = None;
    let turn = loop {
        let snapshot = service.snapshot()?;
        let stage = snapshot["execution"]["stage"].as_str().unwrap_or("");
        let sampling_has_started =
            stage != "sampling" || snapshot["execution"]["step"].as_u64().unwrap_or_default() >= 1;
        if !cancellation_sent && cancel_stage.as_deref() == Some(stage) && sampling_has_started {
            service.conversation_cancel_turn(&turn_id)?;
            cancellation_sent = true;
            cancelled_at = Some(json!({
                "stage": stage,
                "step": snapshot["execution"]["step"],
                "total": snapshot["execution"]["total"],
                "elapsedMilliseconds": started.elapsed().as_millis()
            }));
        }
        if kill_stage.as_deref() == Some(stage) && sampling_has_started {
            let kill_observed = json!({
                "stage": stage,
                "step": snapshot["execution"]["step"],
                "total": snapshot["execution"]["total"],
                "elapsedMilliseconds": started.elapsed().as_millis()
            });
            println!(
                "{}",
                json!({
                    "mode": "kill",
                    "turnId": turn_id,
                    "jobId": snapshot["execution"]["jobId"],
                    "killObservation": kill_observed,
                    "note": "process exits 73 without cleanup; reopen and retry the job"
                })
            );
            std::process::exit(73);
        }
        let turn = snapshot["turns"]
            .as_array()
            .and_then(|turns| turns.iter().find(|turn| turn["id"] == turn_id))
            .cloned()
            .ok_or("accepted turn is not visible")?;
        if matches!(
            turn["state"].as_str(),
            Some("Cancelled" | "Failed" | "Rejected" | "Interrupted" | "Succeeded")
        ) && snapshot["execution"]["busy"] != true
        {
            break turn;
        }
        if started.elapsed() > Duration::from_secs(900) {
            return Err("fault acceptance did not reach a terminal state".into());
        }
        std::thread::sleep(Duration::from_millis(500));
    };
    if turn["state"] != "Cancelled" {
        return Err(format!(
            "expected cancellation, got {}: {}",
            turn["state"],
            turn["error"].as_str().unwrap_or("no detail")
        )
        .into());
    }
    if !retry_after_cancel {
        println!(
            "{}",
            json!({
                "mode": "cancel",
                "turn": turn,
                "cancelledAt": cancelled_at,
                "resourceReleased": service.snapshot()?["execution"]["busy"] == false
            })
        );
        return Ok(());
    }
    let job_id = turn["jobId"]
        .as_str()
        .ok_or("cancelled turn has no job ID")?;
    let retried = service.retry(job_id)?;
    let retry_turn_id = retried["turnId"]
        .as_str()
        .ok_or("retry job has no turn ID")?
        .to_owned();
    let retry_turn = wait_for_turn(&service, &retry_turn_id)?;
    let asset_id = retry_turn["assetId"]
        .as_str()
        .ok_or("retried turn has no asset ID")?;
    let export = export_directory.join("cancel-retry.png");
    service.export(asset_id, &export, false)?;
    println!(
        "{}",
        json!({
            "mode": "cancel-retry",
            "cancelledTurn": turn,
            "cancelledAt": cancelled_at,
            "retryJob": retried,
            "retryTurn": retry_turn,
            "exportPath": export,
            "exportSha256": sha256_file(&export)?
        })
    );
    Ok(())
}

fn submit_task(
    service: &Arc<StudioService>,
    model_id: &str,
    capability: &str,
    prompt: &str,
    seed: &str,
) -> Result<Value, String> {
    let snapshot = service.snapshot()?;
    let conversation_id = snapshot["activeConversationId"]
        .as_str()
        .ok_or("missing active conversation")?
        .to_owned();
    let draft_id = snapshot["activeId"]
        .as_u64()
        .ok_or("missing active draft")?;
    let version = snapshot["drafts"]
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
                name: "文本模型故障验收".into(),
                prompt: prompt.into(),
                params: json!({
                    "modelId": model_id,
                    "capabilityId": capability,
                    "sourceAssetId": null,
                    "aspect": "1:1",
                    "count": 1,
                    "seed": seed
                }),
                collapsed: false,
                prompt_source_id: None,
            },
            version,
        )
        .map_err(|error| error.to_string())?;
    service.conversation_send(ConversationSendInput {
        conversation_id,
        draft_id,
        expected_version: saved.version,
        prompt: prompt.into(),
        model_id: model_id.into(),
        capability_id: Some(capability.into()),
        source_asset_id: None,
        aspect: "1:1".into(),
        count: 1,
        seed: seed.into(),
        submission_key: format!("fault-{}", uuid::Uuid::new_v4()),
    })
}

fn wait_for_turn(service: &StudioService, turn_id: &str) -> Result<Value, String> {
    let started = Instant::now();
    loop {
        let snapshot = service.snapshot()?;
        let turn = snapshot["turns"]
            .as_array()
            .and_then(|turns| turns.iter().find(|turn| turn["id"] == turn_id))
            .cloned()
            .ok_or("accepted turn is not visible")?;
        match turn["state"].as_str() {
            Some("Succeeded") if snapshot["execution"]["busy"] != true => return Ok(turn),
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
            return Err("retry exceeded the acceptance deadline".into());
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn sha256_file(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    std::fs::read(path)
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .map_err(|error| error.to_string())
}
