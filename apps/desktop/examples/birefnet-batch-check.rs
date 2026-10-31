//! Explicit same-process BiRefNet continuity and duplicate-submission acceptance aid.

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

struct Args {
    root: PathBuf,
    sources: Vec<PathBuf>,
    export_directory: PathBuf,
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut root = None;
    let mut sources = Vec::new();
    let mut export_directory = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--data-dir" => root = Some(PathBuf::from(args.next().ok_or("missing data root")?)),
            "--source" => sources.push(PathBuf::from(args.next().ok_or("missing source path")?)),
            "--export-dir" => {
                export_directory = Some(PathBuf::from(
                    args.next().ok_or("missing export directory")?,
                ))
            }
            _ => return Err(format!("unknown option: {flag}").into()),
        }
    }
    if sources.len() != 3 {
        return Err("--source must be supplied exactly three times".into());
    }
    Ok(Args {
        root: root.ok_or("--data-dir is required")?,
        sources,
        export_directory: export_directory.ok_or("--export-dir is required")?,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args()?;
    std::fs::create_dir_all(&args.export_directory)?;
    let executable = std::env::current_exe()?;
    let supervisor = executable
        .parent()
        .and_then(Path::parent)
        .ok_or("invalid target layout")?
        .join("inference-supervisor");
    let service = Arc::new(StudioService::new(
        AppPaths::resolve(Some(&args.root))?,
        supervisor,
    ));
    if let Some(error) = &service.startup_error {
        return Err(error.clone().into());
    }
    let mut records = Vec::new();
    for (index, source_path) in args.sources.iter().enumerate() {
        let imported = service.import_source(source_path)?;
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
                name: format!("BiRefNet batch {}", index + 1),
                prompt: String::new(),
                params: json!({
                    "modelId": "birefnet-general-epoch-244",
                    "capabilityId": "image.remove-background.birefnet-general-epoch-244.v1",
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
        let submission_key = format!("birefnet-batch-{}", uuid::Uuid::new_v4());
        let request = ConversationSendInput {
            conversation_id: conversation_id.clone(),
            draft_id,
            expected_version: saved.version,
            submission_key: submission_key.clone(),
            prompt: String::new(),
            model_id: "birefnet-general-epoch-244".into(),
            capability_id: Some("image.remove-background.birefnet-general-epoch-244.v1".into()),
            source_asset_id: Some(source_id.clone()),
            aspect: "1:1".into(),
            count: 1,
            seed: String::new(),
        };
        let accepted = service.conversation_send(request.clone())?;
        let duplicate = service.conversation_send(request)?;
        if accepted["created"] != true
            || duplicate["created"] != false
            || accepted["turn"]["id"] != duplicate["turn"]["id"]
        {
            return Err("duplicate submission did not return the original Turn".into());
        }
        let mut busy_request_rejected = false;
        if index == 0 {
            let wait_deadline = Instant::now() + Duration::from_secs(10);
            while !service.snapshot()?["execution"]["busy"]
                .as_bool()
                .unwrap_or(false)
            {
                if Instant::now() > wait_deadline {
                    return Err("BiRefNet execution did not become busy".into());
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            let accepted_draft = accepted["draft"]
                .as_object()
                .ok_or("accepted response has no draft")?;
            let busy_draft = service.save_draft(
                DraftInput {
                    id: accepted_draft["id"]
                        .as_u64()
                        .ok_or("accepted draft has no ID")?,
                    name: "BiRefNet busy request".into(),
                    prompt: String::new(),
                    params: accepted_draft["params"].clone(),
                    collapsed: false,
                    prompt_source_id: None,
                },
                accepted_draft["version"]
                    .as_u64()
                    .ok_or("accepted draft has no version")?,
            )?;
            let busy_result = service.conversation_send(ConversationSendInput {
                conversation_id: conversation_id.clone(),
                draft_id,
                expected_version: busy_draft.version,
                submission_key: format!("birefnet-busy-{}", uuid::Uuid::new_v4()),
                prompt: String::new(),
                model_id: "birefnet-general-epoch-244".into(),
                capability_id: Some("image.remove-background.birefnet-general-epoch-244.v1".into()),
                source_asset_id: Some(source_id.clone()),
                aspect: "1:1".into(),
                count: 1,
                seed: String::new(),
            });
            if busy_result.is_ok() {
                return Err("distinct request was accepted while BiRefNet was busy".into());
            }
            busy_request_rejected = true;
        }
        let turn_id = accepted["turn"]["id"]
            .as_str()
            .ok_or("accepted turn has no ID")?
            .to_owned();
        let started = Instant::now();
        loop {
            let current = service.snapshot()?;
            let turn = current["turns"]
                .as_array()
                .and_then(|turns| turns.iter().find(|turn| turn["id"] == turn_id.as_str()))
                .cloned()
                .ok_or("accepted turn is not visible")?;
            match turn["state"].as_str() {
                Some("Succeeded") => {
                    let primary_id = turn["assetId"]
                        .as_str()
                        .ok_or("successful turn has no primary asset")?;
                    let mask_id = turn["maskAssetId"]
                        .as_str()
                        .ok_or("successful turn has no mask asset")?;
                    let primary = service.read_asset(primary_id)?;
                    let mask = service.read_asset(mask_id)?;
                    let primary_export = args
                        .export_directory
                        .join(format!("continuous-{}.png", index + 1));
                    let mask_export = args
                        .export_directory
                        .join(format!("continuous-{}-mask.png", index + 1));
                    service.export(primary_id, &primary_export, false)?;
                    service.export(mask_id, &mask_export, false)?;
                    records.push(json!({
                        "index": index + 1,
                        "sourceAssetId": source_id,
                        "turnId": turn_id,
                        "jobId": turn["jobId"],
                        "attemptId": turn["attemptId"],
                        "primaryAssetId": primary_id,
                        "maskAssetId": mask_id,
                        "primarySha256": format!("{:x}", Sha256::digest(&primary)),
                        "maskSha256": format!("{:x}", Sha256::digest(&mask)),
                        "primaryExport": primary_export,
                        "maskExport": mask_export,
                        "duplicateCreated": false,
                        "busyRequestRejected": busy_request_rejected,
                        "elapsedMilliseconds": started.elapsed().as_millis() as u64,
                        "execution": current["execution"]
                    }));
                    break;
                }
                Some("Failed" | "Rejected" | "Cancelled" | "Interrupted") => {
                    return Err(format!(
                        "BiRefNet batch task ended as {}: {}",
                        turn["state"],
                        turn["error"].as_str().unwrap_or("no detail")
                    )
                    .into());
                }
                _ => {}
            }
            if started.elapsed() > Duration::from_secs(600) {
                return Err("BiRefNet batch task exceeded ten-minute deadline".into());
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }
    println!(
        "{}",
        json!({
            "status": "passed",
            "configurationKey": "B-R",
            "tasks": records,
            "sameProcess": true,
            "executionAfterCompletion": service.snapshot()?["execution"]
        })
    );
    Ok(())
}
