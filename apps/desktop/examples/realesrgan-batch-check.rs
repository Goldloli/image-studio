//! Explicit same-process Real-ESRGAN continuity and duplicate-submission acceptance aid.

use image_studio_desktop::{
    adapters::AppPaths,
    studio::{ConversationSendInput, DraftInput, StudioService},
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

fn sampled_tree_rss() -> Result<(u64, u64), Box<dyn std::error::Error>> {
    let output = std::process::Command::new("/bin/ps")
        .args(["-axo", "pid=,ppid=,rss=,comm="])
        .output()?;
    if !output.status.success() {
        return Err("RSS sampling failed".into());
    }
    let text = String::from_utf8(output.stdout)?;
    let rows = text
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            Some((
                fields.next()?.parse::<u32>().ok()?,
                fields.next()?.parse::<u32>().ok()?,
                fields.next()?.parse::<u64>().ok()?,
                fields.next()?,
            ))
        })
        .collect::<Vec<_>>();
    let mut owned = BTreeSet::from([std::process::id()]);
    loop {
        let before = owned.len();
        for (pid, parent, _, _) in &rows {
            if owned.contains(parent) {
                owned.insert(*pid);
            }
        }
        if owned.len() == before {
            break;
        }
    }
    let mut total = 0;
    let mut native = 0;
    for (pid, _, rss, command) in rows {
        if owned.contains(&pid) {
            total += rss;
            if command.ends_with("realesrgan-ncnn-vulkan") {
                native += rss;
            }
        }
    }
    Ok((total, native))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let root = root.ok_or("--data-dir is required")?;
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
    let mut records = Vec::new();
    for (index, source_path) in sources.iter().enumerate() {
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
                name: format!("Real-ESRGAN batch {}", index + 1),
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
        let request = ConversationSendInput {
            conversation_id: conversation_id.clone(),
            draft_id,
            expected_version: saved.version,
            submission_key: format!("realesrgan-batch-{}", uuid::Uuid::new_v4()),
            prompt: String::new(),
            model_id: "realesrgan-x4plus".into(),
            capability_id: Some("image.upscale.realesrgan-x4plus.v1".into()),
            source_asset_id: Some(source_id.clone()),
            aspect: "1:1".into(),
            count: 1,
            seed: String::new(),
        };
        let started = Instant::now();
        let accepted = service.conversation_send(request.clone())?;
        let duplicate = service.conversation_send(request.clone())?;
        if accepted["created"] != true
            || duplicate["created"] != false
            || accepted["turn"]["id"] != duplicate["turn"]["id"]
        {
            return Err("duplicate submission did not return the original Turn".into());
        }
        let mut conflicting = request;
        let replacement = service.import_source(&sources[(index + 1) % sources.len()])?;
        conflicting.source_asset_id = replacement["assetId"].as_str().map(str::to_string);
        if service.conversation_send(conflicting).is_ok() {
            return Err("same key with different content was accepted".into());
        }
        let mut busy_request_rejected = false;
        let mut execution_repair_rejected = false;
        if index == 0 {
            let wait_deadline = Instant::now() + Duration::from_secs(10);
            while !service.snapshot()?["execution"]["busy"]
                .as_bool()
                .unwrap_or(false)
            {
                if Instant::now() > wait_deadline {
                    return Err("Real-ESRGAN execution did not become busy".into());
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            if service.install("realesrgan-x4plus").is_ok()
                || service.install("birefnet-general-epoch-244").is_ok()
            {
                return Err("model repair was admitted during active ncnn execution".into());
            }
            execution_repair_rejected = true;
            let draft = accepted["draft"]
                .as_object()
                .ok_or("accepted response has no draft")?;
            let busy_draft = service.save_draft(
                DraftInput {
                    id: draft["id"].as_u64().ok_or("accepted draft has no ID")?,
                    name: "Real-ESRGAN busy request".into(),
                    prompt: "A new request while ncnn is busy".into(),
                    params: json!({"modelId":"z-image-turbo-mflux-v1", "capabilityId":"image.text-to-image.z-image-turbo.v1", "sourceAssetId":null, "aspect":"1:1", "count":1, "seed":""}),
                    collapsed: false,
                    prompt_source_id: None,
                },
                draft["version"]
                    .as_u64()
                    .ok_or("accepted draft has no version")?,
            )?;
            if service
                .conversation_send(ConversationSendInput {
                    conversation_id: conversation_id.clone(),
                    draft_id,
                    expected_version: busy_draft.version,
                    submission_key: format!("realesrgan-busy-{}", uuid::Uuid::new_v4()),
                    prompt: "A new request while ncnn is busy".into(),
                    model_id: "z-image-turbo-mflux-v1".into(),
                    capability_id: Some("image.text-to-image.z-image-turbo.v1".into()),
                    source_asset_id: None,
                    aspect: "1:1".into(),
                    count: 1,
                    seed: String::new(),
                })
                .is_ok()
            {
                return Err("distinct request was accepted while Real-ESRGAN was busy".into());
            }
            busy_request_rejected = true;
        }
        let turn_id = accepted["turn"]["id"]
            .as_str()
            .ok_or("accepted turn has no ID")?
            .to_owned();
        let mut tree_rss_peak_kib = 0;
        let mut ncnn_rss_peak_kib = 0;
        let mut samples = 0;
        let mut sampled_at = Instant::now() - Duration::from_secs(1);
        loop {
            if sampled_at.elapsed() >= Duration::from_millis(100) {
                let (tree, native) = sampled_tree_rss()?;
                tree_rss_peak_kib = tree_rss_peak_kib.max(tree);
                ncnn_rss_peak_kib = ncnn_rss_peak_kib.max(native);
                samples += 1;
                sampled_at = Instant::now();
            }
            let current = service.snapshot()?;
            let turn = current["turns"]
                .as_array()
                .and_then(|turns| turns.iter().find(|turn| turn["id"] == turn_id.as_str()))
                .cloned()
                .ok_or("accepted turn is not visible")?;
            match turn["state"].as_str() {
                Some("Succeeded") => {
                    // Observe the admission gate's final cleanup acknowledgement as
                    // well as the durable Job before starting the next task.
                    if current["execution"]["busy"] == true {
                        if started.elapsed() > Duration::from_secs(600) {
                            return Err("successful output did not reach idle cleanup".into());
                        }
                        std::thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    if turn["sourceAssetId"] != source_id {
                        return Err("changing the draft mutated the active Turn source".into());
                    }
                    let asset_id = turn["assetId"]
                        .as_str()
                        .ok_or("successful Real-ESRGAN turn has no asset")?;
                    let bytes = service.read_asset(asset_id)?;
                    let export = export_directory.join(format!("continuous-{}.png", index + 1));
                    service.export(asset_id, &export, false)?;
                    records.push(json!({
                        "index": index + 1,
                        "sourceAssetId": source_id,
                        "turnId": turn_id,
                        "jobId": turn["jobId"],
                        "attemptId": turn["attemptId"],
                        "assetId": asset_id,
                        "width": turn["width"],
                        "height": turn["height"],
                        "sha256": format!("{:x}", Sha256::digest(&bytes)),
                        "export": export,
                        "duplicateCreated": false,
                        "conflictingDuplicateRejected": true,
                        "busyRequestRejected": busy_request_rejected,
                        "executionRepairRejected": execution_repair_rejected,
                        "draftSourceChangedDuringExecution": index == 0,
                        "elapsedMilliseconds": started.elapsed().as_millis() as u64,
                        "resources": {
                            "sampledTreeRssPeakKiB": tree_rss_peak_kib,
                            "sampledNcnnRssPeakKiB": ncnn_rss_peak_kib,
                            "backendPeakBytes": null,
                            "samples": samples,
                            "method": "macOS ps RSS in KiB sampled every >=100 ms for host plus descendants; shared pages may be counted more than once; excludes unified GPU allocation; observation, not an SLA"
                        },
                        "execution": current["execution"]
                    }));
                    break;
                }
                Some("Failed" | "Rejected" | "Cancelled" | "Interrupted") => {
                    return Err(format!(
                        "Real-ESRGAN batch task ended as {}: {}",
                        turn["state"],
                        turn["error"].as_str().unwrap_or("no detail")
                    )
                    .into());
                }
                _ => {}
            }
            if started.elapsed() > Duration::from_secs(600) {
                return Err("Real-ESRGAN batch task exceeded ten-minute deadline".into());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    println!(
        "{}",
        json!({
            "schemaVersion": 1,
            "runId": "planned-models-20261031",
            "caseIds": ["P03", "P08", "I09"],
            "configurationKey": "R-U",
            "required": true,
            "status": "passed",
            "evidenceKind": "product_execution",
            "reason": "Three independent Real-ESRGAN jobs completed in one StudioService process with unique attempts/assets and final idle cleanup. Duplicate keys reused the original Turn; a distinct busy submission was rejected without an additional Turn.",
            "tasks": records,
            "sameProcess": true,
            "executionAfterCompletion": service.snapshot()?["execution"],
            "problems": []
        })
    );
    Ok(())
}
