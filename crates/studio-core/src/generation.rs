//! First-generation recipe and lifecycle, separate from the versioned contracts.
use crate::{ContractError, contracts::*, domain::*, require};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

pub const Z_IMAGE_MODEL: &str = "z-image-turbo-q4";
pub const Z_IMAGE_CAPABILITY: &str = "image.text-to-image.z-image-turbo.v1";
pub const Z_IMAGE_WINDOWS_MODEL: &str = "z-image-turbo-diffusers-cuda-nf4-v1";
pub const Z_IMAGE_WINDOWS_CAPABILITY: &str =
    "image.text-to-image.z-image-turbo-diffusers-cuda-nf4.v1";
pub const Z_IMAGE_WINDOWS_RECIPE: &str = "z-image-turbo-nf4-cuda-v1";
pub const Z_IMAGE_WINDOWS_RUNTIME: &str = "windows-python31210-torch2100-cu126-bnb0482-v1";
pub const Z_IMAGE_WINDOWS_ENGINE: &str = "diffusers-cuda";
pub const Z_IMAGE_WINDOWS_DEVICE: &str = "nvidia-cuda-sm86";
pub const Z_IMAGE_WINDOWS_PRECISION: &str =
    "transformer+text_encoder=nf4;compute=bfloat16;vae=bfloat16";
pub const Z_IMAGE_WINDOWS_SCHEDULER: &str = "FlowMatchEulerDiscreteScheduler";
pub const MAX_PROMPT_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptState {
    Submitted,
    Preparing,
    Running,
    Committing,
    CancelRequested,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

impl AttemptState {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Interrupted
        )
    }

    /// Progress may advance phases, but can never undo accepted cancellation or a terminal.
    pub fn can_advance_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (
                Self::Submitted,
                Self::Preparing | Self::Running | Self::Committing
            ) | (Self::Preparing, Self::Running | Self::Committing)
                | (Self::Running, Self::Committing)
        ) || (self == next && !self.is_terminal())
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Submitted => "submitted",
            Self::Preparing => "preparing",
            Self::Running => "running",
            Self::Committing => "committing",
            Self::CancelRequested => "cancel_requested",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }
}

#[derive(Clone, Debug)]
pub struct GenerationInput {
    pub request_id: RequestId,
    pub project_id: ProjectId,
    pub idempotency_key: IdempotencyKey,
    pub plan_id: PlanId,
    pub prompt: String,
    pub width: u32,
    pub height: u32,
    pub count: u32,
    pub seed: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct PlanEnvironment {
    pub components: Vec<ComponentSnapshot>,
    pub runtime: RuntimeSnapshot,
    pub device: DeviceSnapshot,
    pub scheduler: String,
}

#[derive(Clone, Debug)]
pub struct FrozenGeneration {
    pub request: UserRequest,
    pub plan: ExecutionPlan,
}

/// `resolved_seed` is supplied by the host's random source only when the input has no seed.
/// Tokenizer/template validation is deliberately left to the pinned worker, before GPU work.
pub fn freeze_z_image(
    input: GenerationInput,
    resolved_seed: u32,
    env: PlanEnvironment,
) -> Result<FrozenGeneration, ContractError> {
    let output = ImageOutput {
        width: input.width,
        height: input.height,
        count: input.count,
    };
    let request = UserRequest {
        schema_version: REQUEST_SCHEMA_VERSION,
        request_id: input.request_id,
        project_id: input.project_id,
        capability: CapabilityDescriptor {
            capability_id: Z_IMAGE_CAPABILITY.into(),
            instruction_required: true,
            input_roles: vec![],
        },
        instruction: input.prompt,
        inputs: vec![],
        model_variant_id: ModelVariantId::new(Z_IMAGE_MODEL)?,
        requested_output: output.clone(),
        policy: AdjustmentPolicy {
            allow_model_substitution: false,
            allow_resolution_change: false,
            allow_cpu_fallback: false,
        },
        parameters: BTreeMap::from([("seed".into(), json!(input.seed))]),
        idempotency_key: input.idempotency_key,
    };
    validate_z_image_request(&request)?;
    require(
        !env.scheduler.trim().is_empty(),
        "scheduler identity must be frozen",
    )?;
    let plan = ExecutionPlan {
        schema_version: PLAN_SCHEMA_VERSION,
        plan_id: input.plan_id,
        request_id: request.request_id.clone(),
        model_variant_id: request.model_variant_id.clone(),
        components: env.components,
        runtime: env.runtime,
        device: env.device,
        effective_output: output,
        effective_parameters: BTreeMap::from([
            ("seed".into(), json!(input.seed.unwrap_or(resolved_seed))),
            ("steps".into(), json!(9)),
            ("quantization".into(), json!(4)),
            ("guidance".into(), json!(0.0)),
            ("scheduler".into(), json!(env.scheduler)),
        ]),
        adjustments: vec![],
    };
    validate_z_image_plan(&request, &plan)?;
    Ok(FrozenGeneration { request, plan })
}

pub fn freeze_windows_z_image(
    input: GenerationInput,
    resolved_seed: u32,
    env: PlanEnvironment,
) -> Result<FrozenGeneration, ContractError> {
    require(
        (input.width, input.height, input.count) == (1024, 1024, 1),
        "Windows Z-Image accepts exactly one 1024x1024 image",
    )?;
    let output = ImageOutput {
        width: 1024,
        height: 1024,
        count: 1,
    };
    let request = UserRequest {
        schema_version: REQUEST_SCHEMA_VERSION,
        request_id: input.request_id,
        project_id: input.project_id,
        capability: CapabilityDescriptor {
            capability_id: Z_IMAGE_WINDOWS_CAPABILITY.into(),
            instruction_required: true,
            input_roles: vec![],
        },
        instruction: input.prompt,
        inputs: vec![],
        model_variant_id: ModelVariantId::new(Z_IMAGE_WINDOWS_MODEL)?,
        requested_output: output.clone(),
        policy: AdjustmentPolicy {
            allow_model_substitution: false,
            allow_resolution_change: false,
            allow_cpu_fallback: false,
        },
        parameters: BTreeMap::from([("seed".into(), json!(input.seed))]),
        idempotency_key: input.idempotency_key,
    };
    validate_windows_z_image_request(&request)?;
    let seed = input.seed.unwrap_or(resolved_seed);
    let plan = ExecutionPlan {
        schema_version: PLAN_SCHEMA_VERSION,
        plan_id: input.plan_id,
        request_id: request.request_id.clone(),
        model_variant_id: request.model_variant_id.clone(),
        components: env.components,
        runtime: env.runtime,
        device: env.device,
        effective_output: output,
        effective_parameters: windows_z_image_parameters(seed, &env.scheduler),
        adjustments: vec![],
    };
    validate_windows_z_image_plan(&request, &plan)?;
    Ok(FrozenGeneration { request, plan })
}

pub fn validate_z_image_request(request: &UserRequest) -> Result<(), ContractError> {
    request.validate()?;
    require(
        request.model_variant_id.as_str() == Z_IMAGE_MODEL,
        "unsupported model variant",
    )?;
    require(
        request.capability.capability_id == Z_IMAGE_CAPABILITY
            && request.capability.input_roles.is_empty()
            && request.inputs.is_empty(),
        "unsupported generation capability or inputs",
    )?;
    require(
        !request.instruction.trim().is_empty() && request.instruction.len() <= MAX_PROMPT_BYTES,
        "prompt must contain text and be at most 16 KiB UTF-8",
    )?;
    require(
        matches!(
            (
                request.requested_output.width,
                request.requested_output.height
            ),
            (1024, 1024) | (1024, 768) | (768, 1024)
        ) && request.requested_output.count == 1,
        "Z-Image Turbo requires one image at 1024x1024, 1024x768 or 768x1024",
    )?;
    require(
        request.parameters.keys().all(|key| key == "seed"),
        "unsupported request parameter",
    )?;
    if let Some(seed) = request.parameters.get("seed") {
        require(
            seed.is_null() || seed.as_u64().is_some_and(|v| u32::try_from(v).is_ok()),
            "seed must be empty or u32",
        )?;
    }
    Ok(())
}

pub fn validate_windows_z_image_request(request: &UserRequest) -> Result<(), ContractError> {
    request.validate()?;
    require(
        request.model_variant_id.as_str() == Z_IMAGE_WINDOWS_MODEL,
        "unsupported Windows model variant",
    )?;
    require(
        request.capability.capability_id == Z_IMAGE_WINDOWS_CAPABILITY
            && request.capability.input_roles.is_empty()
            && request.inputs.is_empty(),
        "unsupported Windows generation capability or inputs",
    )?;
    require(
        !request.instruction.trim().is_empty() && request.instruction.len() <= MAX_PROMPT_BYTES,
        "prompt must contain text and be at most 16 KiB UTF-8",
    )?;
    require(
        (
            request.requested_output.width,
            request.requested_output.height,
        ) == (1024, 1024)
            && request.requested_output.count == 1,
        "Windows Z-Image requires one 1024x1024 image",
    )?;
    require(
        request.parameters.keys().all(|key| key == "seed"),
        "unsupported Windows request parameter",
    )?;
    if let Some(seed) = request.parameters.get("seed") {
        require(
            seed.is_null() || seed.as_u64().is_some_and(|v| u32::try_from(v).is_ok()),
            "seed must be empty or u32",
        )?;
    }
    Ok(())
}

pub fn validate_z_image_plan(
    request: &UserRequest,
    plan: &ExecutionPlan,
) -> Result<(), ContractError> {
    validate_z_image_request(request)?;
    plan.validate_against(request)?;
    require(
        plan.model_variant_id == request.model_variant_id
            && plan.effective_output == request.requested_output
            && plan.adjustments.is_empty(),
        "recipe cannot substitute model or output",
    )?;
    let p = &plan.effective_parameters;
    require(
        p.len() == 5
            && p.get("steps") == Some(&json!(9))
            && p.get("quantization") == Some(&json!(4))
            && p.get("guidance").and_then(|v| v.as_f64()) == Some(0.0)
            && p.get("scheduler")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.trim().is_empty()),
        "effective recipe parameters do not match the pinned recipe",
    )?;
    let seed = p
        .get("seed")
        .and_then(|v| v.as_u64())
        .filter(|v| u32::try_from(*v).is_ok());
    require(seed.is_some(), "effective seed must be u32")?;
    if let Some(requested) = request.parameters.get("seed").and_then(|v| v.as_u64()) {
        require(
            seed == Some(requested),
            "effective seed must preserve explicit requested seed",
        )?;
    }
    Ok(())
}

pub fn validate_windows_z_image_plan(
    request: &UserRequest,
    plan: &ExecutionPlan,
) -> Result<(), ContractError> {
    validate_windows_z_image_request(request)?;
    plan.validate_against(request)?;
    require(
        plan.model_variant_id == request.model_variant_id
            && plan.effective_output == request.requested_output
            && plan.adjustments.is_empty(),
        "Windows recipe cannot substitute model or output",
    )?;
    require(
        plan.runtime.package_id == Z_IMAGE_WINDOWS_RUNTIME
            && plan.runtime.engine == Z_IMAGE_WINDOWS_ENGINE
            && plan.device.device_id == Z_IMAGE_WINDOWS_DEVICE
            && plan.device.precision == Z_IMAGE_WINDOWS_PRECISION,
        "Windows runtime or device identity does not match the frozen profile",
    )?;
    let seed = plan
        .effective_parameters
        .get("seed")
        .and_then(|v| v.as_u64())
        .filter(|v| u32::try_from(*v).is_ok())
        .ok_or_else(|| ContractError::InvalidValue("effective seed must be u32".into()))?;
    require(
        plan.effective_parameters
            == windows_z_image_parameters(
                u32::try_from(seed)
                    .map_err(|_| ContractError::InvalidValue("effective seed is invalid".into()))?,
                Z_IMAGE_WINDOWS_SCHEDULER,
            ),
        "effective Windows recipe parameters do not match the pinned recipe",
    )?;
    if let Some(requested) = request.parameters.get("seed").and_then(|v| v.as_u64()) {
        require(
            seed == requested,
            "effective seed must preserve explicit requested seed",
        )?;
    }
    Ok(())
}

fn windows_z_image_parameters(seed: u32, scheduler: &str) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        ("seed".into(), json!(seed)),
        ("steps".into(), json!(9)),
        ("quantization".into(), json!("nf4")),
        ("guidance".into(), json!(0.0)),
        ("scheduler".into(), json!(scheduler)),
        ("recipe".into(), json!(Z_IMAGE_WINDOWS_RECIPE)),
        ("recipe_version".into(), json!(1)),
        ("token_budget".into(), json!(512)),
        ("truncation".into(), json!(false)),
        ("thinking".into(), json!(true)),
        (
            "quantized_components".into(),
            json!(["transformer", "text_encoder"]),
        ),
        ("compute_dtype".into(), json!("bfloat16")),
        ("vae_precision".into(), json!("bfloat16")),
        ("attention".into(), json!("_native_efficient")),
        ("offload".into(), json!("model_cpu")),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn freeze(seed: Option<u32>) -> FrozenGeneration {
        freeze_z_image(
            GenerationInput {
                request_id: RequestId::new("request:1").unwrap(),
                project_id: ProjectId::new("default").unwrap(),
                idempotency_key: IdempotencyKey::new("submit:1").unwrap(),
                plan_id: PlanId::new("plan:1").unwrap(),
                prompt: "中文 prompt".into(),
                width: 1024,
                height: 768,
                count: 1,
                seed,
            },
            42,
            PlanEnvironment {
                components: vec![ComponentSnapshot {
                    role: "transformer".into(),
                    source: SourceRevision {
                        source: "fixture".into(),
                        revision: "pinned".into(),
                    },
                    content_hash: ContentHash::new("a".repeat(64)).unwrap(),
                }],
                runtime: RuntimeSnapshot {
                    package_id: "mflux-0.24.0".into(),
                    engine: "mlx".into(),
                    source: SourceRevision {
                        source: "fixture".into(),
                        revision: "pinned".into(),
                    },
                },
                device: DeviceSnapshot {
                    device_id: "metal".into(),
                    precision: "bfloat16".into(),
                },
                scheduler: "FlowMatchEulerDiscreteScheduler".into(),
            },
        )
        .unwrap()
    }

    fn windows_freeze(seed: Option<u32>) -> FrozenGeneration {
        freeze_windows_z_image(
            GenerationInput {
                request_id: RequestId::new("request:windows").unwrap(),
                project_id: ProjectId::new("default").unwrap(),
                idempotency_key: IdempotencyKey::new("submit:windows").unwrap(),
                plan_id: PlanId::new("plan:windows").unwrap(),
                prompt: "中文 Windows prompt".into(),
                width: 1024,
                height: 1024,
                count: 1,
                seed,
            },
            77,
            PlanEnvironment {
                components: vec![ComponentSnapshot {
                    role: "transformer".into(),
                    source: SourceRevision {
                        source: "modelscope:fixture".into(),
                        revision: "pinned".into(),
                    },
                    content_hash: ContentHash::new("b".repeat(64)).unwrap(),
                }],
                runtime: RuntimeSnapshot {
                    package_id: Z_IMAGE_WINDOWS_RUNTIME.into(),
                    engine: Z_IMAGE_WINDOWS_ENGINE.into(),
                    source: SourceRevision {
                        source: "fixture".into(),
                        revision: "pinned".into(),
                    },
                },
                device: DeviceSnapshot {
                    device_id: Z_IMAGE_WINDOWS_DEVICE.into(),
                    precision: Z_IMAGE_WINDOWS_PRECISION.into(),
                },
                scheduler: Z_IMAGE_WINDOWS_SCHEDULER.into(),
            },
        )
        .unwrap()
    }
    #[test]
    fn random_seed_is_frozen_without_rewriting_request() {
        let f = freeze(None);
        assert!(f.request.parameters["seed"].is_null());
        assert_eq!(f.plan.effective_parameters["seed"], 42);
    }
    #[test]
    fn explicit_seed_is_preserved() {
        let f = freeze(Some(7));
        assert_eq!(f.plan.effective_parameters["seed"], 7);
    }
    #[test]
    fn unsupported_inputs_are_not_downgraded() {
        let mut f = freeze(None);
        f.request.requested_output.count = 2;
        assert!(validate_z_image_request(&f.request).is_err());
        f = freeze(None);
        f.request.parameters.insert("guidance".into(), json!(3));
        assert!(validate_z_image_request(&f.request).is_err());
        f = freeze(None);
        f.request.instruction = "字".repeat(6000);
        assert!(validate_z_image_request(&f.request).is_err());
    }

    #[test]
    fn windows_recipe_freezes_square_nf4_profile_and_random_seed() {
        let frozen = windows_freeze(None);
        assert!(frozen.request.parameters["seed"].is_null());
        assert_eq!(frozen.plan.effective_parameters["seed"], 77);
        assert_eq!(frozen.plan.effective_parameters["steps"], 9);
        assert_eq!(frozen.plan.effective_parameters["quantization"], "nf4");
        assert_eq!(frozen.plan.effective_parameters["guidance"], 0.0);
        assert_eq!(
            frozen.plan.effective_parameters["attention"],
            "_native_efficient"
        );
        assert_eq!(frozen.plan.effective_parameters["offload"], "model_cpu");
        assert_eq!(frozen.plan.effective_output.width, 1024);
        assert_eq!(frozen.plan.effective_output.height, 1024);
        assert_eq!(frozen.plan.effective_output.count, 1);
    }

    #[test]
    fn windows_recipe_rejects_non_square_source_and_parameter_drift() {
        let input = GenerationInput {
            request_id: RequestId::new("request:windows").unwrap(),
            project_id: ProjectId::new("default").unwrap(),
            idempotency_key: IdempotencyKey::new("submit:windows").unwrap(),
            plan_id: PlanId::new("plan:windows").unwrap(),
            prompt: "fixture".into(),
            width: 1024,
            height: 768,
            count: 1,
            seed: None,
        };
        let env = PlanEnvironment {
            components: vec![],
            runtime: RuntimeSnapshot {
                package_id: Z_IMAGE_WINDOWS_RUNTIME.into(),
                engine: Z_IMAGE_WINDOWS_ENGINE.into(),
                source: SourceRevision {
                    source: "fixture".into(),
                    revision: "pinned".into(),
                },
            },
            device: DeviceSnapshot {
                device_id: Z_IMAGE_WINDOWS_DEVICE.into(),
                precision: Z_IMAGE_WINDOWS_PRECISION.into(),
            },
            scheduler: Z_IMAGE_WINDOWS_SCHEDULER.into(),
        };
        assert!(freeze_windows_z_image(input, 1, env).is_err());

        let mut frozen = windows_freeze(Some(7));
        frozen.request.inputs.push(InputBinding {
            role: "source".into(),
            order: 0,
            asset: AssetRef {
                asset_id: AssetId::new("asset:source").unwrap(),
                content_hash: ContentHash::new("c".repeat(64)).unwrap(),
                data_type: AssetKind::ImageRgb,
                location: StorageLocation::Managed {
                    storage_id: StorageLocationId::new("library").unwrap(),
                    relative_path: "inputs/source.png".into(),
                },
            },
        });
        assert!(validate_windows_z_image_request(&frozen.request).is_err());

        let mut frozen = windows_freeze(None);
        frozen
            .plan
            .effective_parameters
            .insert("truncation".into(), json!(true));
        assert!(validate_windows_z_image_plan(&frozen.request, &frozen.plan).is_err());
    }
    #[test]
    fn phases_cannot_undo_cancellation_or_terminal() {
        assert!(!AttemptState::CancelRequested.can_advance_to(AttemptState::Running));
        assert!(!AttemptState::Succeeded.can_advance_to(AttemptState::Running));
        assert!(!AttemptState::Running.can_advance_to(AttemptState::Preparing));
    }
}
