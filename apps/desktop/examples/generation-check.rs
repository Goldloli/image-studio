//! Explicit developer acceptance harness. Never launched by ordinary application startup.
use image_studio_desktop::{
    adapters::AppPaths,
    studio::{StudioService, SubmitInput},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut root = None;
    let mut prompt = None;
    let mut aspect = "1:1".to_string();
    let mut seed = "42".to_string();
    let mut install = false;
    let mut model_id = "z-image-turbo-mflux-v1".to_string();
    let mut cancel_after = None;
    let mut kill_after = None;
    let mut cancel_stage = None;
    let mut retry_job = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--data-dir" => root = Some(PathBuf::from(args.next().ok_or("missing data root")?)),
            "--prompt" => prompt = Some(args.next().ok_or("missing prompt")?),
            "--aspect" => aspect = args.next().ok_or("missing aspect")?,
            "--seed" => seed = args.next().ok_or("missing seed")?,
            "--install" => install = true,
            "--model-id" => model_id = args.next().ok_or("missing model ID")?,
            "--retry-job" => retry_job = Some(args.next().ok_or("missing retry job")?),
            "--cancel-at-stage" => {
                let stage = args.next().ok_or("missing cancel stage")?;
                if !["loading", "sampling", "encoding", "decoding"].contains(&stage.as_str()) {
                    return Err("unsupported cancel stage".into());
                }
                cancel_stage = Some(stage);
            }
            "--cancel-after-seconds" => {
                cancel_after = Some(args.next().ok_or("missing cancel delay")?.parse::<u64>()?)
            }
            "--kill-after-seconds" => {
                kill_after = Some(args.next().ok_or("missing kill delay")?.parse::<u64>()?)
            }
            _ => return Err(format!("unknown option: {flag}").into()),
        }
    }
    let root = root.ok_or("--data-dir is required; this harness never uses default user data")?;
    let executable = std::env::current_exe()?;
    let supervisor = executable
        .parent()
        .and_then(|p| p.parent())
        .ok_or("invalid target layout")?
        .join("inference-supervisor");
    let service = StudioService::new(AppPaths::resolve(Some(&root))?, supervisor);
    if let Some(error) = &service.startup_error {
        return Err(error.clone().into());
    }
    if install {
        service.install(&model_id)?;
        loop {
            let model = service
                .model_views()
                .into_iter()
                .find(|model| model["id"] == model_id)
                .ok_or("registered model view missing")?;
            println!("{}", serde_json::json!({"model":model}));
            if !model["busy"].as_bool().unwrap_or(false) {
                if !model["ready"].as_bool().unwrap_or(false) {
                    return Err("installation did not become ready".into());
                }
                break;
            }
            std::thread::sleep(Duration::from_secs(5));
        }
    }
    if prompt.is_some() && retry_job.is_some() {
        return Err("choose prompt or retry-job".into());
    }
    if prompt.is_some() || retry_job.is_some() {
        let snapshot = service.snapshot()?;
        let job = if let Some(id) = retry_job {
            service.retry(&id)?
        } else {
            service.submit(SubmitInput {
                draft_id: snapshot["activeId"].as_u64().ok_or("no draft")?,
                prompt: prompt.ok_or("missing prompt")?,
                aspect,
                count: 1,
                seed,
                submission_key: uuid::Uuid::new_v4().to_string(),
            })?
        };
        println!("{}", serde_json::json!({"submitted":job}));
        let started = Instant::now();
        let mut cancelled = false;
        loop {
            if kill_after.is_some_and(|s| started.elapsed() >= Duration::from_secs(s)) {
                std::process::exit(73);
            }
            if !cancelled
                && cancel_after.is_some_and(|s| started.elapsed() >= Duration::from_secs(s))
            {
                service.cancel(job["id"].as_str().ok_or("job ID missing")?)?;
                cancelled = true;
            }
            let snapshot = service.snapshot()?;
            if !cancelled && cancel_stage.as_deref() == snapshot["execution"]["stage"].as_str() {
                println!(
                    "{}",
                    serde_json::json!({"acceptance_control":{"action":"cancel","stage":snapshot["execution"]["stage"],"elapsed_seconds":started.elapsed().as_secs_f64()}})
                );
                service.cancel(job["id"].as_str().ok_or("job ID missing")?)?;
                cancelled = true;
            }
            println!(
                "{}",
                serde_json::json!({"execution":snapshot["execution"],"jobs":snapshot["jobs"]})
            );
            if !snapshot["execution"]["busy"].as_bool().unwrap_or(false) {
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    println!("{}", service.snapshot()?);
    Ok(())
}
