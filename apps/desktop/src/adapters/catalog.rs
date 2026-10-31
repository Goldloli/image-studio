//! Host-owned product catalog. Installation and execution facts are attached at query time.
use super::installation::InstallationSnapshot;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const CATALOG: &str = include_str!("../../../../resources/model-catalog/catalog.json");

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogDocument {
    schema_version: u32,
    reviewed_date: String,
    entries: Vec<RegisteredModel>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegisteredModel {
    id: String,
    display_name: String,
    variant: String,
    family: String,
    summary: String,
    description: String,
    capabilities: Vec<Capability>,
    product_status: String,
    runtime: String,
    license: String,
    source: String,
    revision: String,
    model_bytes: Option<u64>,
    known_artifact_bytes: Option<u64>,
    requirements: Vec<String>,
    exclusions: Vec<String>,
    verified_devices: Vec<String>,
    recipe: String,
    install_id: Option<String>,
    installer_supported: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Capability {
    id: String,
    label: String,
}

#[derive(Clone, Debug)]
pub struct ModelCatalog {
    reviewed_date: String,
    entries: Vec<RegisteredModel>,
}

impl ModelCatalog {
    pub fn load() -> Result<Self, String> {
        let document: CatalogDocument =
            serde_json::from_str(CATALOG).map_err(|error| format!("模型目录无法解析：{error}"))?;
        if document.schema_version != 1 {
            return Err(format!("模型目录版本不受支持：{}", document.schema_version));
        }
        let mut ids = BTreeSet::new();
        for entry in &document.entries {
            validate_id(&entry.id)?;
            if !ids.insert(&entry.id) {
                return Err(format!("模型目录包含重复标识：{}", entry.id));
            }
            if !matches!(entry.product_status.as_str(), "implemented" | "planned") {
                return Err(format!("模型 {} 的产品状态无效", entry.id));
            }
            if entry.installer_supported != entry.install_id.is_some() {
                return Err(format!("模型 {} 的安装能力与安装标识不一致", entry.id));
            }
            if entry.capabilities.is_empty() {
                return Err(format!("模型 {} 缺少能力登记", entry.id));
            }
        }
        Ok(Self {
            reviewed_date: document.reviewed_date,
            entries: document.entries,
        })
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|entry| entry.id == id)
    }

    pub fn executable(&self, id: &str) -> bool {
        self.entries.iter().any(|entry| {
            entry.id == id && entry.product_status == "implemented" && entry.installer_supported
        })
    }

    pub fn supports(&self, id: &str, capability: &str) -> bool {
        self.entries.iter().any(|entry| {
            entry.id == id
                && entry.product_status == "implemented"
                && entry.capabilities.iter().any(|item| item.id == capability)
        })
    }

    pub fn entries(&self) -> Vec<Value> {
        self.entries
            .iter()
            .map(|entry| {
                json!({
                    "id": entry.id,
                    "displayName": entry.display_name,
                    "family": entry.family,
                    "productStatus": entry.product_status,
                    "capabilities": entry.capabilities,
                    "runtime": entry.runtime
                })
            })
            .collect()
    }

    pub fn query(
        &self,
        installations: &[InstallationSnapshot],
        installation_error: Option<&str>,
    ) -> Result<Value, String> {
        let entries = self
            .entries
            .iter()
            .map(|entry| project_entry(entry, installations, installation_error))
            .collect::<Vec<_>>();
        let supported = entries
            .iter()
            .filter(|entry| entry["productStatus"] == "implemented")
            .count();
        let installed = entries
            .iter()
            .filter(|entry| entry["componentsReady"].as_bool() == Some(true))
            .count();
        let executable = entries
            .iter()
            .filter(|entry| entry["executable"].as_bool() == Some(true))
            .count();
        let planned = entries
            .iter()
            .filter(|entry| entry["productStatus"] == "planned")
            .count();
        let runtime_ready = entries
            .iter()
            .filter(|entry| entry["runtimeReady"].as_bool() == Some(true))
            .count();
        Ok(json!({
            "schemaVersion": 1,
            "reviewedDate": self.reviewed_date,
            "entries": entries,
            "counts": {
                "total": self.entries.len(),
                "supported": supported,
                "installed": installed,
                "executable": executable,
                "planned": planned,
                "runtimeReady": runtime_ready
            }
        }))
    }
}

fn project_entry(
    entry: &RegisteredModel,
    installations: &[InstallationSnapshot],
    installation_error: Option<&str>,
) -> Value {
    let installed = entry.install_id.as_ref().and_then(|id| {
        installations.iter().find(|snapshot| {
            snapshot.install_id == *id
                && matches!(
                    snapshot.phase.as_str(),
                    "ready"
                        | "preparing"
                        | "downloading"
                        | "verifying"
                        | "installing_runtime"
                        | "probing"
                        | "unavailable"
                        | "failed"
                        | "cancelled"
                        | "interrupted"
                )
        })
    });
    let busy = installed.is_some_and(|snapshot| {
        matches!(
            snapshot.phase.as_str(),
            "preparing" | "downloading" | "verifying" | "installing_runtime" | "probing"
        )
    });
    let executable = installed.is_some_and(|snapshot| {
        snapshot.phase == "ready"
            && snapshot.model_ready
            && snapshot.runtime_ready
            && snapshot.device_verified
    });
    let status_error = if entry.installer_supported {
        installed
            .and_then(|snapshot| snapshot.error.clone())
            .or_else(|| installation_error.map(str::to_owned))
    } else {
        None
    };
    let install_phase = installed
        .map(|snapshot| snapshot.phase.clone())
        .unwrap_or_else(|| {
            if entry.installer_supported {
                if !installations.is_empty() {
                    "not_installed".into()
                } else {
                    "failed".into()
                }
            } else {
                "not_supported".into()
            }
        });
    let received_bytes = installed.map_or(0, |snapshot| snapshot.received_bytes);
    let install_total_bytes = installed.map_or(0, |snapshot| snapshot.total_bytes);
    let runtime_bytes = installed.map_or(0, |snapshot| snapshot.runtime_bytes);
    let required_bytes = installed.map_or(0, |snapshot| snapshot.required_bytes);
    json!({
        "id": entry.id,
        "displayName": entry.display_name,
        "variant": entry.variant,
        "family": entry.family,
        "summary": entry.summary,
        "description": entry.description,
        "capabilities": entry.capabilities,
        "productStatus": entry.product_status,
        "runtime": entry.runtime,
        "license": entry.license,
        "source": entry.source,
        "revision": entry.revision,
        "modelBytes": entry.model_bytes,
        "knownArtifactBytes": entry.known_artifact_bytes,
        "requirements": entry.requirements,
        "exclusions": entry.exclusions,
        "verifiedDevices": entry.verified_devices,
        "recipe": entry.recipe,
        "installId": entry.install_id,
        "installerSupported": entry.installer_supported,
        "installPhase": install_phase,
        "componentsReady": installed.is_some_and(|snapshot| snapshot.model_ready),
        "runtimeReady": installed.is_some_and(|snapshot| snapshot.runtime_ready),
        "deviceVerified": installed.is_some_and(|snapshot| snapshot.device_verified),
        "executable": executable,
        "busy": busy,
        "receivedBytes": received_bytes,
        "installTotalBytes": install_total_bytes,
        "runtimeBytes": runtime_bytes,
        "requiredBytes": required_bytes,
        "error": status_error,
        "canInstall": entry.installer_supported && !busy && !executable,
        "canContinue": entry.installer_supported
            && matches!(install_phase.as_str(), "cancelled" | "interrupted" | "unavailable" | "failed"),
        "canStop": busy,
        "canUse": executable
    })
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
    {
        return Err(format!("模型标识无效：{id}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z_snapshot(phase: &str) -> InstallationSnapshot {
        InstallationSnapshot {
            install_id: "z-image-turbo-mflux-v1".into(),
            phase: phase.into(),
            received_bytes: if phase == "ready" { 100 } else { 20 },
            total_bytes: 100,
            model_bytes: 80,
            runtime_bytes: 20,
            required_bytes: 300,
            error: None,
            runtime_ready: phase == "ready",
            model_ready: phase == "ready",
            device_verified: phase == "ready",
        }
    }

    #[test]
    fn catalog_contains_exactly_the_first_four_targets() {
        let catalog = ModelCatalog::load().unwrap();
        let query = catalog.query(&[], Some("安装器不可用")).unwrap();
        assert_eq!(query["counts"]["total"], 4);
        assert_eq!(query["counts"]["supported"], 4);
        assert_eq!(query["counts"]["planned"], 0);
        let ids = query["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["id"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert!(ids.contains(&"z-image-turbo-mflux-v1"));
        assert!(ids.contains(&"birefnet-general-epoch-244"));
        assert!(ids.contains(&"realesrgan-x4plus"));
    }

    #[test]
    fn install_states_are_joined_by_install_id() {
        let catalog = ModelCatalog::load().unwrap();
        let mut klein = z_snapshot("not_installed");
        klein.install_id = "flux2-klein-4b-distilled".into();
        klein.model_bytes = 15_980_131_745;
        let query = catalog.query(&[z_snapshot("ready"), klein], None).unwrap();
        let entries = query["entries"].as_array().unwrap();
        let z = entries
            .iter()
            .find(|entry| entry["id"] == "z-image-turbo-mflux-v1")
            .unwrap();
        let klein = entries
            .iter()
            .find(|entry| entry["id"] == "flux2-klein-4b-distilled")
            .unwrap();
        assert_eq!(z["executable"], true);
        assert_eq!(klein["executable"], false);
        assert_eq!(klein["installPhase"], "not_installed");
        assert_eq!(query["counts"]["installed"], 1);
    }

    #[test]
    fn planned_models_never_borrow_z_image_install_state() {
        let catalog = ModelCatalog::load().unwrap();
        let query = catalog.query(&[z_snapshot("ready")], None).unwrap();
        let entries = query["entries"].as_array().unwrap();
        assert_eq!(query["counts"]["installed"], 1);
        assert_eq!(query["counts"]["executable"], 1);
        for entry in entries
            .iter()
            .filter(|entry| entry["productStatus"] == "planned")
        {
            assert_eq!(entry["executable"], false);
            assert_eq!(entry["installerSupported"], false);
            assert_eq!(entry["canInstall"], false);
        }
    }

    #[test]
    fn active_install_is_exposed_without_fake_completion() {
        let catalog = ModelCatalog::load().unwrap();
        let query = catalog.query(&[z_snapshot("verifying")], None).unwrap();
        let z = &query["entries"][0];
        assert_eq!(z["installPhase"], "verifying");
        assert_eq!(z["busy"], true);
        assert_eq!(z["executable"], false);
        assert_eq!(z["canStop"], true);
    }
}
