//! Versioned, model-specific generation and image-processing recipes.

use crate::{
    ContractError,
    contracts::{ExecutionPlan, InputBinding, UserRequest},
    domain::{AssetKind, AssetRef, IdempotencyKey, ModelVariantId, PlanId, ProjectId, RequestId},
    generation::{
        BIREFNET_WINDOWS_CAPABILITY, BIREFNET_WINDOWS_MODEL, BIREFNET_WINDOWS_RECIPE,
        KLEIN_WINDOWS_EDIT_CAPABILITY, KLEIN_WINDOWS_EDIT_RECIPE, KLEIN_WINDOWS_MODEL,
        KLEIN_WINDOWS_TEXT_CAPABILITY, KLEIN_WINDOWS_TEXT_RECIPE, MAX_PROMPT_BYTES,
        MOSS_IMAGE_SFT_WINDOWS_CAPABILITY, MOSS_IMAGE_SFT_WINDOWS_MODEL,
        MOSS_IMAGE_SFT_WINDOWS_RECIPE, QWEN_IMAGE_21_TURBO_WINDOWS_CAPABILITY,
        QWEN_IMAGE_21_TURBO_WINDOWS_MODEL, QWEN_IMAGE_21_TURBO_WINDOWS_RECIPE,
        QWEN_IMAGE_21_WINDOWS_CAPABILITY, QWEN_IMAGE_21_WINDOWS_MODEL,
        QWEN_IMAGE_21_WINDOWS_RECIPE, REALESRGAN_WINDOWS_CAPABILITY, REALESRGAN_WINDOWS_MODEL,
        REALESRGAN_WINDOWS_RECIPE, REALESRGAN_WINDOWS_RUNTIME, WINDOWS_CPU_DEVICE,
        WINDOWS_CUDA_DEVICE, WINDOWS_DIFFUSERS_ENGINE, WINDOWS_NCNN_ENGINE, WINDOWS_ONNX_ENGINE,
        WINDOWS_V2_RUNTIME, WINDOWS_VULKAN_DEVICE, Z_IMAGE_CAPABILITY, Z_IMAGE_MODEL,
        Z_IMAGE_WINDOWS_CAPABILITY, Z_IMAGE_WINDOWS_DEVICE, Z_IMAGE_WINDOWS_ENGINE,
        Z_IMAGE_WINDOWS_MODEL, Z_IMAGE_WINDOWS_RECIPE, Z_IMAGE_WINDOWS_RUNTIME,
        Z_IMAGE_WINDOWS_SCHEDULER, is_windows_runtime, validate_windows_z_image_plan,
        validate_windows_z_image_request, validate_z_image_plan, validate_z_image_request,
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
pub const QWEN_IMAGE_21_MODEL: &str = "qwen-image-2.1-mflux-v1";
pub const QWEN_IMAGE_21_CAPABILITY: &str = "image.text-to-image.qwen-image-2.1.v1";
pub const QWEN_IMAGE_21_TURBO_MODEL: &str = "qwen-image-2.1-turbo-mflux-v1";
pub const QWEN_IMAGE_21_TURBO_CAPABILITY: &str = "image.text-to-image.qwen-image-2.1-turbo.v1";
pub const MOSS_IMAGE_SFT_MODEL: &str = "moss-image-sft-mflux-v1";
pub const MOSS_IMAGE_SFT_CAPABILITY: &str = "image.text-to-image.moss-image-sft.v1";

// Compact (8 GB VRAM) Windows recipe identities. They mirror
// `worker-protocol::generation`; studio-core deliberately keeps its own copy of the frozen
// Windows registry, and every identity here must match the worker byte for byte.
pub const KLEIN_WINDOWS_TEXT_COMPACT_RECIPE: &str = "klein4b-text-windows-compact-v1";
pub const KLEIN_WINDOWS_EDIT_COMPACT_RECIPE: &str = "klein4b-edit-windows-compact-v1";
pub const QWEN_IMAGE_21_WINDOWS_COMPACT_RECIPE: &str = "qwen21-windows-txt2img-nf4-40-compact-v1";
pub const QWEN_IMAGE_21_TURBO_WINDOWS_COMPACT_RECIPE: &str =
    "qwen21-turbo-windows-txt2img-nf4-8-compact-v1";
pub const MOSS_IMAGE_SFT_WINDOWS_COMPACT_RECIPE: &str =
    "moss-image-sft-windows-txt2img-nf4-50-compact-v1";
pub const Z_IMAGE_WINDOWS_COMPACT_RECIPE: &str = "z-image-turbo-nf4-cuda-compact-v1";

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
    ZImageTurboCudaNf4,
    KleinText,
    KleinEdit,
    BiRefNet,
    RealEsrgan,
    QwenImage21,
    QwenImage21Turbo,
    MossImageSft,
    KleinTextWindows,
    KleinEditWindows,
    BiRefNetWindows,
    RealEsrganWindows,
    QwenImage21Windows,
    QwenImage21TurboWindows,
    MossImageSftWindows,
    KleinTextWindowsCompact,
    KleinEditWindowsCompact,
    QwenImage21WindowsCompact,
    QwenImage21TurboWindowsCompact,
    MossImageSftWindowsCompact,
    ZImageTurboCudaNf4Compact,
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
pub struct RealEsrganInput {
    pub request_id: RequestId,
    pub project_id: ProjectId,
    pub idempotency_key: IdempotencyKey,
    pub plan_id: PlanId,
    pub source: SourceAsset,
}

#[derive(Clone, Debug)]
pub struct TextModelInput {
    pub request_id: RequestId,
    pub project_id: ProjectId,
    pub idempotency_key: IdempotencyKey,
    pub plan_id: PlanId,
    pub prompt: String,
    pub seed: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct FrozenRecipe {
    pub request: UserRequest,
    pub plan: ExecutionPlan,
}

/// Compact-tier Windows requests ask for one 768² square; every other shape keeps the existing
/// standard-tier recipe selection so 1024² behaviour cannot change silently.
fn requests_compact_output(request: &UserRequest) -> bool {
    (
        request.requested_output.width,
        request.requested_output.height,
    ) == (768, 768)
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
            (Z_IMAGE_WINDOWS_MODEL, Z_IMAGE_WINDOWS_CAPABILITY)
                if requests_compact_output(request) =>
            {
                Self::ZImageTurboCudaNf4Compact
            }
            (Z_IMAGE_WINDOWS_MODEL, Z_IMAGE_WINDOWS_CAPABILITY) => Self::ZImageTurboCudaNf4,
            (KLEIN_MODEL, KLEIN_TEXT_CAPABILITY) => Self::KleinText,
            (KLEIN_MODEL, KLEIN_EDIT_CAPABILITY) => Self::KleinEdit,
            (BIREFNET_MODEL, BIREFNET_CAPABILITY) => Self::BiRefNet,
            (REALESRGAN_MODEL, REALESRGAN_CAPABILITY) => Self::RealEsrgan,
            (QWEN_IMAGE_21_MODEL, QWEN_IMAGE_21_CAPABILITY) => Self::QwenImage21,
            (QWEN_IMAGE_21_TURBO_MODEL, QWEN_IMAGE_21_TURBO_CAPABILITY) => Self::QwenImage21Turbo,
            (MOSS_IMAGE_SFT_MODEL, MOSS_IMAGE_SFT_CAPABILITY) => Self::MossImageSft,
            (KLEIN_WINDOWS_MODEL, KLEIN_WINDOWS_TEXT_CAPABILITY)
                if requests_compact_output(request) =>
            {
                Self::KleinTextWindowsCompact
            }
            (KLEIN_WINDOWS_MODEL, KLEIN_WINDOWS_EDIT_CAPABILITY)
                if requests_compact_output(request) =>
            {
                Self::KleinEditWindowsCompact
            }
            (QWEN_IMAGE_21_WINDOWS_MODEL, QWEN_IMAGE_21_WINDOWS_CAPABILITY)
                if requests_compact_output(request) =>
            {
                Self::QwenImage21WindowsCompact
            }
            (QWEN_IMAGE_21_TURBO_WINDOWS_MODEL, QWEN_IMAGE_21_TURBO_WINDOWS_CAPABILITY)
                if requests_compact_output(request) =>
            {
                Self::QwenImage21TurboWindowsCompact
            }
            (MOSS_IMAGE_SFT_WINDOWS_MODEL, MOSS_IMAGE_SFT_WINDOWS_CAPABILITY)
                if requests_compact_output(request) =>
            {
                Self::MossImageSftWindowsCompact
            }
            (KLEIN_WINDOWS_MODEL, KLEIN_WINDOWS_TEXT_CAPABILITY) => Self::KleinTextWindows,
            (KLEIN_WINDOWS_MODEL, KLEIN_WINDOWS_EDIT_CAPABILITY) => Self::KleinEditWindows,
            (BIREFNET_WINDOWS_MODEL, BIREFNET_WINDOWS_CAPABILITY) => Self::BiRefNetWindows,
            (REALESRGAN_WINDOWS_MODEL, REALESRGAN_WINDOWS_CAPABILITY) => Self::RealEsrganWindows,
            (QWEN_IMAGE_21_WINDOWS_MODEL, QWEN_IMAGE_21_WINDOWS_CAPABILITY) => {
                Self::QwenImage21Windows
            }
            (QWEN_IMAGE_21_TURBO_WINDOWS_MODEL, QWEN_IMAGE_21_TURBO_WINDOWS_CAPABILITY) => {
                Self::QwenImage21TurboWindows
            }
            (MOSS_IMAGE_SFT_WINDOWS_MODEL, MOSS_IMAGE_SFT_WINDOWS_CAPABILITY) => {
                Self::MossImageSftWindows
            }
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
            Self::ZImageTurboCudaNf4 => "z-image-turbo-nf4-cuda-v1",
            Self::KleinText => "klein4b-text-v1",
            Self::KleinEdit => "klein4b-edit-v1",
            Self::BiRefNet => "birefnet-general-cpu-v1",
            Self::RealEsrgan => "realesrgan-x4plus-t128-v1",
            Self::QwenImage21 => "qwen21-txt2img-q8-40-v1",
            Self::QwenImage21Turbo => "qwen21-turbo-txt2img-q8-8-v1",
            Self::MossImageSft => "moss-image-sft-txt2img-q8-50-v1",
            Self::KleinTextWindows => KLEIN_WINDOWS_TEXT_RECIPE,
            Self::KleinEditWindows => KLEIN_WINDOWS_EDIT_RECIPE,
            Self::BiRefNetWindows => BIREFNET_WINDOWS_RECIPE,
            Self::RealEsrganWindows => REALESRGAN_WINDOWS_RECIPE,
            Self::QwenImage21Windows => QWEN_IMAGE_21_WINDOWS_RECIPE,
            Self::QwenImage21TurboWindows => QWEN_IMAGE_21_TURBO_WINDOWS_RECIPE,
            Self::MossImageSftWindows => MOSS_IMAGE_SFT_WINDOWS_RECIPE,
            Self::KleinTextWindowsCompact => KLEIN_WINDOWS_TEXT_COMPACT_RECIPE,
            Self::KleinEditWindowsCompact => KLEIN_WINDOWS_EDIT_COMPACT_RECIPE,
            Self::QwenImage21WindowsCompact => QWEN_IMAGE_21_WINDOWS_COMPACT_RECIPE,
            Self::QwenImage21TurboWindowsCompact => QWEN_IMAGE_21_TURBO_WINDOWS_COMPACT_RECIPE,
            Self::MossImageSftWindowsCompact => MOSS_IMAGE_SFT_WINDOWS_COMPACT_RECIPE,
            Self::ZImageTurboCudaNf4Compact => Z_IMAGE_WINDOWS_COMPACT_RECIPE,
        }
    }

    /// Windows recipes use independent identities and never inherit the macOS parameter maps.
    pub fn is_windows(self) -> bool {
        matches!(
            self,
            Self::ZImageTurboCudaNf4
                | Self::KleinTextWindows
                | Self::KleinEditWindows
                | Self::BiRefNetWindows
                | Self::RealEsrganWindows
                | Self::QwenImage21Windows
                | Self::QwenImage21TurboWindows
                | Self::MossImageSftWindows
                | Self::KleinTextWindowsCompact
                | Self::KleinEditWindowsCompact
                | Self::QwenImage21WindowsCompact
                | Self::QwenImage21TurboWindowsCompact
                | Self::MossImageSftWindowsCompact
                | Self::ZImageTurboCudaNf4Compact
        )
    }

    pub fn outputs(self) -> &'static [OutputRole] {
        const PRIMARY: &[OutputRole] = &[OutputRole::Primary];
        const PRIMARY_AND_MASK: &[OutputRole] = &[OutputRole::Primary, OutputRole::Mask];
        match self {
            Self::BiRefNet | Self::BiRefNetWindows => PRIMARY_AND_MASK,
            _ => PRIMARY,
        }
    }

    pub fn accepts_output(self, role: OutputRole, kind: AssetKind) -> bool {
        match (self, role) {
            (Self::ZImageTurbo, OutputRole::Primary) => {
                matches!(kind, AssetKind::ImageRgb | AssetKind::ImageRgba)
            }
            (Self::ZImageTurboCudaNf4, OutputRole::Primary) => matches!(kind, AssetKind::ImageRgb),
            (
                Self::KleinText
                | Self::KleinEdit
                | Self::RealEsrgan
                | Self::QwenImage21
                | Self::QwenImage21Turbo
                | Self::MossImageSft
                | Self::KleinTextWindows
                | Self::KleinEditWindows
                | Self::RealEsrganWindows
                | Self::QwenImage21Windows
                | Self::QwenImage21TurboWindows
                | Self::MossImageSftWindows
                | Self::KleinTextWindowsCompact
                | Self::KleinEditWindowsCompact
                | Self::QwenImage21WindowsCompact
                | Self::QwenImage21TurboWindowsCompact
                | Self::MossImageSftWindowsCompact
                | Self::ZImageTurboCudaNf4Compact,
                OutputRole::Primary,
            ) => matches!(kind, AssetKind::ImageRgb),
            (Self::BiRefNet | Self::BiRefNetWindows, OutputRole::Primary) => {
                matches!(kind, AssetKind::ImageRgba)
            }
            (Self::BiRefNet | Self::BiRefNetWindows, OutputRole::Mask) => {
                matches!(kind, AssetKind::Mask)
            }
            _ => false,
        }
    }

    pub fn max_output_bytes(self, role: OutputRole) -> Option<u64> {
        match (self, role) {
            (Self::RealEsrgan | Self::RealEsrganWindows, OutputRole::Primary) => {
                Some(MAX_UPSCALE_OUTPUT_BYTES)
            }
            (
                Self::ZImageTurbo
                | Self::ZImageTurboCudaNf4
                | Self::KleinText
                | Self::KleinEdit
                | Self::QwenImage21
                | Self::QwenImage21Turbo
                | Self::MossImageSft
                | Self::KleinTextWindows
                | Self::KleinEditWindows
                | Self::QwenImage21Windows
                | Self::QwenImage21TurboWindows
                | Self::MossImageSftWindows
                | Self::KleinTextWindowsCompact
                | Self::KleinEditWindowsCompact
                | Self::QwenImage21WindowsCompact
                | Self::QwenImage21TurboWindowsCompact
                | Self::MossImageSftWindowsCompact
                | Self::ZImageTurboCudaNf4Compact,
                OutputRole::Primary,
            )
            | (Self::BiRefNet | Self::BiRefNetWindows, OutputRole::Primary | OutputRole::Mask) => {
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
        if self == Self::ZImageTurboCudaNf4 {
            require(
                source.is_none(),
                "Windows Z-Image does not accept a source image",
            )?;
            return validate_windows_z_image_request(request);
        }
        if self == Self::ZImageTurboCudaNf4Compact {
            require(
                source.is_none(),
                "Windows Z-Image compact does not accept a source image",
            )?;
            return validate_windows_z_image_compact_request(request);
        }
        request.validate()?;
        let expected_capability = match self {
            Self::KleinText => KLEIN_TEXT_CAPABILITY,
            Self::KleinEdit => KLEIN_EDIT_CAPABILITY,
            Self::BiRefNet => BIREFNET_CAPABILITY,
            Self::RealEsrgan => REALESRGAN_CAPABILITY,
            Self::QwenImage21 => QWEN_IMAGE_21_CAPABILITY,
            Self::QwenImage21Turbo => QWEN_IMAGE_21_TURBO_CAPABILITY,
            Self::MossImageSft => MOSS_IMAGE_SFT_CAPABILITY,
            Self::KleinTextWindows => KLEIN_WINDOWS_TEXT_CAPABILITY,
            Self::KleinEditWindows => KLEIN_WINDOWS_EDIT_CAPABILITY,
            Self::BiRefNetWindows => BIREFNET_WINDOWS_CAPABILITY,
            Self::RealEsrganWindows => REALESRGAN_WINDOWS_CAPABILITY,
            Self::QwenImage21Windows => QWEN_IMAGE_21_WINDOWS_CAPABILITY,
            Self::QwenImage21TurboWindows => QWEN_IMAGE_21_TURBO_WINDOWS_CAPABILITY,
            Self::MossImageSftWindows => MOSS_IMAGE_SFT_WINDOWS_CAPABILITY,
            Self::KleinTextWindowsCompact => KLEIN_WINDOWS_TEXT_CAPABILITY,
            Self::KleinEditWindowsCompact => KLEIN_WINDOWS_EDIT_CAPABILITY,
            Self::QwenImage21WindowsCompact => QWEN_IMAGE_21_WINDOWS_CAPABILITY,
            Self::QwenImage21TurboWindowsCompact => QWEN_IMAGE_21_TURBO_WINDOWS_CAPABILITY,
            Self::MossImageSftWindowsCompact => MOSS_IMAGE_SFT_WINDOWS_CAPABILITY,
            Self::ZImageTurbo | Self::ZImageTurboCudaNf4 | Self::ZImageTurboCudaNf4Compact => {
                unreachable!()
            }
        };
        require(
            request.capability.capability_id == expected_capability,
            "capability does not match recipe",
        )?;
        match self {
            Self::KleinText | Self::KleinTextWindows => {
                validate_klein_text_request(request, source, 1024)?;
            }
            Self::KleinTextWindowsCompact => {
                validate_klein_text_request(request, source, 768)?;
            }
            Self::KleinEdit | Self::KleinEditWindows => {
                validate_klein_edit_request(request, source, 1024)?;
            }
            Self::KleinEditWindowsCompact => {
                validate_klein_edit_request(request, source, 768)?;
            }
            Self::BiRefNet | Self::BiRefNetWindows => {
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
            Self::RealEsrgan | Self::RealEsrganWindows => {
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
            Self::QwenImage21
            | Self::QwenImage21Turbo
            | Self::MossImageSft
            | Self::QwenImage21Windows
            | Self::QwenImage21TurboWindows
            | Self::MossImageSftWindows => {
                validate_text_model_request(request, source, 1024)?;
            }
            Self::QwenImage21WindowsCompact
            | Self::QwenImage21TurboWindowsCompact
            | Self::MossImageSftWindowsCompact => {
                validate_text_model_request(request, source, 768)?;
            }
            Self::ZImageTurbo | Self::ZImageTurboCudaNf4 | Self::ZImageTurboCudaNf4Compact => {
                unreachable!()
            }
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
        if self == Self::ZImageTurboCudaNf4 {
            return validate_windows_z_image_plan(request, plan);
        }
        self.validate_request(request, source)?;
        plan.validate_against(request)?;
        require(
            plan.model_variant_id == request.model_variant_id
                && plan.effective_output == request.requested_output
                && plan.adjustments.is_empty(),
            "recipe cannot substitute model, output, or apply implicit adjustments",
        )?;
        validate_plan_platform_identity(self, plan)?;
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
            Self::QwenImage21 | Self::QwenImage21Turbo | Self::MossImageSft => {
                let effective_seed = text_model_effective_seed(request, plan)?;
                text_model_parameters(self, effective_seed)
            }
            Self::KleinTextWindows | Self::KleinEditWindows => {
                windows_klein_parameters(self, windows_klein_effective_seed(request, plan)?)
            }
            Self::KleinTextWindowsCompact | Self::KleinEditWindowsCompact => {
                windows_klein_compact_parameters(self, windows_klein_effective_seed(request, plan)?)
            }
            Self::BiRefNetWindows => windows_birefnet_parameters(),
            Self::RealEsrganWindows => windows_realesrgan_parameters(),
            Self::QwenImage21Windows
            | Self::QwenImage21TurboWindows
            | Self::MossImageSftWindows
            | Self::QwenImage21WindowsCompact
            | Self::QwenImage21TurboWindowsCompact
            | Self::MossImageSftWindowsCompact => {
                windows_text_model_parameters(self, text_model_effective_seed(request, plan)?)
            }
            Self::ZImageTurboCudaNf4Compact => {
                windows_z_image_compact_parameters(windows_z_image_effective_seed(request, plan)?)
            }
            Self::ZImageTurbo | Self::ZImageTurboCudaNf4 => unreachable!(),
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

pub fn freeze_realesrgan(
    input: RealEsrganInput,
    env: crate::generation::PlanEnvironment,
) -> Result<FrozenRecipe, ContractError> {
    let source = input.source;
    let width =
        source.facts.width.checked_mul(4).ok_or_else(|| {
            ContractError::InvalidValue("Real-ESRGAN output width overflow".into())
        })?;
    let height =
        source.facts.height.checked_mul(4).ok_or_else(|| {
            ContractError::InvalidValue("Real-ESRGAN output height overflow".into())
        })?;
    let request = UserRequest {
        schema_version: crate::contracts::REQUEST_SCHEMA_VERSION,
        request_id: input.request_id,
        project_id: input.project_id,
        capability: crate::contracts::CapabilityDescriptor {
            capability_id: REALESRGAN_CAPABILITY.into(),
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
        model_variant_id: ModelVariantId::new(REALESRGAN_MODEL)?,
        requested_output: crate::contracts::ImageOutput {
            width,
            height,
            count: 1,
        },
        policy: crate::contracts::AdjustmentPolicy {
            allow_model_substitution: false,
            allow_resolution_change: false,
            allow_cpu_fallback: false,
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
            ("recipe".into(), json!(Recipe::RealEsrgan.id())),
            ("recipe_version".into(), json!(1)),
            ("model".into(), json!("realesrgan-x4plus")),
            ("scale".into(), json!(4)),
            ("tile".into(), json!(128)),
            ("threads".into(), json!("1:1:1")),
            ("tta".into(), json!(false)),
        ]),
        adjustments: vec![],
    };
    Recipe::RealEsrgan.validate_plan(&request, &plan, Some(source.facts))?;
    Ok(FrozenRecipe { request, plan })
}

pub fn freeze_text_model(
    recipe: Recipe,
    input: TextModelInput,
    resolved_seed: u32,
    env: crate::generation::PlanEnvironment,
) -> Result<FrozenRecipe, ContractError> {
    let (model, capability) = match recipe {
        Recipe::QwenImage21 => (QWEN_IMAGE_21_MODEL, QWEN_IMAGE_21_CAPABILITY),
        Recipe::QwenImage21Turbo => (QWEN_IMAGE_21_TURBO_MODEL, QWEN_IMAGE_21_TURBO_CAPABILITY),
        Recipe::MossImageSft => (MOSS_IMAGE_SFT_MODEL, MOSS_IMAGE_SFT_CAPABILITY),
        _ => {
            return Err(ContractError::InvalidValue(
                "text model freezer received a non-text recipe".into(),
            ));
        }
    };
    let request = UserRequest {
        schema_version: crate::contracts::REQUEST_SCHEMA_VERSION,
        request_id: input.request_id,
        project_id: input.project_id,
        capability: crate::contracts::CapabilityDescriptor {
            capability_id: capability.into(),
            instruction_required: true,
            input_roles: vec![],
        },
        instruction: input.prompt,
        inputs: vec![],
        model_variant_id: ModelVariantId::new(model)?,
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
    let effective_seed = input.seed.unwrap_or(resolved_seed);
    let plan = ExecutionPlan {
        schema_version: crate::contracts::PLAN_SCHEMA_VERSION,
        plan_id: input.plan_id,
        request_id: request.request_id.clone(),
        model_variant_id: request.model_variant_id.clone(),
        components: env.components,
        runtime: env.runtime,
        device: env.device,
        effective_output: request.requested_output.clone(),
        effective_parameters: text_model_parameters(recipe, effective_seed),
        adjustments: vec![],
    };
    recipe.validate_plan(&request, &plan, None)?;
    Ok(FrozenRecipe { request, plan })
}

/// Freezes a Windows Klein request; a source image selects the single-image edit capability.
pub fn freeze_windows_klein(
    input: KleinInput,
    resolved_seed: u32,
    env: crate::generation::PlanEnvironment,
) -> Result<FrozenRecipe, ContractError> {
    freeze_windows_klein_for(
        Recipe::KleinTextWindows,
        Recipe::KleinEditWindows,
        1024,
        input,
        resolved_seed,
        env,
    )
}

/// Freezes the compact (8 GB VRAM) Windows Klein request at 768².
pub fn freeze_windows_klein_compact(
    input: KleinInput,
    resolved_seed: u32,
    env: crate::generation::PlanEnvironment,
) -> Result<FrozenRecipe, ContractError> {
    freeze_windows_klein_for(
        Recipe::KleinTextWindowsCompact,
        Recipe::KleinEditWindowsCompact,
        768,
        input,
        resolved_seed,
        env,
    )
}

fn freeze_windows_klein_for(
    text_recipe: Recipe,
    edit_recipe: Recipe,
    resolution: u32,
    input: KleinInput,
    resolved_seed: u32,
    env: crate::generation::PlanEnvironment,
) -> Result<FrozenRecipe, ContractError> {
    let (capability_id, recipe, inputs, source) = match input.source {
        Some(source) => (
            KLEIN_WINDOWS_EDIT_CAPABILITY,
            edit_recipe,
            vec![InputBinding {
                asset: source.asset,
                role: SOURCE_ROLE.into(),
                order: 0,
            }],
            Some(source.facts),
        ),
        None => (KLEIN_WINDOWS_TEXT_CAPABILITY, text_recipe, vec![], None),
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
        model_variant_id: ModelVariantId::new(KLEIN_WINDOWS_MODEL)?,
        requested_output: crate::contracts::ImageOutput {
            width: resolution,
            height: resolution,
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
    let effective_seed = input.seed.unwrap_or(resolved_seed);
    let plan = ExecutionPlan {
        schema_version: crate::contracts::PLAN_SCHEMA_VERSION,
        plan_id: input.plan_id,
        request_id: request.request_id.clone(),
        model_variant_id: request.model_variant_id.clone(),
        components: env.components,
        runtime: env.runtime,
        device: env.device,
        effective_output: request.requested_output.clone(),
        effective_parameters: windows_klein_effective_parameters(recipe, effective_seed),
        adjustments: vec![],
    };
    recipe.validate_plan(&request, &plan, source)?;
    Ok(FrozenRecipe { request, plan })
}

/// Freezes the Windows BiRefNet CPU recipe, preserving normalized source dimensions.
pub fn freeze_windows_birefnet(
    input: BiRefNetInput,
    env: crate::generation::PlanEnvironment,
) -> Result<FrozenRecipe, ContractError> {
    let source = input.source;
    let request = UserRequest {
        schema_version: crate::contracts::REQUEST_SCHEMA_VERSION,
        request_id: input.request_id,
        project_id: input.project_id,
        capability: crate::contracts::CapabilityDescriptor {
            capability_id: BIREFNET_WINDOWS_CAPABILITY.into(),
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
        model_variant_id: ModelVariantId::new(BIREFNET_WINDOWS_MODEL)?,
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
        effective_parameters: windows_birefnet_parameters(),
        adjustments: vec![],
    };
    Recipe::BiRefNetWindows.validate_plan(&request, &plan, Some(source.facts))?;
    Ok(FrozenRecipe { request, plan })
}

/// Freezes the Windows Real-ESRGAN recipe; output is exactly four times the source.
pub fn freeze_windows_realesrgan(
    input: RealEsrganInput,
    env: crate::generation::PlanEnvironment,
) -> Result<FrozenRecipe, ContractError> {
    let source = input.source;
    let width =
        source.facts.width.checked_mul(4).ok_or_else(|| {
            ContractError::InvalidValue("Windows Real-ESRGAN width overflow".into())
        })?;
    let height =
        source.facts.height.checked_mul(4).ok_or_else(|| {
            ContractError::InvalidValue("Windows Real-ESRGAN height overflow".into())
        })?;
    let request = UserRequest {
        schema_version: crate::contracts::REQUEST_SCHEMA_VERSION,
        request_id: input.request_id,
        project_id: input.project_id,
        capability: crate::contracts::CapabilityDescriptor {
            capability_id: REALESRGAN_WINDOWS_CAPABILITY.into(),
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
        model_variant_id: ModelVariantId::new(REALESRGAN_WINDOWS_MODEL)?,
        requested_output: crate::contracts::ImageOutput {
            width,
            height,
            count: 1,
        },
        policy: crate::contracts::AdjustmentPolicy {
            allow_model_substitution: false,
            allow_resolution_change: false,
            allow_cpu_fallback: false,
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
        effective_parameters: windows_realesrgan_parameters(),
        adjustments: vec![],
    };
    Recipe::RealEsrganWindows.validate_plan(&request, &plan, Some(source.facts))?;
    Ok(FrozenRecipe { request, plan })
}

/// Freezes a Windows staged text-to-image request (Qwen base/Turbo or MOSS-Image-SFT).
pub fn freeze_windows_text_model(
    recipe: Recipe,
    input: TextModelInput,
    resolved_seed: u32,
    env: crate::generation::PlanEnvironment,
) -> Result<FrozenRecipe, ContractError> {
    let (model, capability, resolution) = match recipe {
        Recipe::QwenImage21Windows => (
            QWEN_IMAGE_21_WINDOWS_MODEL,
            QWEN_IMAGE_21_WINDOWS_CAPABILITY,
            1024,
        ),
        Recipe::QwenImage21TurboWindows => (
            QWEN_IMAGE_21_TURBO_WINDOWS_MODEL,
            QWEN_IMAGE_21_TURBO_WINDOWS_CAPABILITY,
            1024,
        ),
        Recipe::MossImageSftWindows => (
            MOSS_IMAGE_SFT_WINDOWS_MODEL,
            MOSS_IMAGE_SFT_WINDOWS_CAPABILITY,
            1024,
        ),
        Recipe::QwenImage21WindowsCompact => (
            QWEN_IMAGE_21_WINDOWS_MODEL,
            QWEN_IMAGE_21_WINDOWS_CAPABILITY,
            768,
        ),
        Recipe::QwenImage21TurboWindowsCompact => (
            QWEN_IMAGE_21_TURBO_WINDOWS_MODEL,
            QWEN_IMAGE_21_TURBO_WINDOWS_CAPABILITY,
            768,
        ),
        Recipe::MossImageSftWindowsCompact => (
            MOSS_IMAGE_SFT_WINDOWS_MODEL,
            MOSS_IMAGE_SFT_WINDOWS_CAPABILITY,
            768,
        ),
        _ => {
            return Err(ContractError::InvalidValue(
                "Windows text model freezer received a non-Windows text recipe".into(),
            ));
        }
    };
    let request = UserRequest {
        schema_version: crate::contracts::REQUEST_SCHEMA_VERSION,
        request_id: input.request_id,
        project_id: input.project_id,
        capability: crate::contracts::CapabilityDescriptor {
            capability_id: capability.into(),
            instruction_required: true,
            input_roles: vec![],
        },
        instruction: input.prompt,
        inputs: vec![],
        model_variant_id: ModelVariantId::new(model)?,
        requested_output: crate::contracts::ImageOutput {
            width: resolution,
            height: resolution,
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
    let effective_seed = input.seed.unwrap_or(resolved_seed);
    let plan = ExecutionPlan {
        schema_version: crate::contracts::PLAN_SCHEMA_VERSION,
        plan_id: input.plan_id,
        request_id: request.request_id.clone(),
        model_variant_id: request.model_variant_id.clone(),
        components: env.components,
        runtime: env.runtime,
        device: env.device,
        effective_output: request.requested_output.clone(),
        effective_parameters: windows_text_model_parameters(recipe, effective_seed),
        adjustments: vec![],
    };
    recipe.validate_plan(&request, &plan, None)?;
    Ok(FrozenRecipe { request, plan })
}

fn validate_text_model_request(
    request: &UserRequest,
    source: Option<SourceFacts>,
    resolution: u32,
) -> Result<(), ContractError> {
    require(
        request.capability.instruction_required
            && request.capability.input_roles.is_empty()
            && request.inputs.is_empty()
            && source.is_none(),
        "Qwen/MOSS text-to-image requires text and no source image",
    )?;
    require(
        !request.instruction.trim().is_empty() && request.instruction.len() <= MAX_PROMPT_BYTES,
        "Qwen/MOSS prompt must contain text and be at most 16 KiB UTF-8",
    )?;
    require(
        request.requested_output.width == resolution
            && request.requested_output.height == resolution
            && request.requested_output.count == 1,
        format!("Qwen/MOSS requires one {resolution}x{resolution} image"),
    )?;
    require(
        request.parameters.len() == 1 && request.parameters.contains_key("seed"),
        "Qwen/MOSS requires exactly the seed request parameter",
    )?;
    if let Some(seed) = request.parameters.get("seed") {
        require(
            seed.is_null()
                || seed
                    .as_u64()
                    .is_some_and(|value| u32::try_from(value).is_ok()),
            "Qwen/MOSS seed must be null or u32",
        )?;
    }
    Ok(())
}

fn text_model_effective_seed(
    request: &UserRequest,
    plan: &ExecutionPlan,
) -> Result<u32, ContractError> {
    let expected = request
        .parameters
        .get("seed")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok());
    let actual = plan
        .effective_parameters
        .get("seed")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| {
            ContractError::InvalidValue("Qwen/MOSS effective seed must be u32".into())
        })?;
    if let Some(expected) = expected {
        require(
            actual == expected,
            "effective seed must preserve the explicit Qwen/MOSS seed",
        )?;
    }
    Ok(actual)
}

fn text_model_parameters(recipe: Recipe, seed: u32) -> BTreeMap<String, serde_json::Value> {
    let mut parameters = BTreeMap::from([
        ("seed".into(), json!(seed)),
        ("steps".into(), json!(recipe_steps(recipe))),
        ("quantization".into(), json!(8)),
        ("guidance".into(), json!(recipe_guidance(recipe))),
        ("scheduler".into(), json!(recipe_scheduler(recipe))),
        ("recipe".into(), json!(recipe.id())),
        ("recipe_version".into(), json!(1)),
        ("token_budget".into(), json!(recipe_token_budget(recipe))),
        ("truncation".into(), json!(false)),
        (
            "quantized_components".into(),
            json!(recipe_quantized_components(recipe)),
        ),
        (
            "text_encoder_precision".into(),
            json!(recipe_text_encoder_precision(recipe)),
        ),
        (
            "vae_precision".into(),
            json!(if recipe == Recipe::MossImageSft {
                "bfloat16"
            } else {
                "float32"
            }),
        ),
    ]);
    if recipe == Recipe::QwenImage21Turbo {
        parameters.insert(
            "sampling_schedule".into(),
            json!("qwen-turbo-model-index-v1"),
        );
    }
    if recipe == Recipe::MossImageSft {
        parameters.insert("thinking".into(), json!(true));
        parameters.insert("negative_prompt".into(), json!(""));
        parameters.insert("uncond_skip_layers".into(), json!([22]));
    }
    parameters
}

fn recipe_steps(recipe: Recipe) -> u32 {
    match recipe {
        Recipe::QwenImage21 => 40,
        Recipe::QwenImage21Turbo => 8,
        Recipe::MossImageSft => 50,
        _ => 0,
    }
}

fn recipe_guidance(recipe: Recipe) -> f64 {
    if recipe == Recipe::MossImageSft {
        3.0
    } else {
        1.0
    }
}

fn recipe_scheduler(recipe: Recipe) -> &'static str {
    if recipe == Recipe::MossImageSft {
        "moss_image"
    } else {
        "linear"
    }
}

fn recipe_token_budget(recipe: Recipe) -> u32 {
    if recipe == Recipe::MossImageSft {
        4096
    } else {
        2048
    }
}

fn recipe_quantized_components(recipe: Recipe) -> &'static [&'static str] {
    if recipe == Recipe::MossImageSft {
        &["transformer", "text_encoder"]
    } else {
        &["transformer"]
    }
}

fn recipe_text_encoder_precision(recipe: Recipe) -> &'static str {
    if recipe == Recipe::MossImageSft {
        "q8"
    } else {
        "bfloat16"
    }
}

fn validate_klein_text_request(
    request: &UserRequest,
    source: Option<SourceFacts>,
    resolution: u32,
) -> Result<(), ContractError> {
    require(
        request.capability.instruction_required
            && request.capability.input_roles.is_empty()
            && request.inputs.is_empty()
            && source.is_none(),
        "Klein text-to-image requires text and no source image",
    )?;
    validate_klein_instruction(request)?;
    require(
        request.requested_output.width == resolution
            && request.requested_output.height == resolution
            && request.requested_output.count == 1,
        format!("Klein text-to-image requires one {resolution}x{resolution} image"),
    )?;
    validate_seed_parameters(request)
}

fn validate_klein_edit_request(
    request: &UserRequest,
    source: Option<SourceFacts>,
    resolution: u32,
) -> Result<(), ContractError> {
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
        request.requested_output.width == resolution
            && request.requested_output.height == resolution
            && request.requested_output.count == 1,
        format!("Klein edit requires one {resolution}x{resolution} image"),
    )?;
    validate_seed_parameters(request)
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

/// Runtime, engine and device classes frozen for a Windows recipe (design.md section 11).
fn windows_plan_identity(recipe: Recipe) -> (&'static str, &'static str, &'static str) {
    match recipe {
        Recipe::KleinTextWindows
        | Recipe::KleinEditWindows
        | Recipe::QwenImage21Windows
        | Recipe::QwenImage21TurboWindows
        | Recipe::MossImageSftWindows
        | Recipe::KleinTextWindowsCompact
        | Recipe::KleinEditWindowsCompact
        | Recipe::QwenImage21WindowsCompact
        | Recipe::QwenImage21TurboWindowsCompact
        | Recipe::MossImageSftWindowsCompact => (
            WINDOWS_V2_RUNTIME,
            WINDOWS_DIFFUSERS_ENGINE,
            WINDOWS_CUDA_DEVICE,
        ),
        Recipe::BiRefNetWindows => (WINDOWS_V2_RUNTIME, WINDOWS_ONNX_ENGINE, WINDOWS_CPU_DEVICE),
        Recipe::RealEsrganWindows => (
            REALESRGAN_WINDOWS_RUNTIME,
            WINDOWS_NCNN_ENGINE,
            WINDOWS_VULKAN_DEVICE,
        ),
        Recipe::ZImageTurboCudaNf4Compact => (
            Z_IMAGE_WINDOWS_RUNTIME,
            Z_IMAGE_WINDOWS_ENGINE,
            Z_IMAGE_WINDOWS_DEVICE,
        ),
        _ => unreachable!("only Windows recipes have a Windows plan identity"),
    }
}

/// A Windows plan must bind its exact frozen runtime; a macOS plan must never bind a Windows one.
fn validate_plan_platform_identity(
    recipe: Recipe,
    plan: &ExecutionPlan,
) -> Result<(), ContractError> {
    if recipe.is_windows() {
        let (runtime, engine, device) = windows_plan_identity(recipe);
        require(
            plan.runtime.package_id == runtime
                && plan.runtime.engine == engine
                && plan.device.device_id == device,
            "Windows runtime or device identity does not match the frozen profile",
        )
    } else {
        require(
            !is_windows_runtime(&plan.runtime.package_id),
            "Windows runtime cannot serve a macOS recipe",
        )
    }
}

fn windows_klein_effective_seed(
    request: &UserRequest,
    plan: &ExecutionPlan,
) -> Result<u32, ContractError> {
    let requested = request
        .parameters
        .get("seed")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok());
    let effective = plan
        .effective_parameters
        .get("seed")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| {
            ContractError::InvalidValue("Windows Klein effective seed must be u32".into())
        })?;
    if let Some(requested) = requested {
        require(
            effective == requested,
            "effective seed must preserve the explicit Windows Klein seed",
        )?;
    }
    Ok(effective)
}

/// Klein text-to-image and edit share one frozen NF4 + component-offload map (design.md section 11).
fn windows_klein_parameters(recipe: Recipe, seed: u32) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        ("recipe".into(), json!(recipe.id())),
        ("recipe_version".into(), json!(1)),
        ("seed".into(), json!(seed)),
        ("steps".into(), json!(4)),
        ("guidance".into(), json!(1.0)),
        ("scheduler".into(), json!("flow_match_euler_discrete")),
        ("token_budget".into(), json!(512)),
        ("thinking".into(), json!(false)),
        ("truncation".into(), json!(false)),
        ("quantization".into(), json!("nf4")),
        ("load_strategy".into(), json!("nf4-component-offload-v1")),
    ])
}

/// Compact Klein keeps every standard value except 768²/fp16, sequential offload and the fp16
/// VAE that avoids the sm75 bfloat16 path.
fn windows_klein_compact_parameters(
    recipe: Recipe,
    seed: u32,
) -> BTreeMap<String, serde_json::Value> {
    let mut parameters = windows_klein_parameters(recipe, seed);
    parameters.insert("load_strategy".into(), json!("nf4-sequential-offload-v1"));
    parameters.insert("resolution".into(), json!(768));
    parameters.insert("compute_dtype".into(), json!("float16"));
    parameters.insert("vae_precision".into(), json!("float16"));
    parameters
}

fn windows_klein_effective_parameters(
    recipe: Recipe,
    seed: u32,
) -> BTreeMap<String, serde_json::Value> {
    if matches!(
        recipe,
        Recipe::KleinTextWindowsCompact | Recipe::KleinEditWindowsCompact
    ) {
        windows_klein_compact_parameters(recipe, seed)
    } else {
        windows_klein_parameters(recipe, seed)
    }
}

/// BiRefNet keeps the macOS ONNX Runtime CPU contract on the Windows v2 runtime.
fn windows_birefnet_parameters() -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        ("recipe".into(), json!(Recipe::BiRefNetWindows.id())),
        ("recipe_version".into(), json!(1)),
        ("provider".into(), json!("CPUExecutionProvider")),
        ("precision".into(), json!("float32")),
        ("preprocessing".into(), json!("birefnet-1024-rgb-v1")),
        ("postprocessing".into(), json!("birefnet-logits-sigmoid-v1")),
        ("intra_op_threads".into(), json!(4)),
        ("inter_op_threads".into(), json!(1)),
        ("execution_mode".into(), json!("sequential")),
    ])
}

/// Real-ESRGAN Windows fixes 4x, tile 128, single GPU and 1:1:1 threads with TTA disabled.
fn windows_realesrgan_parameters() -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        ("recipe".into(), json!(Recipe::RealEsrganWindows.id())),
        ("recipe_version".into(), json!(1)),
        ("model".into(), json!("realesrgan-x4plus")),
        ("scale".into(), json!(4)),
        ("tile".into(), json!(128)),
        ("threads".into(), json!("1:1:1")),
        ("tta".into(), json!(false)),
    ])
}

fn windows_qwen21_parameters(recipe: Recipe, seed: u32) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        ("recipe".into(), json!(recipe.id())),
        ("recipe_version".into(), json!(1)),
        ("seed".into(), json!(seed)),
        (
            "steps".into(),
            json!(if matches!(
                recipe,
                Recipe::QwenImage21TurboWindows | Recipe::QwenImage21TurboWindowsCompact
            ) {
                8
            } else {
                40
            }),
        ),
        ("guidance".into(), json!(1.0)),
        ("sampling".into(), json!("linear")),
        ("token_budget".into(), json!(2048)),
        ("quantization".into(), json!("nf4")),
        ("load_strategy".into(), json!("staged-nf4-v1")),
    ])
}

/// Compact Qwen base/Turbo stage the text encoder layer by layer at 768² and fp16.
fn windows_qwen21_compact_parameters(
    recipe: Recipe,
    seed: u32,
) -> BTreeMap<String, serde_json::Value> {
    let mut parameters = windows_qwen21_parameters(recipe, seed);
    parameters.insert("load_strategy".into(), json!("staged-nf4-sequential-v1"));
    parameters.insert("resolution".into(), json!(768));
    parameters.insert("compute_dtype".into(), json!("float16"));
    parameters
}

/// MOSS-Image-SFT keeps the standard sampler, including the fp32 VAE precision.
fn windows_moss_parameters(recipe: Recipe, seed: u32) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        ("recipe".into(), json!(recipe.id())),
        ("recipe_version".into(), json!(1)),
        ("seed".into(), json!(seed)),
        ("steps".into(), json!(50)),
        ("guidance".into(), json!(3.0)),
        ("sampling".into(), json!("moss_image")),
        ("token_budget".into(), json!(4096)),
        ("thinking".into(), json!(true)),
        ("negative_prompt".into(), json!("")),
        ("uncond_skip_layers".into(), json!([22])),
        (
            "quantized_components".into(),
            json!(["transformer", "text_encoder"]),
        ),
        ("text_encoder_precision".into(), json!("nf4")),
        ("vae_precision".into(), json!("float32")),
        ("load_strategy".into(), json!("staged-nf4-v1")),
    ])
}

/// Compact MOSS stages the text encoder layer by layer; sampler values stay frozen.
fn windows_moss_compact_parameters(seed: u32) -> BTreeMap<String, serde_json::Value> {
    let mut parameters = windows_moss_parameters(Recipe::MossImageSftWindowsCompact, seed);
    parameters.insert("load_strategy".into(), json!("staged-nf4-sequential-v1"));
    parameters.insert("resolution".into(), json!(768));
    parameters.insert("compute_dtype".into(), json!("float16"));
    parameters
}

/// Qwen base/Turbo and MOSS use the staged NF4 maps frozen in design.md section 11.
fn windows_text_model_parameters(recipe: Recipe, seed: u32) -> BTreeMap<String, serde_json::Value> {
    match recipe {
        Recipe::QwenImage21Windows | Recipe::QwenImage21TurboWindows => {
            let mut parameters = windows_qwen21_parameters(recipe, seed);
            if recipe == Recipe::QwenImage21TurboWindows {
                parameters.insert(
                    "sampling_schedule".into(),
                    json!("qwen-turbo-model-index-v1"),
                );
            }
            parameters
        }
        Recipe::QwenImage21WindowsCompact | Recipe::QwenImage21TurboWindowsCompact => {
            let mut parameters = windows_qwen21_compact_parameters(recipe, seed);
            if recipe == Recipe::QwenImage21TurboWindowsCompact {
                parameters.insert(
                    "sampling_schedule".into(),
                    json!("qwen-turbo-model-index-v1"),
                );
            }
            parameters
        }
        Recipe::MossImageSftWindows => windows_moss_parameters(recipe, seed),
        Recipe::MossImageSftWindowsCompact => windows_moss_compact_parameters(seed),
        _ => unreachable!("only Windows staged text models have a frozen parameter map"),
    }
}

/// Compact Z-Image keeps the historical v1 Windows runtime contract, only at 768².
fn validate_windows_z_image_compact_request(request: &UserRequest) -> Result<(), ContractError> {
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
        ) == (768, 768)
            && request.requested_output.count == 1,
        "compact Windows Z-Image requires one 768x768 image",
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

fn windows_z_image_effective_seed(
    request: &UserRequest,
    plan: &ExecutionPlan,
) -> Result<u32, ContractError> {
    let requested = request
        .parameters
        .get("seed")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok());
    let effective = plan
        .effective_parameters
        .get("seed")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| {
            ContractError::InvalidValue("Windows Z-Image effective seed must be u32".into())
        })?;
    if let Some(requested) = requested {
        require(
            effective == requested,
            "effective seed must preserve the explicit Windows Z-Image seed",
        )?;
    }
    Ok(effective)
}

/// Windows Z-Image Turbo keeps the historical v1 runtime, NF4 components and model CPU offload.
fn windows_z_image_parameters(seed: u32) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        ("seed".into(), json!(seed)),
        ("steps".into(), json!(9)),
        ("quantization".into(), json!("nf4")),
        ("guidance".into(), json!(0.0)),
        ("scheduler".into(), json!(Z_IMAGE_WINDOWS_SCHEDULER)),
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

/// Compact Z-Image: the standard map plus 768²/fp16 compute and the fp16 VAE.
pub(crate) fn windows_z_image_compact_parameters(seed: u32) -> BTreeMap<String, serde_json::Value> {
    let mut parameters = windows_z_image_parameters(seed);
    parameters.insert("recipe".into(), json!(Z_IMAGE_WINDOWS_COMPACT_RECIPE));
    parameters.insert("resolution".into(), json!(768));
    parameters.insert("compute_dtype".into(), json!("float16"));
    parameters.insert("vae_precision".into(), json!("float16"));
    parameters
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
    fn windows_z_image_has_an_independent_recipe_and_square_contract() {
        let valid = request(
            Z_IMAGE_WINDOWS_MODEL,
            Z_IMAGE_WINDOWS_CAPABILITY,
            true,
            false,
        );
        assert_eq!(
            validate_recipe_request(&valid, None).unwrap(),
            Recipe::ZImageTurboCudaNf4
        );
        assert_eq!(Recipe::ZImageTurboCudaNf4.id(), "z-image-turbo-nf4-cuda-v1");
        assert!(
            Recipe::ZImageTurboCudaNf4.accepts_output(OutputRole::Primary, AssetKind::ImageRgb)
        );
        assert!(
            !Recipe::ZImageTurboCudaNf4.accepts_output(OutputRole::Primary, AssetKind::ImageRgba)
        );

        let mut non_square = valid;
        non_square.requested_output.width = 768;
        assert!(
            validate_recipe_request(&non_square, None).is_err(),
            "Windows Z-Image must not silently accept a non-square request"
        );
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

    #[test]
    fn freeze_realesrgan_exposes_exact_four_times_output_and_tile_recipe() {
        let environment = PlanEnvironment {
            components: vec![ComponentSnapshot {
                role: "weights".into(),
                source: SourceRevision {
                    source: "fixture".into(),
                    revision: "pinned".into(),
                },
                content_hash: ContentHash::new("d".repeat(64)).unwrap(),
            }],
            runtime: RuntimeSnapshot {
                package_id: "ncnn-vulkan".into(),
                engine: "ncnn".into(),
                source: SourceRevision {
                    source: "fixture".into(),
                    revision: "pinned".into(),
                },
            },
            device: DeviceSnapshot {
                device_id: "Apple M3 Max".into(),
                precision: "vulkan-fp16".into(),
            },
            scheduler: "realesrgan-x4plus-t128-v1".into(),
        };
        let frozen = freeze_realesrgan(
            RealEsrganInput {
                request_id: RequestId::new("request:realesrgan").unwrap(),
                project_id: ProjectId::new("default").unwrap(),
                idempotency_key: IdempotencyKey::new("submit:realesrgan").unwrap(),
                plan_id: PlanId::new("plan:realesrgan").unwrap(),
                source: SourceAsset {
                    asset: asset(AssetKind::ImageRgb),
                    facts: SourceFacts {
                        width: 640,
                        height: 360,
                        has_transparent_pixels: false,
                    },
                },
            },
            environment,
        )
        .unwrap();
        assert_eq!(
            (
                frozen.request.requested_output.width,
                frozen.request.requested_output.height
            ),
            (2560, 1440)
        );
        assert_eq!(frozen.plan.effective_parameters["scale"], 4);
        assert_eq!(frozen.plan.effective_parameters["tile"], 128);
        assert_eq!(frozen.plan.effective_parameters["threads"], "1:1:1");
        assert_eq!(frozen.plan.effective_parameters["tta"], false);
    }

    #[test]
    fn qwen_and_moss_text_recipes_are_distinct_and_strict() {
        for (model, capability, recipe) in [
            (
                QWEN_IMAGE_21_MODEL,
                QWEN_IMAGE_21_CAPABILITY,
                Recipe::QwenImage21,
            ),
            (
                QWEN_IMAGE_21_TURBO_MODEL,
                QWEN_IMAGE_21_TURBO_CAPABILITY,
                Recipe::QwenImage21Turbo,
            ),
            (
                MOSS_IMAGE_SFT_MODEL,
                MOSS_IMAGE_SFT_CAPABILITY,
                Recipe::MossImageSft,
            ),
        ] {
            let valid = request(model, capability, true, false);
            assert_eq!(validate_recipe_request(&valid, None).unwrap(), recipe);
            let mut altered = valid;
            altered.requested_output.width = 768;
            assert!(validate_recipe_request(&altered, None).is_err());

            let mut missing_seed = request(model, capability, true, false);
            missing_seed.parameters.clear();
            assert!(validate_recipe_request(&missing_seed, None).is_err());
        }

        let wrong = request(
            QWEN_IMAGE_21_MODEL,
            QWEN_IMAGE_21_TURBO_CAPABILITY,
            true,
            false,
        );
        assert!(validate_recipe_request(&wrong, None).is_err());
    }

    #[test]
    fn freeze_text_models_record_seed_sampling_budget_and_precision() {
        let env = PlanEnvironment {
            components: vec![ComponentSnapshot {
                role: "transformer".into(),
                source: SourceRevision {
                    source: "fixture".into(),
                    revision: "pinned".into(),
                },
                content_hash: ContentHash::new("e".repeat(64)).unwrap(),
            }],
            runtime: RuntimeSnapshot {
                package_id: "recipes-v3".into(),
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
            scheduler: "linear".into(),
        };
        let input = || TextModelInput {
            request_id: RequestId::new("request:text-v3").unwrap(),
            project_id: ProjectId::new("default").unwrap(),
            idempotency_key: IdempotencyKey::new("submit:text-v3").unwrap(),
            plan_id: PlanId::new("plan:text-v3").unwrap(),
            prompt: "a cobalt cube".into(),
            seed: None,
        };

        let base = freeze_text_model(Recipe::QwenImage21, input(), 17, env.clone()).unwrap();
        assert_eq!(base.plan.effective_parameters["seed"], 17);
        assert_eq!(base.plan.effective_parameters["steps"], 40);
        assert_eq!(base.plan.effective_parameters["token_budget"], 2048);
        assert_eq!(
            base.plan.effective_parameters["quantized_components"],
            json!(["transformer"])
        );

        let turbo = freeze_text_model(Recipe::QwenImage21Turbo, input(), 18, env.clone()).unwrap();
        assert_eq!(turbo.plan.effective_parameters["steps"], 8);
        assert_eq!(
            turbo.plan.effective_parameters["sampling_schedule"],
            "qwen-turbo-model-index-v1"
        );

        let moss = freeze_text_model(Recipe::MossImageSft, input(), 19, env).unwrap();
        assert_eq!(moss.plan.effective_parameters["steps"], 50);
        assert_eq!(moss.plan.effective_parameters["token_budget"], 4096);
        assert_eq!(
            moss.plan.effective_parameters["uncond_skip_layers"],
            json!([22])
        );
        assert_eq!(
            moss.plan.effective_parameters["quantized_components"],
            json!(["transformer", "text_encoder"])
        );
    }

    #[test]
    fn windows_compact_recipes_keep_the_frozen_worker_maps() {
        let env = windows_v2_environment();
        let text = freeze_windows_klein_compact(klein_input(None), 17, env.clone()).unwrap();
        assert_eq!(
            text.request.requested_output,
            ImageOutput {
                width: 768,
                height: 768,
                count: 1,
            }
        );
        assert_eq!(
            text.request.capability.capability_id,
            KLEIN_WINDOWS_TEXT_CAPABILITY
        );
        assert_eq!(
            Recipe::from_request(&text.request).unwrap(),
            Recipe::KleinTextWindowsCompact
        );
        assert_eq!(
            text.plan.effective_parameters,
            value_map(&[
                ("recipe", json!("klein4b-text-windows-compact-v1")),
                ("recipe_version", json!(1)),
                ("seed", json!(17)),
                ("steps", json!(4)),
                ("guidance", json!(1.0)),
                ("scheduler", json!("flow_match_euler_discrete")),
                ("token_budget", json!(512)),
                ("thinking", json!(false)),
                ("truncation", json!(false)),
                ("quantization", json!("nf4")),
                ("load_strategy", json!("nf4-sequential-offload-v1")),
                ("resolution", json!(768)),
                ("compute_dtype", json!("float16")),
                ("vae_precision", json!("float16")),
            ])
        );

        let edit = freeze_windows_klein_compact(
            klein_input(Some(source_asset(AssetKind::ImageRgb, 512, 512, false))),
            17,
            env.clone(),
        )
        .unwrap();
        assert_eq!(
            edit.request.capability.capability_id,
            KLEIN_WINDOWS_EDIT_CAPABILITY
        );
        assert_eq!(
            edit.request.requested_output,
            ImageOutput {
                width: 768,
                height: 768,
                count: 1,
            }
        );
        assert_eq!(
            Recipe::from_request(&edit.request).unwrap(),
            Recipe::KleinEditWindowsCompact
        );
        assert_eq!(
            edit.plan.effective_parameters,
            value_map(&[
                ("recipe", json!("klein4b-edit-windows-compact-v1")),
                ("recipe_version", json!(1)),
                ("seed", json!(17)),
                ("steps", json!(4)),
                ("guidance", json!(1.0)),
                ("scheduler", json!("flow_match_euler_discrete")),
                ("token_budget", json!(512)),
                ("thinking", json!(false)),
                ("truncation", json!(false)),
                ("quantization", json!("nf4")),
                ("load_strategy", json!("nf4-sequential-offload-v1")),
                ("resolution", json!(768)),
                ("compute_dtype", json!("float16")),
                ("vae_precision", json!("float16")),
            ])
        );

        let qwen = freeze_windows_text_model(
            Recipe::QwenImage21WindowsCompact,
            text_model_input(),
            17,
            env.clone(),
        )
        .unwrap();
        assert_eq!(
            qwen.request.requested_output,
            ImageOutput {
                width: 768,
                height: 768,
                count: 1,
            }
        );
        assert_eq!(
            qwen.request.capability.capability_id,
            QWEN_IMAGE_21_WINDOWS_CAPABILITY
        );
        assert_eq!(
            Recipe::from_request(&qwen.request).unwrap(),
            Recipe::QwenImage21WindowsCompact
        );
        assert_eq!(
            qwen.plan.effective_parameters,
            value_map(&[
                ("recipe", json!("qwen21-windows-txt2img-nf4-40-compact-v1")),
                ("recipe_version", json!(1)),
                ("seed", json!(17)),
                ("steps", json!(40)),
                ("guidance", json!(1.0)),
                ("sampling", json!("linear")),
                ("token_budget", json!(2048)),
                ("quantization", json!("nf4")),
                ("load_strategy", json!("staged-nf4-sequential-v1")),
                ("resolution", json!(768)),
                ("compute_dtype", json!("float16")),
            ])
        );

        let turbo = freeze_windows_text_model(
            Recipe::QwenImage21TurboWindowsCompact,
            text_model_input(),
            18,
            env.clone(),
        )
        .unwrap();
        assert_eq!(
            turbo.request.requested_output,
            ImageOutput {
                width: 768,
                height: 768,
                count: 1,
            }
        );
        assert_eq!(
            turbo.plan.effective_parameters,
            value_map(&[
                (
                    "recipe",
                    json!("qwen21-turbo-windows-txt2img-nf4-8-compact-v1")
                ),
                ("recipe_version", json!(1)),
                ("seed", json!(18)),
                ("steps", json!(8)),
                ("guidance", json!(1.0)),
                ("sampling", json!("linear")),
                ("token_budget", json!(2048)),
                ("quantization", json!("nf4")),
                ("sampling_schedule", json!("qwen-turbo-model-index-v1")),
                ("load_strategy", json!("staged-nf4-sequential-v1")),
                ("resolution", json!(768)),
                ("compute_dtype", json!("float16")),
            ])
        );

        let moss = freeze_windows_text_model(
            Recipe::MossImageSftWindowsCompact,
            text_model_input(),
            19,
            env,
        )
        .unwrap();
        assert_eq!(
            moss.request.requested_output,
            ImageOutput {
                width: 768,
                height: 768,
                count: 1,
            }
        );
        assert_eq!(
            moss.plan.effective_parameters,
            value_map(&[
                (
                    "recipe",
                    json!("moss-image-sft-windows-txt2img-nf4-50-compact-v1")
                ),
                ("recipe_version", json!(1)),
                ("seed", json!(19)),
                ("steps", json!(50)),
                ("guidance", json!(3.0)),
                ("sampling", json!("moss_image")),
                ("token_budget", json!(4096)),
                ("thinking", json!(true)),
                ("negative_prompt", json!("")),
                ("uncond_skip_layers", json!([22])),
                (
                    "quantized_components",
                    json!(["transformer", "text_encoder"]),
                ),
                ("text_encoder_precision", json!("nf4")),
                ("vae_precision", json!("float32")),
                ("load_strategy", json!("staged-nf4-sequential-v1")),
                ("resolution", json!(768)),
                ("compute_dtype", json!("float16")),
            ])
        );
    }

    #[test]
    fn windows_compact_selection_requires_matching_recipes_and_plans() {
        let env = windows_v2_environment();
        let compact_text =
            freeze_windows_klein_compact(klein_input(None), 17, env.clone()).unwrap();
        let standard_text = freeze_windows_klein(klein_input(None), 17, env.clone()).unwrap();
        assert_eq!(
            Recipe::from_request(&compact_text.request).unwrap(),
            Recipe::KleinTextWindowsCompact
        );
        assert_eq!(
            Recipe::from_request(&standard_text.request).unwrap(),
            Recipe::KleinTextWindows
        );
        assert!(validate_recipe_plan(&compact_text.request, &compact_text.plan, None).is_ok());
        assert!(validate_recipe_plan(&standard_text.request, &standard_text.plan, None).is_ok());
        assert!(validate_recipe_plan(&compact_text.request, &standard_text.plan, None).is_err());
        assert!(validate_recipe_plan(&standard_text.request, &compact_text.plan, None).is_err());
        assert!(
            Recipe::KleinTextWindowsCompact
                .validate_request(&standard_text.request, None)
                .is_err()
        );
        assert!(
            Recipe::KleinTextWindows
                .validate_request(&compact_text.request, None)
                .is_err()
        );

        let compact_qwen = freeze_windows_text_model(
            Recipe::QwenImage21WindowsCompact,
            text_model_input(),
            17,
            env.clone(),
        )
        .unwrap();
        let standard_qwen =
            freeze_windows_text_model(Recipe::QwenImage21Windows, text_model_input(), 17, env)
                .unwrap();
        assert_eq!(
            Recipe::from_request(&compact_qwen.request).unwrap(),
            Recipe::QwenImage21WindowsCompact
        );
        assert_eq!(
            Recipe::from_request(&standard_qwen.request).unwrap(),
            Recipe::QwenImage21Windows
        );
        assert!(validate_recipe_plan(&compact_qwen.request, &compact_qwen.plan, None).is_ok());
        assert!(validate_recipe_plan(&standard_qwen.request, &standard_qwen.plan, None).is_ok());
        assert!(validate_recipe_plan(&compact_qwen.request, &standard_qwen.plan, None).is_err());
        assert!(validate_recipe_plan(&standard_qwen.request, &compact_qwen.plan, None).is_err());
    }

    #[test]
    fn windows_compact_recipe_identities_match_the_frozen_registry() {
        for (model, capability, instruction_required, source, recipe) in [
            (
                KLEIN_WINDOWS_MODEL,
                KLEIN_WINDOWS_TEXT_CAPABILITY,
                true,
                false,
                Recipe::KleinTextWindowsCompact,
            ),
            (
                KLEIN_WINDOWS_MODEL,
                KLEIN_WINDOWS_EDIT_CAPABILITY,
                true,
                true,
                Recipe::KleinEditWindowsCompact,
            ),
            (
                QWEN_IMAGE_21_WINDOWS_MODEL,
                QWEN_IMAGE_21_WINDOWS_CAPABILITY,
                true,
                false,
                Recipe::QwenImage21WindowsCompact,
            ),
            (
                QWEN_IMAGE_21_TURBO_WINDOWS_MODEL,
                QWEN_IMAGE_21_TURBO_WINDOWS_CAPABILITY,
                true,
                false,
                Recipe::QwenImage21TurboWindowsCompact,
            ),
            (
                MOSS_IMAGE_SFT_WINDOWS_MODEL,
                MOSS_IMAGE_SFT_WINDOWS_CAPABILITY,
                true,
                false,
                Recipe::MossImageSftWindowsCompact,
            ),
            (
                Z_IMAGE_WINDOWS_MODEL,
                Z_IMAGE_WINDOWS_CAPABILITY,
                true,
                false,
                Recipe::ZImageTurboCudaNf4Compact,
            ),
        ] {
            let mut compact = request(model, capability, instruction_required, source);
            compact.requested_output = ImageOutput {
                width: 768,
                height: 768,
                count: 1,
            };
            assert_eq!(Recipe::from_request(&compact).unwrap(), recipe);
            assert!(
                recipe.is_windows(),
                "{model} compact must be a Windows recipe"
            );
            assert_eq!(recipe.outputs(), &[OutputRole::Primary]);
            assert!(recipe.accepts_output(OutputRole::Primary, AssetKind::ImageRgb));
            assert!(!recipe.accepts_output(OutputRole::Primary, AssetKind::ImageRgba));
            assert_eq!(
                recipe.max_output_bytes(OutputRole::Primary),
                Some(MAX_TOOL_OUTPUT_BYTES)
            );
        }
        assert_eq!(
            Recipe::KleinTextWindowsCompact.id(),
            "klein4b-text-windows-compact-v1"
        );
        assert_eq!(
            Recipe::KleinEditWindowsCompact.id(),
            "klein4b-edit-windows-compact-v1"
        );
        assert_eq!(
            Recipe::QwenImage21WindowsCompact.id(),
            "qwen21-windows-txt2img-nf4-40-compact-v1"
        );
        assert_eq!(
            Recipe::QwenImage21TurboWindowsCompact.id(),
            "qwen21-turbo-windows-txt2img-nf4-8-compact-v1"
        );
        assert_eq!(
            Recipe::MossImageSftWindowsCompact.id(),
            "moss-image-sft-windows-txt2img-nf4-50-compact-v1"
        );
        assert_eq!(
            Recipe::ZImageTurboCudaNf4Compact.id(),
            "z-image-turbo-nf4-cuda-compact-v1"
        );
        assert_eq!(
            Recipe::KleinTextWindowsCompact.max_output_bytes(OutputRole::Mask),
            None
        );
    }

    #[test]
    fn windows_z_image_compact_recipe_keeps_the_frozen_worker_map() {
        let seed = 17;
        let standard = request(
            Z_IMAGE_WINDOWS_MODEL,
            Z_IMAGE_WINDOWS_CAPABILITY,
            true,
            false,
        );
        let mut compact = standard.clone();
        compact.requested_output = ImageOutput {
            width: 768,
            height: 768,
            count: 1,
        };
        assert_eq!(
            Recipe::from_request(&compact).unwrap(),
            Recipe::ZImageTurboCudaNf4Compact
        );
        assert_eq!(
            Recipe::from_request(&standard).unwrap(),
            Recipe::ZImageTurboCudaNf4,
            "1024 requests keep the standard Z-Image recipe"
        );
        assert_eq!(
            Recipe::ZImageTurboCudaNf4Compact.id(),
            "z-image-turbo-nf4-cuda-compact-v1"
        );
        assert!(Recipe::ZImageTurboCudaNf4Compact.is_windows());
        assert_eq!(
            Recipe::ZImageTurboCudaNf4Compact.outputs(),
            &[OutputRole::Primary]
        );
        assert!(
            Recipe::ZImageTurboCudaNf4Compact
                .accepts_output(OutputRole::Primary, AssetKind::ImageRgb)
        );
        assert!(
            !Recipe::ZImageTurboCudaNf4Compact
                .accepts_output(OutputRole::Primary, AssetKind::ImageRgba)
        );
        assert_eq!(
            Recipe::ZImageTurboCudaNf4Compact.max_output_bytes(OutputRole::Primary),
            Some(MAX_TOOL_OUTPUT_BYTES)
        );
        assert_eq!(
            Recipe::ZImageTurboCudaNf4Compact.max_output_bytes(OutputRole::Mask),
            None
        );
        assert!(validate_recipe_request(&compact, None).is_ok());
        assert!(validate_recipe_request(&standard, None).is_ok());
        assert!(
            Recipe::ZImageTurboCudaNf4Compact
                .validate_request(&standard, None)
                .is_err()
        );
        assert!(
            Recipe::ZImageTurboCudaNf4
                .validate_request(&compact, None)
                .is_err()
        );

        let mut execution_plan = plan(&compact, windows_z_image_compact_parameters(seed));
        execution_plan.runtime = RuntimeSnapshot {
            package_id: crate::generation::Z_IMAGE_WINDOWS_RUNTIME.into(),
            engine: crate::generation::Z_IMAGE_WINDOWS_ENGINE.into(),
            source: SourceRevision {
                source: "fixture".into(),
                revision: "pinned".into(),
            },
        };
        execution_plan.device = DeviceSnapshot {
            device_id: crate::generation::Z_IMAGE_WINDOWS_DEVICE.into(),
            precision: crate::generation::Z_IMAGE_WINDOWS_PRECISION.into(),
        };
        assert_eq!(
            validate_recipe_plan(&compact, &execution_plan, None).unwrap(),
            Recipe::ZImageTurboCudaNf4Compact
        );
        assert_eq!(
            execution_plan.effective_parameters,
            value_map(&[
                ("seed", json!(seed)),
                ("steps", json!(9)),
                ("quantization", json!("nf4")),
                ("guidance", json!(0.0)),
                ("scheduler", json!("FlowMatchEulerDiscreteScheduler")),
                ("recipe", json!("z-image-turbo-nf4-cuda-compact-v1")),
                ("recipe_version", json!(1)),
                ("token_budget", json!(512)),
                ("truncation", json!(false)),
                ("thinking", json!(true)),
                (
                    "quantized_components",
                    json!(["transformer", "text_encoder"]),
                ),
                ("compute_dtype", json!("float16")),
                ("vae_precision", json!("float16")),
                ("attention", json!("_native_efficient")),
                ("offload", json!("model_cpu")),
                ("resolution", json!(768)),
            ])
        );

        let mut wrong = execution_plan.clone();
        wrong.effective_parameters = windows_z_image_parameters(seed);
        assert!(
            validate_recipe_plan(&compact, &wrong, None).is_err(),
            "compact recipe accepted the standard parameter map"
        );

        let mut wrong = execution_plan.clone();
        wrong.runtime.package_id = crate::generation::WINDOWS_V2_RUNTIME.into();
        assert!(
            validate_recipe_plan(&compact, &wrong, None).is_err(),
            "compact recipe accepted the shared v2 runtime"
        );

        let mut wrong = compact.clone();
        wrong.requested_output = standard.requested_output.clone();
        assert_eq!(
            Recipe::from_request(&wrong).unwrap(),
            Recipe::ZImageTurboCudaNf4,
            "1024 requests keep the standard Z-Image recipe"
        );
        assert!(
            Recipe::ZImageTurboCudaNf4Compact
                .validate_request(&wrong, None)
                .is_err(),
            "compact recipe accepted a 1024 output"
        );

        let mut wrong = execution_plan.clone();
        wrong.effective_output = standard.requested_output.clone();
        assert!(
            validate_recipe_plan(&compact, &wrong, None).is_err(),
            "compact plan accepted a 1024 output"
        );

        let mut standard_plan = plan(&standard, windows_z_image_parameters(seed));
        standard_plan.runtime = execution_plan.runtime.clone();
        standard_plan.device = execution_plan.device.clone();
        assert!(validate_recipe_plan(&standard, &standard_plan, None).is_ok());
        let mut wrong = plan(&standard, windows_z_image_compact_parameters(seed));
        wrong.runtime = execution_plan.runtime.clone();
        wrong.device = execution_plan.device.clone();
        assert!(
            validate_recipe_plan(&standard, &wrong, None).is_err(),
            "standard recipe accepted compact parameters"
        );
    }

    fn value_map(entries: &[(&str, serde_json::Value)]) -> BTreeMap<String, serde_json::Value> {
        entries
            .iter()
            .map(|(key, value)| ((*key).into(), value.clone()))
            .collect()
    }

    fn windows_environment(runtime: &str, engine: &str, device_id: &str) -> PlanEnvironment {
        PlanEnvironment {
            components: vec![ComponentSnapshot {
                role: "transformer".into(),
                source: SourceRevision {
                    source: "fixture".into(),
                    revision: "pinned".into(),
                },
                content_hash: ContentHash::new("9".repeat(64)).unwrap(),
            }],
            runtime: RuntimeSnapshot {
                package_id: runtime.into(),
                engine: engine.into(),
                source: SourceRevision {
                    source: "fixture".into(),
                    revision: "pinned".into(),
                },
            },
            device: DeviceSnapshot {
                device_id: device_id.into(),
                precision: "nf4".into(),
            },
            scheduler: "flow_match_euler_discrete".into(),
        }
    }

    fn windows_v2_environment() -> PlanEnvironment {
        windows_environment(
            crate::generation::WINDOWS_V2_RUNTIME,
            crate::generation::WINDOWS_DIFFUSERS_ENGINE,
            crate::generation::WINDOWS_CUDA_DEVICE,
        )
    }

    fn windows_onnx_environment() -> PlanEnvironment {
        windows_environment(
            crate::generation::WINDOWS_V2_RUNTIME,
            crate::generation::WINDOWS_ONNX_ENGINE,
            crate::generation::WINDOWS_CPU_DEVICE,
        )
    }

    fn windows_vulkan_environment() -> PlanEnvironment {
        windows_environment(
            crate::generation::REALESRGAN_WINDOWS_RUNTIME,
            crate::generation::WINDOWS_NCNN_ENGINE,
            crate::generation::WINDOWS_VULKAN_DEVICE,
        )
    }

    fn klein_input(source: Option<SourceAsset>) -> KleinInput {
        KleinInput {
            request_id: RequestId::new("request:windows-klein").unwrap(),
            project_id: ProjectId::new("default").unwrap(),
            idempotency_key: IdempotencyKey::new("submit:windows-klein").unwrap(),
            plan_id: PlanId::new("plan:windows-klein").unwrap(),
            prompt: "a cobalt cube".into(),
            seed: None,
            source,
        }
    }

    fn source_asset(kind: AssetKind, width: u32, height: u32, transparent: bool) -> SourceAsset {
        SourceAsset {
            asset: asset(kind),
            facts: SourceFacts {
                width,
                height,
                has_transparent_pixels: transparent,
            },
        }
    }

    fn birefnet_input(source: SourceAsset) -> BiRefNetInput {
        BiRefNetInput {
            request_id: RequestId::new("request:windows-birefnet").unwrap(),
            project_id: ProjectId::new("default").unwrap(),
            idempotency_key: IdempotencyKey::new("submit:windows-birefnet").unwrap(),
            plan_id: PlanId::new("plan:windows-birefnet").unwrap(),
            source,
        }
    }

    fn realesrgan_input(source: SourceAsset) -> RealEsrganInput {
        RealEsrganInput {
            request_id: RequestId::new("request:windows-realesrgan").unwrap(),
            project_id: ProjectId::new("default").unwrap(),
            idempotency_key: IdempotencyKey::new("submit:windows-realesrgan").unwrap(),
            plan_id: PlanId::new("plan:windows-realesrgan").unwrap(),
            source,
        }
    }

    fn text_model_input() -> TextModelInput {
        TextModelInput {
            request_id: RequestId::new("request:windows-text").unwrap(),
            project_id: ProjectId::new("default").unwrap(),
            idempotency_key: IdempotencyKey::new("submit:windows-text").unwrap(),
            plan_id: PlanId::new("plan:windows-text").unwrap(),
            prompt: "a cobalt cube".into(),
            seed: None,
        }
    }

    fn macos_environment() -> PlanEnvironment {
        PlanEnvironment {
            components: vec![ComponentSnapshot {
                role: "transformer".into(),
                source: SourceRevision {
                    source: "fixture".into(),
                    revision: "pinned".into(),
                },
                content_hash: ContentHash::new("8".repeat(64)).unwrap(),
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
        }
    }

    #[test]
    fn windows_recipe_identities_match_the_frozen_registry() {
        for (model, capability, instruction_required, source, recipe) in [
            (
                KLEIN_WINDOWS_MODEL,
                KLEIN_WINDOWS_TEXT_CAPABILITY,
                true,
                false,
                Recipe::KleinTextWindows,
            ),
            (
                KLEIN_WINDOWS_MODEL,
                KLEIN_WINDOWS_EDIT_CAPABILITY,
                true,
                true,
                Recipe::KleinEditWindows,
            ),
            (
                BIREFNET_WINDOWS_MODEL,
                BIREFNET_WINDOWS_CAPABILITY,
                false,
                true,
                Recipe::BiRefNetWindows,
            ),
            (
                REALESRGAN_WINDOWS_MODEL,
                REALESRGAN_WINDOWS_CAPABILITY,
                false,
                true,
                Recipe::RealEsrganWindows,
            ),
            (
                QWEN_IMAGE_21_WINDOWS_MODEL,
                QWEN_IMAGE_21_WINDOWS_CAPABILITY,
                true,
                false,
                Recipe::QwenImage21Windows,
            ),
            (
                QWEN_IMAGE_21_TURBO_WINDOWS_MODEL,
                QWEN_IMAGE_21_TURBO_WINDOWS_CAPABILITY,
                true,
                false,
                Recipe::QwenImage21TurboWindows,
            ),
            (
                MOSS_IMAGE_SFT_WINDOWS_MODEL,
                MOSS_IMAGE_SFT_WINDOWS_CAPABILITY,
                true,
                false,
                Recipe::MossImageSftWindows,
            ),
        ] {
            let request = request(model, capability, instruction_required, source);
            assert_eq!(Recipe::from_request(&request).unwrap(), recipe);
            assert!(recipe.is_windows(), "{model} must be a Windows recipe");
        }
        assert_eq!(Recipe::KleinTextWindows.id(), KLEIN_WINDOWS_TEXT_RECIPE);
        assert_eq!(Recipe::KleinEditWindows.id(), KLEIN_WINDOWS_EDIT_RECIPE);
        assert_eq!(Recipe::BiRefNetWindows.id(), BIREFNET_WINDOWS_RECIPE);
        assert_eq!(Recipe::RealEsrganWindows.id(), REALESRGAN_WINDOWS_RECIPE);
        assert_eq!(
            Recipe::QwenImage21Windows.id(),
            QWEN_IMAGE_21_WINDOWS_RECIPE
        );
        assert_eq!(
            Recipe::QwenImage21TurboWindows.id(),
            QWEN_IMAGE_21_TURBO_WINDOWS_RECIPE
        );
        assert_eq!(
            Recipe::MossImageSftWindows.id(),
            MOSS_IMAGE_SFT_WINDOWS_RECIPE
        );
        assert!(!Recipe::KleinText.is_windows());
        assert!(!Recipe::QwenImage21.is_windows());
        assert!(Recipe::ZImageTurboCudaNf4.is_windows());
    }

    #[test]
    fn windows_klein_freezes_exact_nf4_component_offload_map() {
        let env = windows_v2_environment();
        let text = freeze_windows_klein(klein_input(None), 17, env.clone()).unwrap();
        assert_eq!(
            text.request.capability.capability_id,
            KLEIN_WINDOWS_TEXT_CAPABILITY
        );
        assert_eq!(text.request.model_variant_id.as_str(), KLEIN_WINDOWS_MODEL);
        assert!(text.request.parameters["seed"].is_null());
        assert_eq!(
            text.request.requested_output,
            ImageOutput {
                width: 1024,
                height: 1024,
                count: 1,
            }
        );
        assert_eq!(
            text.plan.effective_parameters,
            value_map(&[
                ("recipe", json!(KLEIN_WINDOWS_TEXT_RECIPE)),
                ("recipe_version", json!(1)),
                ("seed", json!(17)),
                ("steps", json!(4)),
                ("guidance", json!(1.0)),
                ("scheduler", json!("flow_match_euler_discrete")),
                ("token_budget", json!(512)),
                ("thinking", json!(false)),
                ("truncation", json!(false)),
                ("quantization", json!("nf4")),
                ("load_strategy", json!("nf4-component-offload-v1")),
            ])
        );

        let edit = freeze_windows_klein(
            klein_input(Some(source_asset(AssetKind::ImageRgb, 512, 512, false))),
            17,
            env,
        )
        .unwrap();
        assert_eq!(
            edit.request.capability.capability_id,
            KLEIN_WINDOWS_EDIT_CAPABILITY
        );
        assert_eq!(edit.request.inputs.len(), 1);
        assert_eq!(edit.request.inputs[0].role, SOURCE_ROLE);
        assert_eq!(
            edit.plan.effective_parameters,
            value_map(&[
                ("recipe", json!(KLEIN_WINDOWS_EDIT_RECIPE)),
                ("recipe_version", json!(1)),
                ("seed", json!(17)),
                ("steps", json!(4)),
                ("guidance", json!(1.0)),
                ("scheduler", json!("flow_match_euler_discrete")),
                ("token_budget", json!(512)),
                ("thinking", json!(false)),
                ("truncation", json!(false)),
                ("quantization", json!("nf4")),
                ("load_strategy", json!("nf4-component-offload-v1")),
            ])
        );
        assert!(
            freeze_windows_klein(klein_input(None), 17, windows_vulkan_environment()).is_err(),
            "Windows Klein must bind the shared v2 diffusers runtime"
        );
    }

    #[test]
    fn windows_birefnet_freezes_dual_output_cpu_map() {
        let frozen = freeze_windows_birefnet(
            birefnet_input(source_asset(AssetKind::ImageRgba, 768, 1024, true)),
            windows_onnx_environment(),
        )
        .unwrap();
        assert_eq!(frozen.request.instruction, "");
        assert!(frozen.request.parameters.is_empty());
        assert_eq!(
            frozen.plan.effective_output,
            ImageOutput {
                width: 768,
                height: 1024,
                count: 1,
            }
        );
        assert_eq!(
            frozen.plan.effective_parameters,
            value_map(&[
                ("recipe", json!(BIREFNET_WINDOWS_RECIPE)),
                ("recipe_version", json!(1)),
                ("provider", json!("CPUExecutionProvider")),
                ("precision", json!("float32")),
                ("preprocessing", json!("birefnet-1024-rgb-v1")),
                ("postprocessing", json!("birefnet-logits-sigmoid-v1")),
                ("intra_op_threads", json!(4)),
                ("inter_op_threads", json!(1)),
                ("execution_mode", json!("sequential")),
            ])
        );
        assert_eq!(
            Recipe::BiRefNetWindows.outputs(),
            &[OutputRole::Primary, OutputRole::Mask]
        );
        assert!(Recipe::BiRefNetWindows.accepts_output(OutputRole::Primary, AssetKind::ImageRgba));
        assert!(Recipe::BiRefNetWindows.accepts_output(OutputRole::Mask, AssetKind::Mask));
        assert!(!Recipe::BiRefNetWindows.accepts_output(OutputRole::Primary, AssetKind::ImageRgb));
        let wrong_runtime = freeze_windows_birefnet(
            birefnet_input(source_asset(AssetKind::ImageRgba, 768, 1024, true)),
            windows_v2_environment(),
        );
        assert!(
            wrong_runtime.is_err(),
            "BiRefNet must bind the ONNX CPU engine"
        );
    }

    #[test]
    fn windows_realesrgan_freezes_exact_four_times_output() {
        let frozen = freeze_windows_realesrgan(
            realesrgan_input(source_asset(AssetKind::ImageRgb, 640, 360, false)),
            windows_vulkan_environment(),
        )
        .unwrap();
        assert_eq!(
            frozen.plan.effective_output,
            ImageOutput {
                width: 2560,
                height: 1440,
                count: 1,
            }
        );
        assert_eq!(
            frozen.plan.effective_parameters,
            value_map(&[
                ("recipe", json!(REALESRGAN_WINDOWS_RECIPE)),
                ("recipe_version", json!(1)),
                ("model", json!("realesrgan-x4plus")),
                ("scale", json!(4)),
                ("tile", json!(128)),
                ("threads", json!("1:1:1")),
                ("tta", json!(false)),
            ])
        );
        assert_eq!(Recipe::RealEsrganWindows.outputs(), &[OutputRole::Primary]);
        assert_eq!(
            Recipe::RealEsrganWindows.max_output_bytes(OutputRole::Primary),
            Some(MAX_UPSCALE_OUTPUT_BYTES)
        );
        assert!(
            freeze_windows_realesrgan(
                realesrgan_input(source_asset(AssetKind::ImageRgb, 640, 360, false)),
                windows_v2_environment(),
            )
            .is_err(),
            "Real-ESRGAN must bind the ncnn Vulkan runtime"
        );
    }

    #[test]
    fn windows_staged_text_models_freeze_exact_maps() {
        let env = windows_v2_environment();
        let qwen = freeze_windows_text_model(
            Recipe::QwenImage21Windows,
            text_model_input(),
            17,
            env.clone(),
        )
        .unwrap();
        assert_eq!(
            qwen.request.capability.capability_id,
            QWEN_IMAGE_21_WINDOWS_CAPABILITY
        );
        assert_eq!(
            qwen.request.model_variant_id.as_str(),
            QWEN_IMAGE_21_WINDOWS_MODEL
        );
        assert_eq!(
            qwen.plan.effective_parameters,
            value_map(&[
                ("recipe", json!(QWEN_IMAGE_21_WINDOWS_RECIPE)),
                ("recipe_version", json!(1)),
                ("seed", json!(17)),
                ("steps", json!(40)),
                ("guidance", json!(1.0)),
                ("sampling", json!("linear")),
                ("token_budget", json!(2048)),
                ("quantization", json!("nf4")),
                ("load_strategy", json!("staged-nf4-v1")),
            ])
        );

        let turbo = freeze_windows_text_model(
            Recipe::QwenImage21TurboWindows,
            text_model_input(),
            18,
            env.clone(),
        )
        .unwrap();
        assert_eq!(
            turbo.plan.effective_parameters,
            value_map(&[
                ("recipe", json!(QWEN_IMAGE_21_TURBO_WINDOWS_RECIPE)),
                ("recipe_version", json!(1)),
                ("seed", json!(18)),
                ("steps", json!(8)),
                ("guidance", json!(1.0)),
                ("sampling", json!("linear")),
                ("token_budget", json!(2048)),
                ("quantization", json!("nf4")),
                ("sampling_schedule", json!("qwen-turbo-model-index-v1"),),
                ("load_strategy", json!("staged-nf4-v1")),
            ])
        );

        let moss = freeze_windows_text_model(
            Recipe::MossImageSftWindows,
            text_model_input(),
            19,
            env.clone(),
        )
        .unwrap();
        assert_eq!(
            moss.plan.effective_parameters,
            value_map(&[
                ("recipe", json!(MOSS_IMAGE_SFT_WINDOWS_RECIPE)),
                ("recipe_version", json!(1)),
                ("seed", json!(19)),
                ("steps", json!(50)),
                ("guidance", json!(3.0)),
                ("sampling", json!("moss_image")),
                ("token_budget", json!(4096)),
                ("thinking", json!(true)),
                ("negative_prompt", json!("")),
                ("uncond_skip_layers", json!([22])),
                (
                    "quantized_components",
                    json!(["transformer", "text_encoder"]),
                ),
                ("text_encoder_precision", json!("nf4")),
                ("vae_precision", json!("float32")),
                ("load_strategy", json!("staged-nf4-v1")),
            ])
        );

        assert!(
            freeze_windows_text_model(Recipe::QwenImage21, text_model_input(), 17, env).is_err(),
            "the Windows freezer must reject macOS recipes"
        );
    }

    #[test]
    fn windows_plans_reject_parameter_and_identity_drift() {
        let env = windows_v2_environment();
        let frozen = [
            (
                freeze_windows_klein(klein_input(None), 17, env.clone()).unwrap(),
                None,
            ),
            (
                freeze_windows_birefnet(
                    birefnet_input(source_asset(AssetKind::ImageRgba, 512, 512, true)),
                    windows_onnx_environment(),
                )
                .unwrap(),
                Some(SourceFacts {
                    width: 512,
                    height: 512,
                    has_transparent_pixels: true,
                }),
            ),
            (
                freeze_windows_realesrgan(
                    realesrgan_input(source_asset(AssetKind::ImageRgb, 512, 512, false)),
                    windows_vulkan_environment(),
                )
                .unwrap(),
                Some(SourceFacts {
                    width: 512,
                    height: 512,
                    has_transparent_pixels: false,
                }),
            ),
            (
                freeze_windows_text_model(Recipe::QwenImage21Windows, text_model_input(), 17, env)
                    .unwrap(),
                None,
            ),
        ];
        for (frozen, source) in frozen {
            let mutations: [fn(&mut ExecutionPlan); 8] = [
                |plan| {
                    plan.effective_parameters.insert("steps".into(), json!(5));
                },
                |plan| {
                    plan.effective_parameters
                        .insert("quantization".into(), json!(4));
                },
                |plan| {
                    plan.effective_parameters.remove("recipe_version");
                },
                |plan| {
                    plan.effective_parameters.remove("recipe");
                },
                |plan| {
                    plan.runtime.package_id = "mflux-0.24.0".into();
                },
                |plan| {
                    plan.runtime.engine = "mlx".into();
                },
                |plan| {
                    plan.device.device_id = "metal".into();
                },
                |plan| {
                    plan.effective_output.count = 2;
                },
            ];
            validate_recipe_plan(&frozen.request, &frozen.plan, source).unwrap();
            for mutate in mutations {
                let mut plan = frozen.plan.clone();
                mutate(&mut plan);
                assert!(
                    validate_recipe_plan(&frozen.request, &plan, source).is_err(),
                    "frozen Windows plan accepted a mutated field"
                );
            }
        }
    }

    #[test]
    fn macos_recipes_reject_windows_runtime_plans() {
        let env = macos_environment();
        let frozen = freeze_klein(klein_input(None), 17, env.clone()).unwrap();
        let mut tampered = frozen.plan.clone();
        tampered.runtime.package_id = crate::generation::WINDOWS_V2_RUNTIME.into();
        assert!(validate_recipe_plan(&frozen.request, &tampered, None).is_err());

        let frozen = freeze_text_model(Recipe::QwenImage21, text_model_input(), 17, env).unwrap();
        let mut tampered = frozen.plan.clone();
        tampered.runtime.package_id = crate::generation::WINDOWS_V2_RUNTIME.into();
        assert!(validate_recipe_plan(&frozen.request, &tampered, None).is_err());

        assert!(
            Recipe::from_request(&request(
                KLEIN_MODEL,
                KLEIN_WINDOWS_TEXT_CAPABILITY,
                true,
                false
            ))
            .is_err()
        );
        assert!(
            Recipe::from_request(&request(
                KLEIN_WINDOWS_MODEL,
                KLEIN_TEXT_CAPABILITY,
                true,
                false
            ))
            .is_err()
        );
    }

    #[test]
    fn windows_recipe_registry_keeps_z_image_behaviour() {
        assert_eq!(
            Recipe::from_request(&request(
                Z_IMAGE_WINDOWS_MODEL,
                Z_IMAGE_WINDOWS_CAPABILITY,
                true,
                false
            ))
            .unwrap(),
            Recipe::ZImageTurboCudaNf4
        );
        assert_eq!(Recipe::ZImageTurboCudaNf4.id(), "z-image-turbo-nf4-cuda-v1");
        assert_eq!(Recipe::ZImageTurboCudaNf4.outputs(), &[OutputRole::Primary]);
        assert_ne!(
            crate::generation::WINDOWS_V2_RUNTIME,
            crate::generation::Z_IMAGE_WINDOWS_RUNTIME
        );
        assert_eq!(
            Recipe::ZImageTurbo.id(),
            "z-image-turbo-q4-v1",
            "the macOS Z-Image identity must not change"
        );
    }

    #[test]
    fn windows_outputs_and_klein_edit_source_requirements_are_exact() {
        let facts = SourceFacts {
            width: 512,
            height: 512,
            has_transparent_pixels: false,
        };
        let mut edit = request(
            KLEIN_WINDOWS_MODEL,
            KLEIN_WINDOWS_EDIT_CAPABILITY,
            true,
            true,
        );
        assert!(validate_recipe_request(&edit, Some(facts)).is_ok());
        edit.inputs.clear();
        assert!(validate_recipe_request(&edit, Some(facts)).is_err());

        let mut two_sources = request(
            KLEIN_WINDOWS_MODEL,
            KLEIN_WINDOWS_EDIT_CAPABILITY,
            true,
            true,
        );
        two_sources.inputs.push(two_sources.inputs[0].clone());
        assert!(validate_recipe_request(&two_sources, Some(facts)).is_err());

        let text = request(
            KLEIN_WINDOWS_MODEL,
            KLEIN_WINDOWS_TEXT_CAPABILITY,
            true,
            false,
        );
        assert!(validate_recipe_request(&text, None).is_ok());

        for recipe in [
            Recipe::KleinTextWindows,
            Recipe::KleinEditWindows,
            Recipe::RealEsrganWindows,
            Recipe::QwenImage21Windows,
            Recipe::QwenImage21TurboWindows,
            Recipe::MossImageSftWindows,
        ] {
            assert_eq!(recipe.outputs(), &[OutputRole::Primary]);
            assert!(recipe.accepts_output(OutputRole::Primary, AssetKind::ImageRgb));
            assert!(!recipe.accepts_output(OutputRole::Primary, AssetKind::ImageRgba));
        }
        assert_eq!(
            Recipe::KleinTextWindows.max_output_bytes(OutputRole::Mask),
            None
        );
        assert_eq!(
            Recipe::BiRefNetWindows.max_output_bytes(OutputRole::Mask),
            Some(MAX_TOOL_OUTPUT_BYTES)
        );
        assert_eq!(
            Recipe::BiRefNetWindows.max_output_bytes(OutputRole::Primary),
            Some(MAX_TOOL_OUTPUT_BYTES)
        );
        assert_eq!(
            Recipe::KleinTextWindows.max_output_bytes(OutputRole::Primary),
            Some(MAX_TOOL_OUTPUT_BYTES)
        );
    }
}
