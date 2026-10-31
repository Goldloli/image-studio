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
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--data-dir" => root = Some(PathBuf::from(args.next().ok_or("missing data root")?)),
            "--mode" => mode = args.next().ok_or("missing mode")?,
            "--prompt" => prompt = args.next().ok_or("missing prompt")?,
            "--source" => source = Some(PathBuf::from(args.next().ok_or("missing source path")?)),
            "--export" => export = Some(PathBuf::from(args.next().ok_or("missing export path")?)),
            _ => return Err(format!("unknown option: {flag}").into()),
        }
    }
    if !matches!(mode.as_str(), "text" | "edit") {
        return Err("--mode must be text or edit".into());
    }
    let root = root.ok_or("--data-dir is required; use an isolated acceptance root")?;
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
                "seed": "42"
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
        seed: "42".into(),
        submission_key: uuid::Uuid::new_v4().to_string(),
    })?;
    println!("{}", json!({"accepted": accepted}));
    let turn_id = accepted["turn"]["id"]
        .as_str()
        .ok_or("accepted turn has no ID")?
        .to_owned();
    let started = Instant::now();
    loop {
        let current = service.snapshot()?;
        let turn = current["turns"]
            .as_array()
            .and_then(|turns| turns.iter().find(|turn| turn["id"] == turn_id))
            .cloned()
            .ok_or("accepted turn is not visible")?;
        println!(
            "{}",
            json!({
                "turn": turn,
                "execution": current["execution"],
                "elapsed_seconds": started.elapsed().as_secs()
            })
        );
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
        std::thread::sleep(Duration::from_secs(5));
    }
}
