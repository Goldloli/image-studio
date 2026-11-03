//! Versioned product feature registry. Availability is derived from model facts at query time.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[cfg(not(windows))]
const CATALOG: &str = include_str!("../../../../resources/feature-catalog/catalog.json");
#[cfg(windows)]
const CATALOG: &str = include_str!("../../../../resources/windows-foundation/feature-catalog.json");

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogDocument {
    schema_version: u32,
    reviewed_date: String,
    entries: Vec<RegisteredFeature>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegisteredFeature {
    feature_id: String,
    title: String,
    category: String,
    summary: String,
    description: String,
    inputs: Vec<String>,
    outputs: Vec<String>,
    product_status: String,
    scope: String,
    action: Option<String>,
    model_id: Option<String>,
    dependencies: Vec<Dependency>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Dependency {
    kind: String,
    label: String,
}

#[derive(Clone, Debug)]
pub struct FeatureCatalog {
    reviewed_date: String,
    entries: Vec<RegisteredFeature>,
}

impl FeatureCatalog {
    pub fn load() -> Result<Self, String> {
        let document: CatalogDocument =
            serde_json::from_str(CATALOG).map_err(|error| format!("功能目录无法解析：{error}"))?;
        if document.schema_version != 1 {
            return Err(format!("功能目录版本不受支持：{}", document.schema_version));
        }
        let mut ids = BTreeSet::new();
        for entry in &document.entries {
            if entry.feature_id.is_empty()
                || entry.feature_id.len() > 128
                || !entry
                    .feature_id
                    .bytes()
                    .all(|value| value.is_ascii_alphanumeric() || b"-_.:".contains(&value))
            {
                return Err("功能目录包含无效标识".into());
            }
            if !ids.insert(&entry.feature_id) {
                return Err(format!("功能目录包含重复标识：{}", entry.feature_id));
            }
            if !matches!(entry.product_status.as_str(), "implemented" | "planned") {
                return Err(format!("功能 {} 的产品状态无效", entry.feature_id));
            }
            if entry.inputs.is_empty() || entry.outputs.is_empty() {
                return Err(format!("功能 {} 缺少输入或输出", entry.feature_id));
            }
            if entry.product_status == "implemented"
                && entry.action.as_deref() != Some("open-composer")
            {
                return Err(format!("已接入功能 {} 缺少统一提交入口", entry.feature_id));
            }
            if entry.product_status == "planned" && entry.action.is_some() {
                return Err(format!("计划功能 {} 不得登记可执行动作", entry.feature_id));
            }
        }
        Ok(Self {
            reviewed_date: document.reviewed_date,
            entries: document.entries,
        })
    }

    /// `model_catalog` is the host projection; family names or plan documents never imply readiness.
    pub fn query(&self, model_catalog: &Value, generation_busy: bool) -> Value {
        let entries = self
            .entries
            .iter()
            .map(|entry| project_entry(entry, model_catalog, generation_busy))
            .collect::<Vec<_>>();
        let available = entries
            .iter()
            .filter(|entry| entry["state"] == "available")
            .count();
        let planned = entries
            .iter()
            .filter(|entry| entry["state"] == "planned")
            .count();
        json!({
            "schemaVersion": 1,
            "reviewedDate": self.reviewed_date,
            "entries": entries,
            "counts": {
                "total": self.entries.len(),
                "available": available,
                "planned": planned
            }
        })
    }
}

fn project_entry(entry: &RegisteredFeature, model_catalog: &Value, generation_busy: bool) -> Value {
    let model = entry.model_id.as_deref().and_then(|id| {
        model_catalog
            .get("entries")
            .and_then(Value::as_array)
            .and_then(|entries| entries.iter().find(|model| model["id"] == id))
    });
    let (state, state_label, reason, can_start) = if entry.product_status == "planned" {
        (
            "planned",
            "计划中",
            Some("当前版本未接入执行器".to_string()),
            false,
        )
    } else if generation_busy {
        (
            "busy",
            "正在执行",
            Some("当前已有生成或清理在进行".to_string()),
            false,
        )
    } else {
        match model {
            None => (
                "blocked",
                "目录异常",
                Some("所需模型未出现在权威模型目录中".to_string()),
                false,
            ),
            Some(model)
                if model["executable"].as_bool() == Some(true)
                    && model["hardwareEligible"].as_bool() != Some(false) =>
            {
                ("available", "可开始", None, true)
            }
            Some(model) if model["componentsReady"].as_bool() != Some(true) => (
                "model-missing",
                "模型未安装",
                Some(
                    model["error"]
                        .as_str()
                        .unwrap_or("所需本地模型尚未安装完成")
                        .to_string(),
                ),
                false,
            ),
            Some(model) if model["runtimeReady"].as_bool() != Some(true) => (
                "runtime-not-ready",
                "运行环境未就绪",
                Some("本地运行环境尚未通过检查".to_string()),
                false,
            ),
            Some(model) if model["hardwareEligible"].as_bool() == Some(false) => (
                "hardware-ineligible",
                "设备不满足门槛",
                Some(
                    model["hardwareReason"]
                        .as_str()
                        .unwrap_or("当前设备不满足该型号的显存或内存门槛")
                        .to_string(),
                ),
                false,
            ),
            Some(_) => (
                "device-not-ready",
                "设备未就绪",
                Some("本地设备尚未完成实机验证".to_string()),
                false,
            ),
        }
    };
    json!({
        "featureId": entry.feature_id,
        "title": entry.title,
        "category": entry.category,
        "summary": entry.summary,
        "description": entry.description,
        "inputs": entry.inputs,
        "outputs": entry.outputs,
        "productStatus": entry.product_status,
        "scope": entry.scope,
        "action": entry.action,
        "modelId": entry.model_id,
        "dependencies": entry.dependencies,
        "state": state,
        "stateLabel": state_label,
        "blockingReason": reason,
        "canStart": can_start
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_model_id() -> &'static str {
        #[cfg(windows)]
        return studio_core::generation::Z_IMAGE_WINDOWS_MODEL;
        #[cfg(not(windows))]
        "z-image-turbo-mflux-v1"
    }

    fn klein_model_id() -> &'static str {
        #[cfg(windows)]
        return studio_core::generation::KLEIN_WINDOWS_MODEL;
        #[cfg(not(windows))]
        "flux2-klein-4b-distilled"
    }

    #[test]
    fn registry_only_marks_implemented_model_with_real_ready_facts() {
        let catalog = FeatureCatalog::load().unwrap();
        let model_catalog = json!({
            "entries": [{
                "id": text_model_id(),
                "family": "Z-Image",
                "executable": false,
                "componentsReady": true,
                "runtimeReady": false,
                "hardwareEligible": true,
                "error": null
            }, {
                "id": klein_model_id(),
                "family": "FLUX.2",
                "executable": false,
                "componentsReady": true,
                "runtimeReady": false,
                "hardwareEligible": true,
                "error": null
            }]
        });
        let result = catalog.query(&model_catalog, false);
        let text = result["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["featureId"] == "text-to-image")
            .unwrap();
        let edit = result["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["featureId"] == "image-edit")
            .unwrap();
        assert_eq!(text["state"], "runtime-not-ready");
        assert_eq!(text["canStart"], false);
        assert_eq!(edit["state"], "runtime-not-ready");
        assert_eq!(edit["canStart"], false);
        assert_eq!(edit["action"], "open-composer");
    }

    #[test]
    fn ready_projection_uses_catalog_not_family_keywords() {
        let catalog = FeatureCatalog::load().unwrap();
        let model_catalog = json!({
            "entries": [{
                "id": text_model_id(),
                "family": "unrelated keyword",
                "executable": true,
                "componentsReady": true,
                "runtimeReady": true,
                "hardwareEligible": true,
                "error": null
            }, {
                "id": klein_model_id(),
                "family": "FLUX.2",
                "executable": true,
                "componentsReady": true,
                "runtimeReady": true,
                "hardwareEligible": true,
                "error": null
            }]
        });
        let result = catalog.query(&model_catalog, false);
        assert_eq!(result["entries"][0]["state"], "available");
        assert_eq!(result["entries"][0]["canStart"], true);
    }

    #[test]
    fn hardware_ineligible_model_blocks_the_feature_with_reason() {
        let catalog = FeatureCatalog::load().unwrap();
        let model_catalog = json!({
            "entries": [{
                "id": klein_model_id(),
                "family": "FLUX.2",
                "executable": true,
                "componentsReady": true,
                "runtimeReady": true,
                "hardwareEligible": false,
                "hardwareReason": "当前设备不满足硬件门槛：显存 ≥ 12 GB（当前 8 GB）。安装与生成入口已禁用。",
                "error": null
            }]
        });
        let result = catalog.query(&model_catalog, false);
        let edit = result["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["featureId"] == "image-edit")
            .unwrap();
        assert_eq!(edit["state"], "hardware-ineligible");
        assert_eq!(edit["stateLabel"], "设备不满足门槛");
        assert_eq!(edit["canStart"], false);
        assert!(
            edit["blockingReason"]
                .as_str()
                .unwrap()
                .contains("硬件门槛")
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_projection_exposes_four_implemented_features() {
        let catalog = FeatureCatalog::load().unwrap();
        let result = catalog.query(
            &json!({
                "entries": [{
                    "id": studio_core::generation::Z_IMAGE_WINDOWS_MODEL,
                    "executable": true,
                    "componentsReady": true,
                    "runtimeReady": true,
                    "hardwareEligible": true,
                    "error": null
                }, {
                    "id": studio_core::generation::KLEIN_WINDOWS_MODEL,
                    "executable": false,
                    "componentsReady": false,
                    "runtimeReady": false,
                    "hardwareEligible": true,
                    "error": null
                }, {
                    "id": studio_core::generation::BIREFNET_WINDOWS_MODEL,
                    "executable": false,
                    "componentsReady": false,
                    "runtimeReady": false,
                    "hardwareEligible": true,
                    "error": null
                }, {
                    "id": studio_core::generation::REALESRGAN_WINDOWS_MODEL,
                    "executable": false,
                    "componentsReady": false,
                    "runtimeReady": false,
                    "hardwareEligible": true,
                    "error": null
                }]
            }),
            false,
        );
        assert_eq!(result["counts"]["total"], 4);
        assert_eq!(result["counts"]["planned"], 0);
        let features = result["entries"].as_array().unwrap();
        let expected = [
            (
                "text-to-image",
                studio_core::generation::Z_IMAGE_WINDOWS_MODEL,
            ),
            ("image-edit", studio_core::generation::KLEIN_WINDOWS_MODEL),
            (
                "remove-background",
                studio_core::generation::BIREFNET_WINDOWS_MODEL,
            ),
            ("upscale", studio_core::generation::REALESRGAN_WINDOWS_MODEL),
        ];
        for (feature_id, model_id) in expected {
            let feature = features
                .iter()
                .find(|entry| entry["featureId"] == feature_id)
                .unwrap();
            assert_eq!(feature["productStatus"], "implemented", "{feature_id}");
            assert_eq!(feature["action"], "open-composer", "{feature_id}");
            assert_eq!(feature["modelId"], model_id, "{feature_id}");
            assert_eq!(
                feature["canStart"],
                feature["state"] == "available",
                "{feature_id}"
            );
        }
        let text = features
            .iter()
            .find(|entry| entry["featureId"] == "text-to-image")
            .unwrap();
        assert_eq!(text["state"], "available");
        assert_eq!(text["canStart"], true);
        let edit = features
            .iter()
            .find(|entry| entry["featureId"] == "image-edit")
            .unwrap();
        assert_eq!(edit["state"], "model-missing");
        assert_eq!(edit["canStart"], false);
    }
}
