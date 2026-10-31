//! Explicit native-default install helper for acceptance; ordinary startup never invokes it.

use image_studio_desktop::{adapters::AppPaths, studio::StudioService};
use serde_json::json;
use std::{path::Path, thread, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executable = std::env::current_exe()?;
    let supervisor = executable
        .parent()
        .and_then(Path::parent)
        .ok_or("invalid target layout")?
        .join("inference-supervisor");
    let paths = AppPaths::resolve(None)?;
    let service = StudioService::new(paths.clone(), supervisor);
    if let Some(error) = &service.startup_error {
        return Err(error.clone().into());
    }
    service.install("flux2-klein-4b-distilled")?;
    loop {
        let model = service
            .model_views()
            .into_iter()
            .find(|model| model["id"] == "flux2-klein-4b-distilled")
            .ok_or("Klein model view is missing")?;
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
                return Err("Klein default installation did not become fully ready".into());
            }
            return Ok(());
        }
        thread::sleep(Duration::from_secs(2));
    }
}
