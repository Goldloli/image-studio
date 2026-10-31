//! Versioned, model-specific generation and image-processing recipes.

use crate::{
    ContractError,
    contracts::{ExecutionPlan, InputBinding, UserRequest},
    domain::{AssetKind, AssetRef, IdempotencyKey, ModelVariantId, PlanId, ProjectId, RequestId},
    generation::{
        MAX_PROMPT_BYTES, Z_IMAGE_CAPABILITY, Z_IMAGE_MODEL, validate_z_image_plan,
        validate_z_image_request,
    },
    require,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

pub const KLEIN_MODEL: &str = "flux2-klein-4b-distilled";
pub const KLEIN_TEXT_CAPABILITY: &str = "image.text-to-image.flux2-klein-4b.v1";
pub const KLEIN_EDIT_CAPABILITY: &str = "image.edit.flux2-klein-4b.v1";
pub const BIREFNET_MODEL: &str = "birefnet-general-epoch-244";
pub const BIREFNET_CAPABILITY: &str = "image.remove-background.birefnet-general-epoch-244.v1";
pub const REALESRGAN_MODEL: &str = "realesrgan-x4plus";
pub const REALESRGAN_CAPABILITY: &str = "image.upscale.realesrgan-x4plus.v1";

pub const SOURCE_ROLE: &str = "source";
pub const PRIMARY_ROLE: &str = "primary";
pub const MASK_ROLE: &str = "mask";

pub const MAX_SOURCE_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_SOURCE_EDGE: u32 = 4096;
pub const MAX_SOURCE_PIXELS: u64 = 4 * 1024 * 1024;
pub const MAX_TOOL_OUTPUT_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_UPSCALE_OUTPUT_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_UPSCALE_OUTPUT_PIXELS: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Recipe {
    ZImageTurbo,
    KleinText,
    KleinEdit,
    BiRefNet,
    RealEsrgan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputRole {
    Primary,
    Mask,
}

impl OutputRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Primary => PRIMARY_ROLE,
            Self::Mask => MASK_ROLE,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceFacts {
    pub width: u32,
    pub height: u32,
    pub has_transparent_pixels: bool,
}

#[derive(Clone, Debug)]
pub struct SourceAsset {
    pub asset: AssetRef,
    pub facts: SourceFacts,
}

#[derive(Clone, Debug)]
pub struct KleinInput {
    pub request_id: RequestId,
    pub project_id: ProjectId,
    pub idempotency_key: IdempotencyKey,
    pub plan_id: PlanId,
    pub prompt: String,
    pub seed: Option<u32>,
    pub source: Option<SourceAsset>,
}

#[derive(Clone, Debug)]
pub struct BiRefNetInput {
    pub request_id: RequestId,
    pub project_id: ProjectId,
    pub idempotency_key: IdempotencyKey,
    pub plan_id: PlanId,
    pub source: SourceAsset,
}

#[derive(Clone, Debug)]
pub struct FrozenRecipe {
    pub request: UserRequest,
    pub plan: ExecutionPlan,
}

impl SourceFacts {
    pub fn validate(self) -> Result<(), ContractError> {
        require(
            self.width > 0 && self.height > 0,
            "source image dimensions must be positive",
        )?;
        require(
            u64::from(self.width) * u64::from(self.height) <= MAX_SOURCE_PIXELS,
            "source image exceeds 4 megapixels",
        )
    }
}

impl Recipe {
    pub fn from_request(request: &UserRequest) -> Result<Self, ContractError> {
        let recipe = match (
            request.model_variant_id.as_str(),
            request.capability.capability_id.as_str(),
        ) {
            (Z_IMAGE_MODEL, Z_IMAGE_CAPABILITY) => Self::ZImageTurbo,
            (KLEIN_MODEL, KLEIN_TEXT_CAPABILITY) => Self::KleinText,
            (KLEIN_MODEL, KLEIN_EDIT_CAPABILITY) => Self::KleinEdit,
            (BIREFNET_MODEL, BIREFNET_CAPABILITY) => Self::BiRefNet,
            (REALESRGAN_MODEL, REALESRGAN_CAPABILITY) => Self::RealEsrgan,
            _ => {
                return Err(ContractError::InvalidValue(
                    "unsupported model and capability combination".into(),
                ));
            }
        };
        Ok(recipe)
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::ZImageTurbo => "z-image-turbo-q4-v1",
            Self::KleinText => "klein4b-text-v1",
            Self::KleinEdit => "klein4b-edit-v1",
            Self::BiRefNet => "birefnet-general-cpu-v1",
            Self::RealEsrgan => "realesrgan-x4plus-t128-v1",
        }
    }

    pub fn outputs(self) -> &'static [OutputRole] {
        const PRIMARY: &[OutputRole] = &[OutputRole::Primary];
        const PRIMARY_AND_MASK: &[OutputRole] = &[OutputRole::Primary, OutputRole::Mask];
        match self {
            Self::BiRefNet => PRIMARY_AND_MASK,
            _ => PRIMARY,
        }
    }

    pub fn accepts_output(self, role: OutputRole, kind: AssetKind) -> bool {
        match (self, role) {
            (Self::ZImageTurbo, OutputRole::Primary) => {
                matches!(kind, AssetKind::ImageRgb | AssetKind::ImageRgba)
            }
            (Self::KleinText | Self::KleinEdit | Self::RealEsrgan, OutputRole::Primary) => {
                matches!(kind, AssetKind::ImageRgb)
            }
            (Self::BiRefNet, OutputRole::Primary) => matches!(kind, AssetKind::ImageRgba),
            (Self::BiRefNet, OutputRole::Mask) => matches!(kind, AssetKind::Mask),
            _ => false,
        }
    }

    pub fn max_output_bytes(self, role: OutputRole) -> Option<u64> {
        match (self, role) {
            (Self::RealEsrgan, OutputRole::Primary) => Some(MAX_UPSCALE_OUTPUT_BYTES),
            (Self::ZImageTurbo | Self::KleinText | Self::KleinEdit, OutputRole::Primary)
            | (Self::BiRefNet, OutputRole::Primary | OutputRole::Mask) => {
                Some(MAX_TOOL_OUTPUT_BYTES)
            }
            _ => None,
        }
    }

    pub fn validate_request(
        self,
        request: &UserRequest,
        source: Option<SourceFacts>,
    ) -> Result<(), ContractError> {
        if self == Self::ZImageTurbo {
            require(source.is_none(), "Z-Image does not accept a source image")?;
            return validate_z_image_request(request);
        }
        request.validate()?;
        let expected_capability = match self {
            Self::KleinText => KLEIN_TEXT_CAPABILITY,
            Self::KleinEdit => KLEIN_EDIT_CAPABILITY,
            Self::BiRefNet => BIREFNET_CAPABILITY,
            Self::RealEsrgan => REALESRGAN_CAPABILITY,
            Self::ZImageTurbo => unreachable!(),
        };
        require(
            request.capability.capability_id == expected_capability,
            "capability does not match recipe",
        )?;
        match self {
            Self::KleinText => {
                require(
                    request.capability.instruction_required
                        && request.capability.input_roles.is_empty()
                        && request.inputs.is_empty()
                        && source.is_none(),
                    "Klein text-to-image requires text and no source image",
                )?;
                validate_klein_instruction(request)?;
                require(
                    request.requested_output.width == 1024
                        && request.requested_output.height == 1024
                        && request.requested_output.count == 1,
                    "Klein text-to-image requires one 1024x1024 image",
                )?;
                validate_seed_parameters(request)?;
            }
            Self::KleinEdit => {
                validate_single_source(request, source)?;
                let source = source.expect("validated above");
                require(
                    source.width >= 16 && source.height >= 16,
                    "Klein edit source edges must be at least 16 pixels",
                )?;
                require(
                    !source.has_transparent_pixels,
                    "Klein edit does not accept effective transparency",
                )?;
                validate_klein_instruction(request)?;
                require(
                    request.requested_output.width == 1024
                        && request.requested_output.height == 1024
                        && request.requested_output.count == 1,
                    "Klein edit requires one 1024x1024 image",
                )?;
                validate_seed_parameters(request)?;
            }
            Self::BiRefNet => {
                validate_single_source(request, source)?;
                let source = source.expect("validated above");
                require(
                    !request.capability.instruction_required && request.instruction.is_empty(),
                    "BiRefNet does not accept an instruction",
                )?;
                require(
                    request.parameters.is_empty(),
                    "BiRefNet does not accept request parameters",
                )?;
                require(
                    request.requested_output.width == source.width
                        && request.requested_output.height == source.height
                        && request.requested_output.count == 1,
                    "BiRefNet output must preserve normalized source dimensions",
                )?;
            }
            Self::RealEsrgan => {
                validate_single_source(request, source)?;
                let source = source.expect("validated above");
                require(
                    !source.has_transparent_pixels,
                    "Real-ESRGAN does not accept effective transparency",
                )?;
                require(
                    source.width <= 1024
                        && source.height <= 1024
                        && u64::from(source.width) * u64::from(source.height) <= 1024 * 1024,
                    "Real-ESRGAN input must be at most 1024x1024 and one megapixel",
                )?;
                let width = source
                    .width
                    .checked_mul(4)
                    .ok_or_else(|| ContractError::InvalidValue("upscale width overflow".into()))?;
                let height = source
                    .height
                    .checked_mul(4)
                    .ok_or_else(|| ContractError::InvalidValue("upscale height overflow".into()))?;
                require(
                    !request.capability.instruction_required && request.instruction.is_empty(),
                    "Real-ESRGAN does not accept an instruction",
                )?;
                require(
                    request.parameters.is_empty(),
                    "Real-ESRGAN does not accept request parameters",
                )?;
                require(
                    request.requested_output.width == width
                        && request.requested_output.height == height
                        && request.requested_output.count == 1,
                    "Real-ESRGAN output must be exactly four times the source dimensions",
                )?;
            }
            Self::ZImageTurbo => unreachable!(),
        }
        Ok(())
    }

    pub fn validate_plan(
        self,
        request: &UserRequest,
        plan: &ExecutionPlan,
        source: Option<SourceFacts>,
    ) -> Result<(), ContractError> {
        if self == Self::ZImageTurbo {
            return validate_z_image_plan(request, plan);
        }
        self.validate_request(request, source)?;
        plan.validate_against(request)?;
        require(
            plan.model_variant_id == request.model_variant_id
                && plan.effective_output == request.requested_output
                && plan.adjustments.is_empty(),
            "recipe cannot substitute model, output, or apply implicit adjustments",
        )?;
        let expected = match self {
            Self::KleinText | Self::KleinEdit => {
                let seed = request
                    .parameters
                    .get("seed")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let effective_seed = match seed {
                    serde_json::Value::Null => plan
                        .effective_parameters
                        .get("seed")
                        .cloned()
                        .ok_or_else(|| {
                            ContractError::InvalidValue(
                                "Klein plan requires a resolved seed".into(),
                            )
                        })?,
                    serde_json::Value::Number(value) => serde_json::Value::Number(value),
                    _ => {
                        return Err(ContractError::InvalidValue(
                            "Klein seed must be null or u32".into(),
                        ));
                    }
                };
                require(
                    effective_seed
                        .as_u64()
                        .is_some_and(|value| u32::try_from(value).is_ok()),
                    "Klein effective seed must be u32",
                )?;
                BTreeMap::from([
                    ("seed".into(), effective_seed),
                    ("steps".into(), json!(4)),
                    ("quantization".into(), json!(4)),
                    ("guidance".into(), json!(1.0)),
                    ("scheduler".into(), json!("flow_match_euler_discrete")),
                    ("recipe".into(), json!(self.id())),
                    ("recipe_version".into(), json!(1)),
                    ("token_budget".into(), json!(512)),
                    ("thinking".into(), json!(false)),
                    ("truncation".into(), json!(false)),
                ])
            }
            Self::BiRefNet => BTreeMap::from([
                ("recipe".into(), json!(self.id())),
                ("recipe_version".into(), json!(1)),
                ("provider".into(), json!("CPUExecutionProvider")),
                ("precision".into(), json!("float32")),
                ("preprocessing".into(), json!("birefnet-1024-rgb-v1")),
                ("postprocessing".into(), json!("birefnet-logits-sigmoid-v1")),
                ("intra_op_threads".into(), json!(4)),
                ("inter_op_threads".into(), json!(1)),
                ("execution_mode".into(), json!("sequential")),
            ]),
            Self::RealEsrgan => BTreeMap::from([
                ("recipe".into(), json!(self.id())),
                ("recipe_version".into(), json!(1)),
                ("model".into(), json!("realesrgan-x4plus")),
                ("scale".into(), json!(4)),
                ("tile".into(), json!(128)),
                ("threads".into(), json!("1:1:1")),
                ("tta".into(), json!(false)),
            ]),
            Self::ZImageTurbo => unreachable!(),
        };
        require(
            plan.effective_parameters == expected,
            "effective recipe parameters do not match the pinned recipe",
        )
    }
}

pub fn validate_recipe_request(
    request: &UserRequest,
    source: Option<SourceFacts>,
) -> Result<Recipe, ContractError> {
    let recipe = Recipe::from_request(request)?;
    recipe.validate_request(request, source)?;
    Ok(recipe)
}

pub fn validate_recipe_plan(
    request: &UserRequest,
    plan: &ExecutionPlan,
    source: Option<SourceFacts>,
) -> Result<Recipe, ContractError> {
    let recipe = Recipe::from_request(request)?;
    recipe.validate_plan(request, plan, source)?;
    Ok(recipe)
}

pub fn freeze_klein(
    input: KleinInput,
    resolved_seed: u32,
    env: crate::generation::PlanEnvironment,
) -> Result<FrozenRecipe, ContractError> {
    let (capability_id, recipe, inputs, source) = match input.source {
        Some(source) => (
            KLEIN_EDIT_CAPABILITY,
            Recipe::KleinEdit,
            vec![InputBinding {
                asset: source.asset,
                role: SOURCE_ROLE.into(),
                order: 0,
            }],
            Some(source.facts),
        ),
        None => (KLEIN_TEXT_CAPABILITY, Recipe::KleinText, vec![], None),
    };
    let request = UserRequest {
        schema_version: crate::contracts::REQUEST_SCHEMA_VERSION,
        request_id: input.request_id,
        project_id: input.project_id,
        capability: crate::contracts::CapabilityDescriptor {
            capability_id: capability_id.into(),
            instruction_required: true,
            input_roles: inputs
                .iter()
                .map(|_| crate::contracts::RoleRule {
                    role: SOURCE_ROLE.into(),
                    minimum: 1,
                    maximum: 1,
                })
                .collect(),
        },
        instruction: input.prompt,
        inputs,
        model_variant_id: ModelVariantId::new(KLEIN_MODEL)?,
        requested_output: crate::contracts::ImageOutput {
            width: 1024,
            height: 1024,
            count: 1,
        },
        policy: crate::contracts::AdjustmentPolicy {
            allow_model_substitution: false,
            allow_resolution_change: false,
            allow_cpu_fallback: false,
        },
        parameters: BTreeMap::from([(
            "seed".into(),
            input
                .seed
                .map_or(serde_json::Value::Null, |seed| json!(seed)),
        )]),
        idempotency_key: input.idempotency_key,
    };
    let plan = ExecutionPlan {
        schema_version: crate::contracts::PLAN_SCHEMA_VERSION,
        plan_id: input.plan_id,
        request_id: request.request_id.clone(),
        model_variant_id: request.model_variant_id.clone(),
        components: env.components,
        runtime: env.runtime,
        device: env.device,
        effective_output: request.requested_output.clone(),
        effective_parameters: BTreeMap::from([
            ("seed".into(), json!(input.seed.unwrap_or(resolved_seed))),
            ("steps".into(), json!(4)),
            ("quantization".into(), json!(4)),
            ("guidance".into(), json!(1.0)),
            ("scheduler".into(), json!("flow_match_euler_discrete")),
            ("recipe".into(), json!(recipe.id())),
            ("recipe_version".into(), json!(1)),
            ("token_budget".into(), json!(512)),
            ("thinking".into(), json!(false)),
            ("truncation".into(), json!(false)),
        ]),
        adjustments: vec![],
    };
    recipe.validate_plan(&request, &plan, source)?;
    Ok(FrozenRecipe { request, plan })
}

pub fn freeze_birefnet(
    input: BiRefNetInput,
    env: crate::generation::PlanEnvironment,
) -> Result<FrozenRecipe, ContractError> {
    let source = input.source;
    let request = UserRequest {
        schema_version: crate::contracts::REQUEST_SCHEMA_VERSION,
        request_id: input.request_id,
        project_id: input.project_id,
        capability: crate::contracts::CapabilityDescriptor {
            capability_id: BIREFNET_CAPABILITY.into(),
            instruction_required: false,
            input_roles: vec![crate::contracts::RoleRule {
                role: SOURCE_ROLE.into(),
                minimum: 1,
                maximum: 1,
            }],
        },
        instruction: String::new(),
        inputs: vec![InputBinding {
            asset: source.asset,
            role: SOURCE_ROLE.into(),
            order: 0,
        }],
        model_variant_id: ModelVariantId::new(BIREFNET_MODEL)?,
        requested_output: crate::contracts::ImageOutput {
            width: source.facts.width,
            height: source.facts.height,
            count: 1,
        },
        policy: crate::contracts::AdjustmentPolicy {
            allow_model_substitution: false,
            allow_resolution_change: false,
            allow_cpu_fallback: true,
        },
        parameters: BTreeMap::new(),
        idempotency_key: input.idempotency_key,
    };
    let plan = ExecutionPlan {
        schema_version: crate::contracts::PLAN_SCHEMA_VERSION,
        plan_id: input.plan_id,
        request_id: request.request_id.clone(),
        model_variant_id: request.model_variant_id.clone(),
        components: env.components,
        runtime: env.runtime,
        device: env.device,
        effective_output: request.requested_output.clone(),
        effective_parameters: BTreeMap::from([
            ("recipe".into(), json!(Recipe::BiRefNet.id())),
            ("recipe_version".into(), json!(1)),
            ("provider".into(), json!("CPUExecutionProvider")),
            ("precision".into(), json!("float32")),
            ("preprocessing".into(), json!("birefnet-1024-rgb-v1")),
            ("postprocessing".into(), json!("birefnet-logits-sigmoid-v1")),
            ("intra_op_threads".into(), json!(4)),
            ("inter_op_threads".into(), json!(1)),
            ("execution_mode".into(), json!("sequential")),
        ]),
        adjustments: vec![],
    };
    Recipe::BiRefNet.validate_plan(&request, &plan, Some(source.facts))?;
    Ok(FrozenRecipe { request, plan })
}

fn validate_klein_instruction(request: &UserRequest) -> Result<(), ContractError> {
    require(
        request.capability.instruction_required
            && !request.instruction.trim().is_empty()
            && request.instruction.len() <= MAX_PROMPT_BYTES,
        "Klein instruction must contain text and be at most 16 KiB UTF-8",
    )
}

fn validate_seed_parameters(request: &UserRequest) -> Result<(), ContractError> {
    require(
        request.parameters.keys().all(|key| key == "seed"),
        "unsupported Klein request parameter",
    )?;
    if let Some(seed) = request.parameters.get("seed") {
        require(
            seed.is_null()
                || seed
                    .as_u64()
                    .is_some_and(|value| u32::try_from(value).is_ok()),
            "Klein seed must be null or u32",
        )?;
    }
    Ok(())
}

fn validate_single_source(
    request: &UserRequest,
    source: Option<SourceFacts>,
) -> Result<(), ContractError> {
    require(
        request.capability.input_roles.len() == 1
            && request.capability.input_roles[0].role == SOURCE_ROLE
            && request.capability.input_roles[0].minimum == 1
            && request.capability.input_roles[0].maximum == 1
            && request.inputs.len() == 1
            && request.inputs[0].role == SOURCE_ROLE
            && request.inputs[0].order == 0,
        "processing recipe requires exactly one ordered source image",
    )?;
    require(
        matches!(
            request.inputs[0].asset.data_type,
            AssetKind::ImageRgb | AssetKind::ImageRgba
        ),
        "processing source must be an RGB or RGBA image",
    )?;
    let source = source.ok_or_else(|| {
        ContractError::InvalidValue("processing recipe requires source image facts".into())
    })?;
    source.validate()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        contracts::{
            AdjustmentPolicy, CapabilityDescriptor, ComponentSnapshot, DeviceSnapshot,
            ExecutionPlan, ImageOutput, InputBinding, PLAN_SCHEMA_VERSION, REQUEST_SCHEMA_VERSION,
            RuntimeSnapshot, SourceRevision, UserRequest,
        },
        domain::{
            AssetId, AssetRef, ContentHash, IdempotencyKey, ModelVariantId, PlanId, ProjectId,
            RequestId, StorageLocation, StorageLocationId,
        },
        generation::PlanEnvironment,
    };

    fn asset(kind: AssetKind) -> AssetRef {
        AssetRef {
            asset_id: AssetId::new("asset-source").unwrap(),
            content_hash: ContentHash::new("a".repeat(64)).unwrap(),
            data_type: kind,
            location: StorageLocation::Managed {
                storage_id: StorageLocationId::new("library").unwrap(),
                relative_path: "inputs/source.png".into(),
            },
        }
    }

    fn request(
        model: &str,
        capability: &str,
        instruction_required: bool,
        source: bool,
    ) -> UserRequest {
        UserRequest {
            schema_version: REQUEST_SCHEMA_VERSION,
            request_id: RequestId::new("request:1").unwrap(),
            project_id: ProjectId::new("default").unwrap(),
            capability: CapabilityDescriptor {
                capability_id: capability.into(),
                instruction_required,
                input_roles: if source {
                    vec![crate::contracts::RoleRule {
                        role: SOURCE_ROLE.into(),
                        minimum: 1,
                        maximum: 1,
                    }]
                } else {
                    vec![]
                },
            },
            instruction: if instruction_required {
                "draw a cobalt cube"
            } else {
                ""
            }
            .into(),
            inputs: if source {
                vec![InputBinding {
                    asset: asset(AssetKind::ImageRgb),
                    role: SOURCE_ROLE.into(),
                    order: 0,
                }]
            } else {
                vec![]
            },
            model_variant_id: ModelVariantId::new(model).unwrap(),
            requested_output: ImageOutput {
                width: 1024,
                height: 1024,
                count: 1,
            },
            policy: AdjustmentPolicy {
                allow_model_substitution: false,
                allow_resolution_change: false,
                allow_cpu_fallback: false,
            },
            parameters: if instruction_required {
                BTreeMap::from([("seed".into(), serde_json::Value::Null)])
            } else {
                BTreeMap::new()
            },
            idempotency_key: IdempotencyKey::new("submit:1").unwrap(),
        }
    }

    fn plan(
        request: &UserRequest,
        parameters: BTreeMap<String, serde_json::Value>,
    ) -> ExecutionPlan {
        ExecutionPlan {
            schema_version: PLAN_SCHEMA_VERSION,
            plan_id: PlanId::new("plan:1").unwrap(),
            request_id: request.request_id.clone(),
            model_variant_id: request.model_variant_id.clone(),
            components: vec![ComponentSnapshot {
                role: "weights".into(),
                source: SourceRevision {
                    source: "fixture".into(),
                    revision: "pinned".into(),
                },
                content_hash: ContentHash::new("b".repeat(64)).unwrap(),
            }],
            runtime: RuntimeSnapshot {
                package_id: "fixture-runtime".into(),
                engine: "fixture".into(),
                source: SourceRevision {
                    source: "fixture".into(),
                    revision: "pinned".into(),
                },
            },
            device: DeviceSnapshot {
                device_id: "fixture".into(),
                precision: "float32".into(),
            },
            effective_output: request.requested_output.clone(),
            effective_parameters: parameters,
            adjustments: vec![],
        }
    }

    #[test]
    fn klein_text_and_edit_have_distinct_source_rules() {
        let text = request(KLEIN_MODEL, KLEIN_TEXT_CAPABILITY, true, false);
        assert!(validate_recipe_request(&text, None).is_ok());

        let mut edit = request(KLEIN_MODEL, KLEIN_EDIT_CAPABILITY, true, true);
        let facts = SourceFacts {
            width: 768,
            height: 1024,
            has_transparent_pixels: false,
        };
        assert!(validate_recipe_request(&edit, Some(facts)).is_ok());
        assert!(validate_recipe_request(&edit, None).is_err());
        edit.inputs[0].asset.data_type = AssetKind::Mask;
        assert!(validate_recipe_request(&edit, Some(facts)).is_err());
    }

    #[test]
    fn transparent_images_are_rejected_for_edit_and_upscale() {
        for (model, capability) in [
            (KLEIN_MODEL, KLEIN_EDIT_CAPABILITY),
            (REALESRGAN_MODEL, REALESRGAN_CAPABILITY),
        ] {
            let request = request(model, capability, true, true);
            let facts = SourceFacts {
                width: 512,
                height: 512,
                has_transparent_pixels: true,
            };
            assert!(validate_recipe_request(&request, Some(facts)).is_err());
        }
    }

    #[test]
    fn tool_recipes_do_not_require_fake_instruction_or_seed() {
        let mut birefnet = request(BIREFNET_MODEL, BIREFNET_CAPABILITY, false, true);
        birefnet.instruction.clear();
        let facts = SourceFacts {
            width: 768,
            height: 1024,
            has_transparent_pixels: true,
        };
        birefnet.requested_output.width = facts.width;
        birefnet.requested_output.height = facts.height;
        assert!(validate_recipe_request(&birefnet, Some(facts)).is_ok());

        let mut upscale = request(REALESRGAN_MODEL, REALESRGAN_CAPABILITY, false, true);
        upscale.instruction.clear();
        upscale.requested_output.width = 2048;
        upscale.requested_output.height = 2048;
        let facts = SourceFacts {
            width: 512,
            height: 512,
            has_transparent_pixels: false,
        };
        assert!(validate_recipe_request(&upscale, Some(facts)).is_ok());
    }

    #[test]
    fn plan_parameters_are_exact_for_each_new_backend() {
        let text = request(KLEIN_MODEL, KLEIN_TEXT_CAPABILITY, true, false);
        let klein_plan = plan(
            &text,
            BTreeMap::from([
                ("seed".into(), json!(42)),
                ("steps".into(), json!(4)),
                ("quantization".into(), json!(4)),
                ("guidance".into(), json!(1.0)),
                ("scheduler".into(), json!("flow_match_euler_discrete")),
                ("recipe".into(), json!("klein4b-text-v1")),
                ("recipe_version".into(), json!(1)),
                ("token_budget".into(), json!(512)),
                ("thinking".into(), json!(false)),
                ("truncation".into(), json!(false)),
            ]),
        );
        assert_eq!(
            validate_recipe_plan(&text, &klein_plan, None).unwrap(),
            Recipe::KleinText
        );

        let mut birefnet = request(BIREFNET_MODEL, BIREFNET_CAPABILITY, false, true);
        birefnet.instruction.clear();
        let facts = SourceFacts {
            width: 768,
            height: 1024,
            has_transparent_pixels: false,
        };
        birefnet.requested_output.width = facts.width;
        birefnet.requested_output.height = facts.height;
        let biref_plan = plan(
            &birefnet,
            BTreeMap::from([
                ("recipe".into(), json!("birefnet-general-cpu-v1")),
                ("recipe_version".into(), json!(1)),
                ("provider".into(), json!("CPUExecutionProvider")),
                ("precision".into(), json!("float32")),
                ("preprocessing".into(), json!("birefnet-1024-rgb-v1")),
                ("postprocessing".into(), json!("birefnet-logits-sigmoid-v1")),
                ("intra_op_threads".into(), json!(4)),
                ("inter_op_threads".into(), json!(1)),
                ("execution_mode".into(), json!("sequential")),
            ]),
        );
        assert!(validate_recipe_plan(&birefnet, &biref_plan, Some(facts)).is_ok());
        let mut altered = biref_plan;
        altered
            .effective_parameters
            .insert("provider".into(), json!("CoreMLExecutionProvider"));
        assert!(validate_recipe_plan(&birefnet, &altered, Some(facts)).is_err());
    }

    #[test]
    fn upscale_output_must_be_exact_four_times_source() {
        let mut request = request(REALESRGAN_MODEL, REALESRGAN_CAPABILITY, false, true);
        request.instruction.clear();
        request.requested_output = ImageOutput {
            width: 2048,
            height: 3072,
            count: 1,
        };
        let facts = SourceFacts {
            width: 512,
            height: 768,
            has_transparent_pixels: false,
        };
        assert!(validate_recipe_request(&request, Some(facts)).is_ok());
        request.requested_output.width = 2047;
        assert!(validate_recipe_request(&request, Some(facts)).is_err());
    }

    #[test]
    fn freeze_klein_keeps_text_and_edit_as_separate_recipes() {
        let env = PlanEnvironment {
            components: vec![ComponentSnapshot {
                role: "transformer".into(),
                source: SourceRevision {
                    source: "fixture".into(),
                    revision: "pinned".into(),
                },
                content_hash: ContentHash::new("f".repeat(64)).unwrap(),
            }],
            runtime: RuntimeSnapshot {
                package_id: "recipes-v2".into(),
                engine: "mlx".into(),
                source: SourceRevision {
                    source: "fixture".into(),
                    revision: "pinned".into(),
                },
            },
            device: DeviceSnapshot {
                device_id: "gpu".into(),
                precision: "bfloat16".into(),
            },
            scheduler: "flow_match_euler_discrete".into(),
        };
        let input = || KleinInput {
            request_id: RequestId::new("request:klein").unwrap(),
            project_id: ProjectId::new("default").unwrap(),
            idempotency_key: IdempotencyKey::new("submit:klein").unwrap(),
            plan_id: PlanId::new("plan:klein").unwrap(),
            prompt: "cobalt cube".into(),
            seed: None,
            source: None,
        };
        let text = freeze_klein(input(), 17, env.clone()).unwrap();
        assert_eq!(text.request.capability.capability_id, KLEIN_TEXT_CAPABILITY);
        assert_eq!(text.plan.effective_parameters["seed"], 17);

        let mut edit_input = input();
        edit_input.source = Some(SourceAsset {
            asset: asset(AssetKind::ImageRgb),
            facts: SourceFacts {
                width: 512,
                height: 512,
                has_transparent_pixels: false,
            },
        });
        let edit = freeze_klein(edit_input, 17, env).unwrap();
        assert_eq!(edit.request.capability.capability_id, KLEIN_EDIT_CAPABILITY);
        assert_eq!(edit.request.inputs.len(), 1);
    }

    #[test]
    fn freeze_birefnet_preserves_source_dimensions_and_cpu_recipe() {
        let environment = PlanEnvironment {
            components: vec![ComponentSnapshot {
                role: "weights".into(),
                source: SourceRevision {
                    source: "fixture".into(),
                    revision: "pinned".into(),
                },
                content_hash: ContentHash::new("c".repeat(64)).unwrap(),
            }],
            runtime: RuntimeSnapshot {
                package_id: "onnx-cpu".into(),
                engine: "onnxruntime".into(),
                source: SourceRevision {
                    source: "fixture".into(),
                    revision: "pinned".into(),
                },
            },
            device: DeviceSnapshot {
                device_id: "cpu".into(),
                precision: "float32".into(),
            },
            scheduler: "birefnet-1024-rgb-v1".into(),
        };
        let frozen = freeze_birefnet(
            BiRefNetInput {
                request_id: RequestId::new("request:birefnet").unwrap(),
                project_id: ProjectId::new("default").unwrap(),
                idempotency_key: IdempotencyKey::new("submit:birefnet").unwrap(),
                plan_id: PlanId::new("plan:birefnet").unwrap(),
                source: SourceAsset {
                    asset: asset(AssetKind::ImageRgba),
                    facts: SourceFacts {
                        width: 768,
                        height: 1024,
                        has_transparent_pixels: true,
                    },
                },
            },
            environment,
        )
        .unwrap();
        assert_eq!(frozen.request.instruction, "");
        assert!(frozen.request.parameters.is_empty());
        assert_eq!(frozen.request.requested_output.width, 768);
        assert_eq!(frozen.request.requested_output.height, 1024);
        assert_eq!(
            frozen.plan.effective_parameters["provider"],
            "CPUExecutionProvider"
        );
        assert_eq!(
            frozen.plan.effective_parameters["postprocessing"],
            "birefnet-logits-sigmoid-v1"
        );
    }
}
