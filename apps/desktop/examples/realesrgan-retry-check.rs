//! Explicit retry of a cancelled Real-ESRGAN Job for lifecycle acceptance.

use image_studio_desktop::{
    adapters::AppPaths,
    studio::{DraftInput, StudioService},
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
    let mut job_id = None;
    let mut export = None;
    let mut snapshot_only = false;
    let mut draft_source = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--data-dir" => root = Some(PathBuf::from(args.next().ok_or("missing data root")?)),
            "--job-id" => job_id = Some(args.next().ok_or("missing job id")?),
            "--export" => export = Some(PathBuf::from(args.next().ok_or("missing export path")?)),
            "--snapshot-only" => snapshot_only = true,
            "--draft-source" => {
                draft_source = Some(PathBuf::from(
                    args.next().ok_or("missing replacement source")?,
                ))
            }
            _ => return Err(format!("unknown option: {flag}").into()),
        }
    }
    let root = root.ok_or("--data-dir is required")?;
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
    if snapshot_only {
        println!("{}", json!({"snapshot": service.snapshot()?}));
        return Ok(());
    }
    let job_id = job_id.ok_or("--job-id is required")?;
    let export = export.ok_or("--export is required")?;
    if !export.is_absolute() {
        return Err("--export must be absolute".into());
    }
    let replacement_source = if let Some(path) = draft_source {
        let imported = service.import_source(&path)?;
        let snapshot = service.snapshot()?;
        let draft_id = snapshot["activeId"].as_u64().ok_or("missing draft")?;
        let draft = snapshot["drafts"]
            .as_array()
            .and_then(|drafts| drafts.iter().find(|d| d["id"] == draft_id))
            .ok_or("missing active draft")?;
        let mut params = draft["params"].clone();
        params["sourceAssetId"] = imported["assetId"].clone();
        service.save_draft(
            DraftInput {
                id: draft_id,
                name: "Changed next upscale source".into(),
                prompt: String::new(),
                params,
                collapsed: false,
                prompt_source_id: None,
            },
            draft["version"].as_u64().ok_or("missing draft version")?,
        )?;
        Some(imported["assetId"].clone())
    } else {
        None
    };
    let retried = service.retry(&job_id)?;
    let turn_id = retried["turnId"]
        .as_str()
        .ok_or("retried Job has no Turn")?
        .to_owned();
    let started = Instant::now();
    loop {
        let current = service.snapshot()?;
        let turn = current["turns"]
            .as_array()
            .and_then(|turns| turns.iter().find(|turn| turn["id"] == turn_id.as_str()))
            .cloned()
            .ok_or("retried Turn is not visible")?;
        match turn["state"].as_str() {
            Some("Succeeded") => {
                let asset_id = turn["assetId"]
                    .as_str()
                    .ok_or("successful retry has no asset")?;
                let bytes = service.read_asset(asset_id)?;
                service.export(asset_id, &export, false)?;
                let detail = service.conversation_turn_detail(&turn_id)?;
                println!(
                    "{}",
                    json!({
                        "status": "passed",
                        "jobId": job_id,
                        "turnId": turn_id,
                        "assetId": asset_id,
                        "sourceAssetId": turn["sourceAssetId"],
                        "replacementDraftSourceAssetId": replacement_source,
                        "sha256": format!("{:x}", Sha256::digest(&bytes)),
                        "bytes": bytes.len(),
                        "export": export,
                        "attempts": detail["attempts"],
                        "elapsedMilliseconds": started.elapsed().as_millis() as u64
                    })
                );
                return Ok(());
            }
            Some("Failed" | "Rejected" | "Interrupted" | "Cancelled") => {
                return Err(format!("retry ended as {}", turn["state"]).into());
            }
            _ => {}
        }
        if started.elapsed() > Duration::from_secs(600) {
            return Err("Real-ESRGAN retry exceeded ten-minute deadline".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
