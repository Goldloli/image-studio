//! Explicit native-default install helper for acceptance; ordinary startup never invokes it.

use image_studio_desktop::{adapters::AppPaths, studio::StudioService};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut data_dir = None;
    let mut model_id = "flux2-klein-4b-distilled".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--data-dir" => data_dir = Some(PathBuf::from(args.next().ok_or("missing data root")?)),
            "--model-id" => model_id = args.next().ok_or("missing model id")?,
            _ => return Err(format!("unknown option: {flag}").into()),
        }
    }
    let executable = std::env::current_exe()?;
    let supervisor = executable
        .parent()
        .and_then(Path::parent)
        .ok_or("invalid target layout")?
        .join("inference-supervisor");
    let paths = AppPaths::resolve(data_dir.as_deref())?;
    let service = StudioService::new(paths.clone(), supervisor);
    if let Some(error) = &service.startup_error {
        return Err(error.clone().into());
    }
    service.install(&model_id)?;
    loop {
        let model = service
            .model_views()
            .into_iter()
            .find(|model| model["id"] == model_id.as_str())
            .ok_or("registered model view is missing")?;
        println!(
            "{}",
            json!({
                "dataRoot": paths.data,
                "cacheRoot": paths.cache,
                "model": model
            })
        );
        if !model["busy"].as_bool().unwrap_or(false) {
            if model["ready"].as_bool() != Some(true)
                || model["deviceVerified"].as_bool() != Some(true)
            {
                return Err("model installation did not become fully ready".into());
            }
            return Ok(());
        }
        thread::sleep(Duration::from_secs(2));
    }
}
