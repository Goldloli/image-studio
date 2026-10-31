//! Explicit Klein product-path acceptance harness. Ordinary app startup never invokes it.

use image::{ImageBuffer, Rgb};
use image_studio_desktop::{
    adapters::AppPaths,
    studio::{ConversationSendInput, DraftInput, StudioService},
};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut root = None;
    let mut mode = "text".to_string();
    let mut prompt = "A quiet cobalt glass cube on a warm concrete table".to_string();
    let mut source = None;
    let mut export = None;
    let mut seed = "42".to_string();
    let mut retry_job = None;
    let mut cancel_after = None;
    let mut cancel_stage = None;
    let mut cancel_step = None;
    let mut poll_milliseconds = 5000;
    let mut accept_cancelled = false;
    let mut snapshot_only = false;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--data-dir" => root = Some(PathBuf::from(args.next().ok_or("missing data root")?)),
            "--mode" => mode = args.next().ok_or("missing mode")?,
            "--prompt" => prompt = args.next().ok_or("missing prompt")?,
            "--source" => source = Some(PathBuf::from(args.next().ok_or("missing source path")?)),
            "--export" => export = Some(PathBuf::from(args.next().ok_or("missing export path")?)),
            "--seed" => seed = args.next().ok_or("missing seed")?,
            "--retry-job" => retry_job = Some(args.next().ok_or("missing retry job")?),
            "--cancel-after-seconds" => {
                cancel_after = Some(args.next().ok_or("missing cancel delay")?.parse::<u64>()?)
            }
            "--cancel-at-stage" => {
                let stage = args.next().ok_or("missing cancel stage")?;
                if !["loading", "sampling", "encoding", "decoding"].contains(&stage.as_str()) {
                    return Err("unsupported cancel stage".into());
                }
                cancel_stage = Some(stage);
            }
            "--cancel-at-step" => {
                cancel_step = Some(args.next().ok_or("missing cancel step")?.parse::<u32>()?)
            }
            "--poll-milliseconds" => {
                poll_milliseconds = args.next().ok_or("missing poll interval")?.parse::<u64>()?;
                if !(100..=60_000).contains(&poll_milliseconds) {
                    return Err("poll interval must be 100–60000 milliseconds".into());
                }
            }
            "--accept-cancelled" => accept_cancelled = true,
            "--snapshot-only" => snapshot_only = true,
            _ => return Err(format!("unknown option: {flag}").into()),
        }
    }
    if !matches!(mode.as_str(), "text" | "edit") {
        return Err("--mode must be text or edit".into());
    }
    let root = root.ok_or("--data-dir is required; use an isolated acceptance root")?;
    let export = export
        .map(|path| {
            if path.is_absolute() {
                Ok(path)
            } else {
                std::env::current_dir().map(|cwd| cwd.join(path))
            }
        })
        .transpose()?;
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
    if let Some(job_id) = retry_job {
        let retried = service.retry(&job_id)?;
        println!("{}", json!({"retried": retried}));
        let job_id = retried["id"]
            .as_str()
            .ok_or("retried job has no ID")?
            .to_owned();
        let started = Instant::now();
        let mut cancel_requested = false;
        let mut last_observation = String::new();
        loop {
            let current = service.snapshot()?;
            let job = current["jobs"]
                .as_array()
                .and_then(|jobs| jobs.iter().find(|job| job["id"] == job_id.as_str()))
                .cloned()
                .ok_or("retried job is not visible")?;
            let observation = format!(
                "{}|{}|{}|{}|{}",
                current["execution"]["stage"].as_str().unwrap_or(""),
                current["execution"]["step"],
                current["execution"]["total"],
                job["state"].as_str().unwrap_or(""),
                started.elapsed().as_secs()
            );
            if observation != last_observation {
                println!(
                    "{}",
                    json!({
                        "job": job,
                        "execution": current["execution"],
                        "elapsed_seconds": started.elapsed().as_secs()
                    })
                );
                last_observation = observation;
            }
            if !cancel_requested {
                let by_time = cancel_after
                    .is_some_and(|seconds| started.elapsed() >= Duration::from_secs(seconds));
                let minimum_step = cancel_step.unwrap_or_else(|| {
                    if cancel_stage.as_deref() == Some("sampling") {
                        1
                    } else {
                        0
                    }
                });
                let by_stage = cancel_stage.as_deref() == current["execution"]["stage"].as_str()
                    && current["execution"]["step"].as_u64().unwrap_or(0)
                        >= u64::from(minimum_step);
                if by_time || by_stage {
                    service.cancel(&job_id)?;
                    cancel_requested = true;
                    println!(
                        "{}",
                        json!({
                            "cancelRequested": true,
                            "jobId": job_id,
                            "trigger": if by_stage { "stage" } else { "time" },
                            "stage": current["execution"]["stage"]
                        })
                    );
                }
            }
            match job["state"].as_str() {
                Some("Succeeded") => {
                    let asset_id = job["assetId"]
                        .as_str()
                        .ok_or("successful retried job has no asset")?;
                    if let Some(path) = &export {
                        service.export(asset_id, path, false)?;
                        println!("{}", json!({"exported": path, "assetId": asset_id}));
                    }
                    return Ok(());
                }
                Some("Cancelled") if accept_cancelled => {
                    println!("{}", json!({"cancelled": true, "jobId": job_id}));
                    return Ok(());
                }
                Some("Failed" | "Cancelled" | "Interrupted") => {
                    return Err(format!(
                        "retried Klein job ended as {}: {}",
                        job["state"],
                        job["error"].as_str().unwrap_or("no detail")
                    )
                    .into());
                }
                _ => {}
            }
            if started.elapsed() > Duration::from_secs(3600) {
                return Err("Klein retry exceeded one-hour acceptance deadline".into());
            }
            std::thread::sleep(Duration::from_millis(poll_milliseconds));
        }
    }
    let source_id = if mode == "edit" {
        let source = match source {
            Some(path) => path,
            None => {
                let path = root.join("klein-edit-source.png");
                ImageBuffer::from_fn(640, 640, |x, y| {
                    Rgb([
                        (32 + x / 8).min(255) as u8,
                        (48 + y / 8).min(255) as u8,
                        96u8,
                    ])
                })
                .save(&path)?;
                path
            }
        };
        let imported = service.import_source(&source)?;
        Some(
            imported["assetId"]
                .as_str()
                .ok_or("source import did not return an asset ID")?
                .to_owned(),
        )
    } else {
        None
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
    let capability = if source_id.is_some() {
        "image.edit.flux2-klein-4b.v1"
    } else {
        "image.text-to-image.flux2-klein-4b.v1"
    };
    let saved = service.save_draft(
        DraftInput {
            id: draft_id,
            name: if source_id.is_some() {
                "Klein edit check"
            } else {
                "Klein text check"
            }
            .into(),
            prompt: prompt.clone(),
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
    )?;
    let accepted = service.conversation_send(ConversationSendInput {
        conversation_id: conversation_id.clone(),
        draft_id,
        expected_version: saved.version,
        prompt,
        model_id: "flux2-klein-4b-distilled".into(),
        capability_id: Some(capability.into()),
        source_asset_id: source_id,
        aspect: "1:1".into(),
        count: 1,
        seed,
        submission_key: uuid::Uuid::new_v4().to_string(),
    })?;
    println!("{}", json!({"accepted": accepted}));
    let turn_id = accepted["turn"]["id"]
        .as_str()
        .ok_or("accepted turn has no ID")?
        .to_owned();
    let started = Instant::now();
    let mut cancel_requested = false;
    let mut last_observation = String::new();
    loop {
        let current = service.snapshot()?;
        let turn = current["turns"]
            .as_array()
            .and_then(|turns| turns.iter().find(|turn| turn["id"] == turn_id))
            .cloned()
            .ok_or("accepted turn is not visible")?;
        let observation = format!(
            "{}|{}|{}|{}|{}",
            current["execution"]["stage"].as_str().unwrap_or(""),
            current["execution"]["step"],
            current["execution"]["total"],
            turn["state"].as_str().unwrap_or(""),
            started.elapsed().as_secs()
        );
        if observation != last_observation {
            println!(
                "{}",
                json!({
                    "turn": turn,
                    "execution": current["execution"],
                    "elapsed_seconds": started.elapsed().as_secs()
                })
            );
            last_observation = observation;
        }
        if !cancel_requested {
            let by_time = cancel_after
                .is_some_and(|seconds| started.elapsed() >= Duration::from_secs(seconds));
            let minimum_step = cancel_step.unwrap_or_else(|| {
                if cancel_stage.as_deref() == Some("sampling") {
                    1
                } else {
                    0
                }
            });
            let by_stage = cancel_stage.as_deref() == current["execution"]["stage"].as_str()
                && current["execution"]["step"].as_u64().unwrap_or(0) >= u64::from(minimum_step);
            if by_time || by_stage {
                service.conversation_cancel_turn(&turn_id)?;
                cancel_requested = true;
                println!(
                    "{}",
                    json!({
                        "cancelRequested": true,
                        "turnId": turn_id,
                        "trigger": if by_stage { "stage" } else { "time" },
                        "stage": current["execution"]["stage"]
                    })
                );
            }
        }
        match turn["state"].as_str() {
            Some("Succeeded") => {
                let asset_id = turn["assetId"]
                    .as_str()
                    .ok_or("successful turn has no asset")?;
                if let Some(path) = export {
                    service.export(asset_id, &path, false)?;
                    println!("{}", json!({"exported": path, "assetId": asset_id}));
                }
                return Ok(());
            }
            Some("Cancelled") if accept_cancelled => {
                println!("{}", json!({"cancelled": true, "jobId": turn["jobId"]}));
                return Ok(());
            }
            Some("Failed" | "Rejected" | "Cancelled" | "Interrupted") => {
                return Err(format!(
                    "Klein turn ended as {}: {}",
                    turn["state"],
                    turn["error"].as_str().unwrap_or("no detail")
                )
                .into());
            }
            _ => {}
        }
        if started.elapsed() > Duration::from_secs(3600) {
            return Err("Klein turn exceeded one-hour acceptance deadline".into());
        }
        std::thread::sleep(Duration::from_millis(poll_milliseconds));
    }
}
