//! Host-owned product catalog. Installation and execution facts are attached at query time.
use super::device::{DeviceFacts, HardwareGate};
use super::installation::InstallationSnapshot;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[cfg(not(windows))]
const CATALOG: &str = include_str!("../../../../resources/model-catalog/catalog.json");
#[cfg(windows)]
const CATALOG: &str = include_str!("../../../../resources/windows-foundation/model-catalog.json");

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
    /// Hardware thresholds from design.md section 10.2. Optional so the macOS catalog keeps its
    /// existing shape; Windows entries always carry the full metadata set and `load` enforces it.
    #[serde(default)]
    min_vram_gb: Option<u32>,
    #[serde(default)]
    min_ram_gb: Option<u32>,
    #[serde(default)]
    quantization: Option<String>,
    #[serde(default)]
    license_notice: Option<String>,
    #[serde(default)]
    windows_status: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Capability {
    id: String,
    label: String,
}

/// Static hardware/license metadata registered for one model. It is exposed to the settings and
/// model views so the UI can separate "已安装/可执行" from "设备满足".
#[derive(Clone, Debug, Default)]
pub struct ModelMetadata {
    pub min_vram_gb: Option<u32>,
    pub min_ram_gb: Option<u32>,
    pub quantization: Option<String>,
    pub license_notice: Option<String>,
    pub windows_status: Option<String>,
}

/// The static metadata plus the device comparison for one model.
#[derive(Clone, Debug)]
pub struct ModelFacts {
    pub metadata: ModelMetadata,
    pub hardware: HardwareGate,
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
            let has_thresholds = entry.min_vram_gb.is_some() || entry.min_ram_gb.is_some();
            let complete = entry.min_vram_gb.is_some()
                && entry.min_ram_gb.is_some()
                && entry.quantization.is_some()
                && entry.license_notice.is_some()
                && entry.windows_status.is_some();
            if has_thresholds && !complete {
                return Err(format!("模型 {} 的硬件门槛元数据不完整", entry.id));
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

    /// Static hardware/license facts for a registered model, or `None` when the id is unknown.
    pub fn model_facts(&self, id: &str, device: &DeviceFacts) -> Option<ModelFacts> {
        let entry = self.entries.iter().find(|entry| entry.id == id)?;
        Some(ModelFacts {
            metadata: entry_metadata(entry),
            hardware: device.gate(entry.min_vram_gb, entry.min_ram_gb),
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
        device: &DeviceFacts,
    ) -> Result<Value, String> {
        let entries = self
            .entries
            .iter()
            .map(|entry| project_entry(entry, installations, installation_error, device))
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
        let hardware_eligible = entries
            .iter()
            .filter(|entry| entry["hardwareEligible"].as_bool() == Some(true))
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
                "runtimeReady": runtime_ready,
                "hardwareEligible": hardware_eligible
            }
        }))
    }
}

fn entry_metadata(entry: &RegisteredModel) -> ModelMetadata {
    ModelMetadata {
        min_vram_gb: entry.min_vram_gb,
        min_ram_gb: entry.min_ram_gb,
        quantization: entry.quantization.clone(),
        license_notice: entry.license_notice.clone(),
        windows_status: entry.windows_status.clone(),
    }
}

fn project_entry(
    entry: &RegisteredModel,
    installations: &[InstallationSnapshot],
    installation_error: Option<&str>,
    device: &DeviceFacts,
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
    let metadata = entry_metadata(entry);
    // A required threshold that is unmet (or cannot be proven) disables install and execution
    // entry points with an actionable Chinese reason; it never silently passes.
    let hardware = device.gate(entry.min_vram_gb, entry.min_ram_gb);
    let hardware_eligible = hardware.eligible;
    // The projection is merged from several `json!` chunks: one literal with the full field set
    // would exceed serde_json's default macro recursion limit.
    let mut projected = json!({
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
        "canInstall": entry.installer_supported && !busy && !executable && hardware_eligible,
        "canContinue": entry.installer_supported
            && hardware_eligible
            && matches!(
                install_phase.as_str(),
                "cancelled" | "interrupted" | "unavailable" | "failed"
            ),
        "canStop": busy,
        "canUse": executable && hardware_eligible
    });
    extend_projection(
        &mut projected,
        json!({
            "minVramGb": metadata.min_vram_gb,
            "minRamGb": metadata.min_ram_gb,
            "quantization": metadata.quantization,
            "licenseNotice": metadata.license_notice,
            "windowsStatus": metadata.windows_status,
            "hardwareEligible": hardware_eligible,
            "hardwareReason": hardware.reason
        }),
    );
    projected
}

/// Appends `extra` object fields to `target` when both are JSON objects. The projection is a flat
/// object, so this only exists to keep each `json!` literal below the macro recursion limit.
fn extend_projection(target: &mut Value, extra: Value) {
    if let (Value::Object(target), Value::Object(extra)) = (target, extra) {
        target.extend(extra);
    }
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
        #[cfg(windows)]
        let install_id = "z-image-turbo-diffusers-cuda-nf4-v1";
        #[cfg(not(windows))]
        let install_id = "z-image-turbo-mflux-v1";
        InstallationSnapshot {
            install_id: install_id.into(),
            phase: phase.into(),
            received_bytes: if phase == "ready" { 100 } else { 20 },
            total_bytes: 100,
            model_bytes: if cfg!(windows) { 32_848_305_533 } else { 80 },
            runtime_bytes: if cfg!(windows) { 200 } else { 20 },
            required_bytes: if cfg!(windows) { 1_000 } else { 300 },
            error: None,
            runtime_ready: phase == "ready",
            model_ready: phase == "ready",
            device_verified: phase == "ready",
        }
    }

    /// The certified target machine: RTX 3080 Ti 12 GB + 32 GB RAM.
    fn target_device() -> DeviceFacts {
        #[cfg(windows)]
        return DeviceFacts {
            vram_gb: Some(12),
            ram_gb: 32,
        };
        #[cfg(not(windows))]
        DeviceFacts::default()
    }

    fn entry<'a>(query: &'a Value, id: &str) -> &'a Value {
        query["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == id)
            .unwrap_or_else(|| panic!("missing catalog entry {id}"))
    }

    #[test]
    fn catalog_contains_the_exact_supported_targets() {
        let catalog = ModelCatalog::load().unwrap();
        let query = catalog
            .query(&[], Some("安装器不可用"), &target_device())
            .unwrap();
        assert_eq!(query["counts"]["total"], 7);
        #[cfg(windows)]
        {
            assert_eq!(query["counts"]["supported"], 7);
            assert_eq!(query["counts"]["planned"], 0);
            assert_eq!(query["counts"]["hardwareEligible"], 7);
            assert_eq!(
                query["entries"][0]["id"],
                "z-image-turbo-diffusers-cuda-nf4-v1"
            );
            assert_eq!(
                query["entries"][0]["runtime"],
                "Diffusers CUDA NF4 v1 · Python 3.12.10 / PyTorch 2.10.0 / CUDA 12.6"
            );
        }
        #[cfg(not(windows))]
        {
            assert_eq!(query["counts"]["supported"], 7);
            assert_eq!(query["counts"]["planned"], 0);
        }
        let ids = query["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["id"].as_str().unwrap())
            .collect::<Vec<_>>();
        #[cfg(windows)]
        {
            for id in [
                "z-image-turbo-diffusers-cuda-nf4-v1",
                "flux2-klein-4b-distilled-windows-v1",
                "birefnet-general-epoch-244-windows-v1",
                "realesrgan-x4plus-windows-v1",
                "qwen-image-2.1-windows-v1",
                "qwen-image-2.1-turbo-windows-v1",
                "moss-image-sft-windows-v1",
            ] {
                assert!(ids.contains(&id), "missing Windows model {id}");
            }
        }
        #[cfg(not(windows))]
        {
            assert!(ids.contains(&"z-image-turbo-mflux-v1"));
            assert!(ids.contains(&"birefnet-general-epoch-244"));
            assert!(ids.contains(&"realesrgan-x4plus"));
            assert!(ids.contains(&"qwen-image-2.1-mflux-v1"));
            assert!(ids.contains(&"qwen-image-2.1-turbo-mflux-v1"));
            assert!(ids.contains(&"moss-image-sft-mflux-v1"));
        }
    }

    #[test]
    fn install_states_are_joined_by_install_id() {
        let catalog = ModelCatalog::load().unwrap();
        let mut klein = z_snapshot("not_installed");
        #[cfg(windows)]
        {
            klein.install_id = "flux2-klein-4b-distilled-windows-v1".into();
        }
        #[cfg(not(windows))]
        {
            klein.install_id = "flux2-klein-4b-distilled".into();
        }
        klein.model_bytes = 15_980_131_745;
        let query = catalog
            .query(&[z_snapshot("ready"), klein], None, &target_device())
            .unwrap();
        let z = entry(
            &query,
            if cfg!(windows) {
                "z-image-turbo-diffusers-cuda-nf4-v1"
            } else {
                "z-image-turbo-mflux-v1"
            },
        );
        let klein = entry(
            &query,
            if cfg!(windows) {
                "flux2-klein-4b-distilled-windows-v1"
            } else {
                "flux2-klein-4b-distilled"
            },
        );
        assert_eq!(z["executable"], true);
        assert_eq!(klein["executable"], false);
        #[cfg(windows)]
        assert_eq!(klein["installPhase"], "not_installed");
        #[cfg(not(windows))]
        assert_eq!(klein["installPhase"], "not_installed");
        assert_eq!(query["counts"]["installed"], 1);
    }

    #[test]
    fn entries_never_borrow_z_image_install_state() {
        let catalog = ModelCatalog::load().unwrap();
        let query = catalog
            .query(&[z_snapshot("ready")], None, &target_device())
            .unwrap();
        assert_eq!(query["counts"]["installed"], 1);
        assert_eq!(query["counts"]["executable"], 1);
        let z_id = if cfg!(windows) {
            "z-image-turbo-diffusers-cuda-nf4-v1"
        } else {
            "z-image-turbo-mflux-v1"
        };
        for item in query["entries"].as_array().unwrap() {
            if item["id"] == z_id {
                continue;
            }
            assert_eq!(item["executable"], false);
            assert_eq!(item["canUse"], false);
            assert_ne!(item["installPhase"], "ready");
        }
    }

    #[test]
    fn active_install_is_exposed_without_fake_completion() {
        let catalog = ModelCatalog::load().unwrap();
        let query = catalog
            .query(&[z_snapshot("verifying")], None, &target_device())
            .unwrap();
        let z = &query["entries"][0];
        assert_eq!(z["installPhase"], "verifying");
        assert_eq!(z["busy"], true);
        assert_eq!(z["executable"], false);
        assert_eq!(z["canStop"], true);
    }

    #[cfg(windows)]
    #[test]
    fn windows_catalog_registers_every_parity_identity_with_metadata() {
        let catalog = ModelCatalog::load().unwrap();
        let query = catalog
            .query(&[], Some("安装器不可用"), &target_device())
            .unwrap();
        let mut capability_pairs = 0;
        for item in query["entries"].as_array().unwrap() {
            assert_eq!(item["productStatus"], "implemented");
            assert_eq!(item["installerSupported"], true);
            assert_eq!(item["installId"], item["id"]);
            assert!(item["quantization"].is_string());
            assert!(item["licenseNotice"].is_string());
            assert!(item["windowsStatus"].is_string());
            assert!(item["minVramGb"].is_u64());
            assert!(item["minRamGb"].is_u64());
            capability_pairs += item["capabilities"].as_array().unwrap().len();
        }
        assert_eq!(capability_pairs, 8);

        let z = entry(&query, "z-image-turbo-diffusers-cuda-nf4-v1");
        assert_eq!(
            (z["minVramGb"].as_u64(), z["minRamGb"].as_u64()),
            (Some(12), Some(32))
        );
        let klein = entry(&query, "flux2-klein-4b-distilled-windows-v1");
        assert_eq!(
            (klein["minVramGb"].as_u64(), klein["minRamGb"].as_u64()),
            (Some(12), Some(32))
        );
        let birefnet = entry(&query, "birefnet-general-epoch-244-windows-v1");
        assert_eq!(
            (
                birefnet["minVramGb"].as_u64(),
                birefnet["minRamGb"].as_u64()
            ),
            (Some(0), Some(16))
        );
        assert_eq!(birefnet["quantization"], "float32");
        assert_eq!(birefnet["licenseNotice"], "MIT");
        let realesrgan = entry(&query, "realesrgan-x4plus-windows-v1");
        assert_eq!(
            (
                realesrgan["minVramGb"].as_u64(),
                realesrgan["minRamGb"].as_u64()
            ),
            (Some(4), Some(8))
        );
        assert!(
            realesrgan["licenseNotice"]
                .as_str()
                .unwrap()
                .contains("BSD-3-Clause")
        );
        for id in [
            "qwen-image-2.1-windows-v1",
            "qwen-image-2.1-turbo-windows-v1",
        ] {
            let qwen = entry(&query, id);
            assert_eq!(
                (qwen["minVramGb"].as_u64(), qwen["minRamGb"].as_u64()),
                (Some(12), Some(32))
            );
            assert!(qwen["licenseNotice"].as_str().unwrap().contains("非商业"));
        }
        let moss = entry(&query, "moss-image-sft-windows-v1");
        assert!(
            moss["licenseNotice"]
                .as_str()
                .unwrap()
                .contains("Apache-2.0")
        );
    }

    #[cfg(windows)]
    #[test]
    fn unsatisfied_hardware_disables_entry_points_with_a_chinese_reason() {
        let catalog = ModelCatalog::load().unwrap();
        // An 8 GB / 16 GB machine: the staged large models must be blocked while the CPU route
        // and the small ncnn route stay eligible.
        let device = DeviceFacts {
            vram_gb: Some(8),
            ram_gb: 16,
        };
        let query = catalog.query(&[], None, &device).unwrap();
        for id in [
            "z-image-turbo-diffusers-cuda-nf4-v1",
            "flux2-klein-4b-distilled-windows-v1",
            "qwen-image-2.1-windows-v1",
            "qwen-image-2.1-turbo-windows-v1",
            "moss-image-sft-windows-v1",
        ] {
            let item = entry(&query, id);
            assert_eq!(item["hardwareEligible"], false, "{id}");
            assert_eq!(item["canInstall"], false, "{id}");
            assert_eq!(item["canUse"], false, "{id}");
            let reason = item["hardwareReason"].as_str().unwrap();
            assert!(reason.contains("硬件门槛"), "{id}: {reason}");
            assert!(reason.contains("安装与生成入口已禁用"), "{id}: {reason}");
        }
        let birefnet = entry(&query, "birefnet-general-epoch-244-windows-v1");
        assert_eq!(birefnet["hardwareEligible"], true);
        assert_eq!(birefnet["canInstall"], true);
        let realesrgan = entry(&query, "realesrgan-x4plus-windows-v1");
        assert_eq!(realesrgan["hardwareEligible"], true);
        assert_eq!(realesrgan["canInstall"], true);
    }

    #[cfg(windows)]
    #[test]
    fn unknown_device_facts_are_not_silently_accepted() {
        let catalog = ModelCatalog::load().unwrap();
        let query = catalog
            .query(
                &[],
                None,
                &DeviceFacts {
                    vram_gb: None,
                    ram_gb: 0,
                },
            )
            .unwrap();
        let klein = entry(&query, "flux2-klein-4b-distilled-windows-v1");
        assert_eq!(klein["hardwareEligible"], false);
        let reason = klein["hardwareReason"].as_str().unwrap();
        assert!(reason.contains("无法确认"));
        assert!(reason.contains("nvidia-smi"));
        // The certified target machine passes everything.
        let satisfied = catalog.query(&[], None, &target_device()).unwrap();
        for item in satisfied["entries"].as_array().unwrap() {
            assert_eq!(item["hardwareEligible"], true, "{}", item["id"]);
        }
        let z = entry(&satisfied, "z-image-turbo-diffusers-cuda-nf4-v1");
        assert_eq!(z["canInstall"], true);
    }

    #[cfg(not(windows))]
    #[test]
    fn macos_catalog_has_no_unverifiable_hardware_gates() {
        let catalog = ModelCatalog::load().unwrap();
        let query = catalog
            .query(
                &[],
                None,
                &DeviceFacts {
                    vram_gb: None,
                    ram_gb: 0,
                },
            )
            .unwrap();
        for item in query["entries"].as_array().unwrap() {
            assert_eq!(item["hardwareEligible"], true, "{}", item["id"]);
            assert_eq!(item["minVramGb"], Value::Null);
            assert_eq!(item["minRamGb"], Value::Null);
        }
    }
}
