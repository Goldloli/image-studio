//! Explicit Real-ESRGAN product-path acceptance harness. Ordinary app startup never invokes it.

use image_studio_desktop::{
    adapters::AppPaths,
    studio::{ConversationSendInput, DraftInput, StudioService},
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut root = None;
    let mut source = None;
    let mut export_directory = None;
    let mut cancel_stage = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--data-dir" => root = Some(PathBuf::from(args.next().ok_or("missing data root")?)),
            "--source" => source = Some(PathBuf::from(args.next().ok_or("missing source path")?)),
            "--export-dir" => {
                export_directory = Some(PathBuf::from(
                    args.next().ok_or("missing export directory")?,
                ))
            }
            "--cancel-stage" => {
                let stage = args.next().ok_or("missing cancel stage")?;
                if !matches!(stage.as_str(), "loading" | "inference" | "decoding") {
                    return Err("unsupported Real-ESRGAN cancel stage".into());
                }
                cancel_stage = Some(stage);
            }
            _ => return Err(format!("unknown option: {flag}").into()),
        }
    }
    let root = root.ok_or("--data-dir is required")?;
    let source = source.ok_or("--source is required")?;
    let export_directory = export_directory.ok_or("--export-dir is required")?;
    std::fs::create_dir_all(&export_directory)?;
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
    let imported = service.import_source(&source)?;
    let source_id = imported["assetId"]
        .as_str()
        .ok_or("source import did not return an asset ID")?
        .to_owned();
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
    let saved = service.save_draft(
        DraftInput {
            id: draft_id,
            name: "Real-ESRGAN check".into(),
            prompt: String::new(),
            params: json!({
                "modelId": "realesrgan-x4plus",
                "capabilityId": "image.upscale.realesrgan-x4plus.v1",
                "sourceAssetId": source_id,
                "aspect": "1:1",
                "count": 1,
                "seed": ""
            }),
            collapsed: false,
            prompt_source_id: None,
        },
        expected_version,
    )?;
    let accepted = service.conversation_send(ConversationSendInput {
        conversation_id,
        draft_id,
        expected_version: saved.version,
        submission_key: format!("realesrgan-{}", uuid::Uuid::new_v4()),
        prompt: String::new(),
        model_id: "realesrgan-x4plus".into(),
        capability_id: Some("image.upscale.realesrgan-x4plus.v1".into()),
        source_asset_id: Some(source_id),
        aspect: "1:1".into(),
        count: 1,
        seed: String::new(),
    })?;
    let turn_id = accepted["turn"]["id"]
        .as_str()
        .ok_or("accepted turn has no ID")?
        .to_owned();
    let mut stages = Vec::new();
    let mut cancel_requested = false;
    let started = Instant::now();
    loop {
        let current = service.snapshot()?;
        let turn = current["turns"]
            .as_array()
            .and_then(|turns| turns.iter().find(|turn| turn["id"] == turn_id.as_str()))
            .cloned()
            .ok_or("accepted turn is not visible")?;
        let stage = current["execution"]["stage"]
            .as_str()
            .unwrap_or("")
            .to_owned();
        if !stage.is_empty() && stages.last() != Some(&stage) {
            stages.push(stage.clone());
            println!(
                "{}",
                json!({
                    "turnId": turn_id,
                    "stage": stage,
                    "elapsedMilliseconds": started.elapsed().as_millis() as u64
                })
            );
        }
        if !cancel_requested
            && cancel_stage.as_deref().is_some_and(|wanted| {
                wanted == current["execution"]["stage"].as_str().unwrap_or("")
            })
        {
            service.conversation_cancel_turn(&turn_id)?;
            cancel_requested = true;
        }
        match turn["state"].as_str() {
            Some("Succeeded") => {
                let asset_id = turn["assetId"]
                    .as_str()
                    .ok_or("successful Real-ESRGAN turn has no asset")?;
                let bytes = service.read_asset(asset_id)?;
                let export = export_directory.join("realesrgan-primary.png");
                service.export(asset_id, &export, false)?;
                if export.is_relative() || std::fs::read(&export)? != bytes {
                    return Err("export differs from committed Real-ESRGAN asset".into());
                }
                println!(
                    "{}",
                    json!({
                        "status": "passed",
                        "turnId": turn_id,
                        "sourceAssetId": turn["sourceAssetId"],
                        "assetId": asset_id,
                        "width": turn["width"],
                        "height": turn["height"],
                        "sha256": format!("{:x}", Sha256::digest(&bytes)),
                        "bytes": bytes.len(),
                        "export": export,
                        "elapsedMilliseconds": started.elapsed().as_millis() as u64,
                        "stageSequence": stages,
                        "execution": current["execution"]
                    })
                );
                return Ok(());
            }
            Some("Cancelled") if cancel_requested => {
                println!(
                    "{}",
                    json!({
                        "status": "cancelled",
                        "turnId": turn_id,
                        "attempts": current["jobs"].as_array().and_then(|jobs| jobs.iter().find(|job| job["id"] == turn["jobId"])).map(|job| job["attemptId"].clone()).unwrap_or(serde_json::Value::Null),
                        "assetId": turn["assetId"],
                        "elapsedMilliseconds": started.elapsed().as_millis() as u64,
                        "stageSequence": stages,
                        "execution": current["execution"]
                    })
                );
                return Ok(());
            }
            Some("Failed" | "Rejected" | "Interrupted") => {
                return Err(format!(
                    "Real-ESRGAN turn ended as {}: {}",
                    turn["state"],
                    turn["error"].as_str().unwrap_or("no detail")
                )
                .into());
            }
            _ => {}
        }
        if started.elapsed() > Duration::from_secs(3600) {
            return Err("Real-ESRGAN acceptance exceeded one-hour deadline".into());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
