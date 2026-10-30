//! Versioned data contracts. Use from_json/to_json for validated document IO.
use crate::{ContractError, domain::*, nonempty, require};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const REQUEST_SCHEMA_VERSION: u32 = 1;
pub const PLAN_SCHEMA_VERSION: u32 = 1;
pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

fn version(found: u32, expected: u32, document: &'static str) -> Result<(), ContractError> {
    if found != expected {
        return Err(ContractError::UnsupportedVersion {
            document,
            found: found.into(),
            supported: expected,
        });
    }
    Ok(())
}

fn field_error(error: serde_json::Error) -> ContractError {
    let message = error.to_string();
    // Serde's structural diagnostics carry the missing/unknown field name.
    if let Some(rest) = message.strip_prefix("missing field `") {
        return ContractError::MissingField(rest.split('`').next().unwrap_or(rest).into());
    }
    if let Some(rest) = message.strip_prefix("unknown field `") {
        return ContractError::UnknownField(rest.split('`').next().unwrap_or(rest).into());
    }
    ContractError::InvalidValue(message)
}

fn decode<T: DeserializeOwned>(
    input: &str,
    expected: u32,
    document: &'static str,
) -> Result<T, ContractError> {
    let value: Value = serde_json::from_str(input)
        .map_err(|error| ContractError::InvalidJson(error.to_string()))?;
    let object = value
        .as_object()
        .ok_or_else(|| ContractError::InvalidValue("document must be an object".into()))?;
    let found = object
        .get("schema_version")
        .ok_or_else(|| ContractError::MissingField("schema_version".into()))?
        .as_u64()
        .ok_or_else(|| {
            ContractError::InvalidValue("schema_version must be an unsigned integer".into())
        })?;
    if found != u64::from(expected) {
        return Err(ContractError::UnsupportedVersion {
            document,
            found,
            supported: expected,
        });
    }
    serde_json::from_value(value).map_err(field_error)
}

macro_rules! document_io {
    ($name:ident, $version:ident, $kind:literal) => {
        impl $name {
            pub fn from_json(input: &str) -> Result<Self, ContractError> {
                let document: Self = decode(input, $version, $kind)?;
                document.validate()?;
                Ok(document)
            }
            pub fn to_json(&self) -> Result<String, ContractError> {
                self.validate()?;
                serde_json::to_string_pretty(self)
                    .map_err(|error| ContractError::InvalidJson(error.to_string()))
            }
        }
    };
}

fn label(value: &str, field: &str) -> Result<(), ContractError> {
    require(
        !value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c)),
        format!("{field} must be a nonempty stable label"),
    )
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleRule {
    pub role: String,
    pub minimum: u32,
    pub maximum: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityDescriptor {
    pub capability_id: String,
    pub instruction_required: bool,
    pub input_roles: Vec<RoleRule>,
}

impl CapabilityDescriptor {
    pub fn validate(&self) -> Result<(), ContractError> {
        label(&self.capability_id, "capability_id")?;
        require(
            self.capability_id.contains('.'),
            "capability_id requires a namespace",
        )?;
        let mut names = BTreeSet::new();
        for rule in &self.input_roles {
            label(&rule.role, "input role")?;
            require(
                rule.minimum <= rule.maximum,
                "input role minimum exceeds maximum",
            )?;
            require(names.insert(&rule.role), "capability repeats an input role")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputBinding {
    pub asset: AssetRef,
    pub role: String,
    pub order: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageOutput {
    pub width: u32,
    pub height: u32,
    pub count: u32,
}

impl ImageOutput {
    pub fn validate(&self) -> Result<(), ContractError> {
        require(
            self.width > 0 && self.height > 0 && self.count > 0,
            "output dimensions and count must be positive",
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdjustmentPolicy {
    pub allow_model_substitution: bool,
    pub allow_resolution_change: bool,
    pub allow_cpu_fallback: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserRequest {
    pub schema_version: u32,
    pub request_id: RequestId,
    pub project_id: ProjectId,
    pub capability: CapabilityDescriptor,
    pub instruction: String,
    pub inputs: Vec<InputBinding>,
    pub model_variant_id: ModelVariantId,
    pub requested_output: ImageOutput,
    pub policy: AdjustmentPolicy,
    pub parameters: BTreeMap<String, Value>,
    pub idempotency_key: IdempotencyKey,
}

impl UserRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version, REQUEST_SCHEMA_VERSION, "user_request")?;
        self.capability.validate()?;
        if self.capability.instruction_required {
            nonempty(&self.instruction, "instruction")?;
        }
        require(!self.instruction.contains('\0'), "instruction contains NUL")?;
        self.requested_output.validate()?;
        let mut orders = BTreeSet::new();
        let mut counts = BTreeMap::<&str, u32>::new();
        let mut identities = BTreeMap::new();
        for input in &self.inputs {
            input.asset.validate()?;
            if let Some(previous) =
                identities.insert(&input.asset.asset_id, &input.asset.content_hash)
            {
                require(
                    previous == &input.asset.content_hash,
                    "one asset identity cannot refer to different content hashes",
                )?;
            }
            require(
                self.capability
                    .input_roles
                    .iter()
                    .any(|rule| rule.role == input.role),
                format!("unsupported input role: {}", input.role),
            )?;
            require(orders.insert(input.order), "input orders must be unique")?;
            *counts.entry(&input.role).or_default() += 1;
        }
        for rule in &self.capability.input_roles {
            let count = counts.get(rule.role.as_str()).copied().unwrap_or(0);
            require(
                count >= rule.minimum && count <= rule.maximum,
                format!(
                    "input count for {} is outside its capability rule",
                    rule.role
                ),
            )?;
        }
        Ok(())
    }
}

document_io!(UserRequest, REQUEST_SCHEMA_VERSION, "user_request");

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRevision {
    pub source: String,
    pub revision: String,
}

impl SourceRevision {
    fn validate(&self) -> Result<(), ContractError> {
        nonempty(&self.source, "source")?;
        nonempty(&self.revision, "revision")
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentSnapshot {
    pub role: String,
    pub source: SourceRevision,
    pub content_hash: ContentHash,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeSnapshot {
    pub package_id: String,
    pub engine: String,
    pub source: SourceRevision,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceSnapshot {
    pub device_id: String,
    pub precision: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanAdjustment {
    pub field: String,
    pub requested: Value,
    pub effective: Value,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPlan {
    pub schema_version: u32,
    pub plan_id: PlanId,
    pub request_id: RequestId,
    pub model_variant_id: ModelVariantId,
    pub components: Vec<ComponentSnapshot>,
    pub runtime: RuntimeSnapshot,
    pub device: DeviceSnapshot,
    pub effective_output: ImageOutput,
    pub effective_parameters: BTreeMap<String, Value>,
    pub adjustments: Vec<PlanAdjustment>,
}

impl ExecutionPlan {
    pub fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version, PLAN_SCHEMA_VERSION, "execution_plan")?;
        require(
            !self.components.is_empty(),
            "plan requires at least one component snapshot",
        )?;
        let mut roles = BTreeSet::new();
        for component in &self.components {
            label(&component.role, "component role")?;
            require(
                roles.insert(&component.role),
                "component roles must be unique",
            )?;
            component.source.validate()?;
        }
        label(&self.runtime.package_id, "runtime package")?;
        nonempty(&self.runtime.engine, "engine")?;
        self.runtime.source.validate()?;
        nonempty(&self.device.device_id, "device_id")?;
        nonempty(&self.device.precision, "precision")?;
        self.effective_output.validate()?;
        let mut changed_fields = BTreeSet::new();
        for adjustment in &self.adjustments {
            nonempty(&adjustment.field, "adjustment field")?;
            nonempty(&adjustment.reason, "adjustment reason")?;
            require(
                adjustment.requested != adjustment.effective,
                "adjustment must describe an actual change",
            )?;
            require(
                changed_fields.insert(&adjustment.field),
                "adjustment field must be unique",
            )?;
        }
        Ok(())
    }

    pub fn validate_against(&self, request: &UserRequest) -> Result<(), ContractError> {
        self.validate()?;
        request.validate()?;
        require(
            self.request_id == request.request_id,
            "plan request_id does not match request",
        )?;
        require(
            self.effective_output.count == request.requested_output.count,
            "plan cannot silently change output count",
        )?;
        if self.effective_output != request.requested_output {
            require(
                request.policy.allow_resolution_change,
                "resolution change is not allowed",
            )?;
            self.require_adjustment(
                "output",
                serde_json::to_value(&request.requested_output).expect("output is serializable"),
                serde_json::to_value(&self.effective_output).expect("output is serializable"),
            )?;
        }
        if self.model_variant_id != request.model_variant_id {
            require(
                request.policy.allow_model_substitution,
                "model substitution is not allowed",
            )?;
            self.require_adjustment(
                "model_variant_id",
                Value::String(request.model_variant_id.to_string()),
                Value::String(self.model_variant_id.to_string()),
            )?;
        }
        Ok(())
    }

    fn require_adjustment(
        &self,
        field: &str,
        requested: Value,
        effective: Value,
    ) -> Result<(), ContractError> {
        require(
            self.adjustments.iter().any(|change| {
                change.field == field
                    && change.requested == requested
                    && change.effective == effective
            }),
            format!("plan must record the actual {field} adjustment"),
        )
    }
}

document_io!(ExecutionPlan, PLAN_SCHEMA_VERSION, "execution_plan");

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    /// This data tests contracts and is never product/model execution evidence.
    ContractFixture,
    ExecutionRecord,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum FinalResult {
    Succeeded,
    Failed { code: String },
    Cancelled,
    Interrupted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunManifest {
    pub schema_version: u32,
    pub evidence_kind: EvidenceKind,
    pub job_id: JobId,
    pub attempt: Attempt,
    pub request: UserRequest,
    pub plan: ExecutionPlan,
    pub input_assets: Vec<AssetRef>,
    pub outputs: Vec<AssetRef>,
    pub final_result: FinalResult,
}

impl RunManifest {
    pub fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version, MANIFEST_SCHEMA_VERSION, "run_manifest")?;
        self.plan.validate_against(&self.request)?;
        require(
            self.attempt.job_id == self.job_id,
            "manifest attempt belongs to a different job",
        )?;
        let required_inputs: BTreeMap<_, _> = self
            .request
            .inputs
            .iter()
            .map(|binding| (&binding.asset.asset_id, &binding.asset.content_hash))
            .collect();
        let mut seen_inputs = BTreeSet::new();
        for input in &self.input_assets {
            input.validate()?;
            require(
                seen_inputs.insert(&input.asset_id),
                "manifest repeats an input asset",
            )?;
            require(
                required_inputs.get(&input.asset_id) == Some(&&input.content_hash),
                "manifest input identity or hash does not match request",
            )?;
        }
        require(
            required_inputs.len() == seen_inputs.len(),
            "manifest is missing an input asset snapshot",
        )?;
        let mut output_ids = BTreeSet::new();
        for output in &self.outputs {
            output.validate()?;
            if let Some(input_hash) = required_inputs.get(&output.asset_id) {
                require(
                    *input_hash == &output.content_hash,
                    "output cannot change bytes under an existing input asset identity",
                )?;
            }
            require(
                output_ids.insert(&output.asset_id),
                "manifest repeats an output asset",
            )?;
        }
        if self.final_result == FinalResult::Succeeded {
            require(
                !self.outputs.is_empty(),
                "successful manifest requires output references and hashes",
            )?;
        }
        if let FinalResult::Failed { code } = &self.final_result {
            nonempty(code, "failure code")?;
        }
        Ok(())
    }
}

document_io!(RunManifest, MANIFEST_SCHEMA_VERSION, "run_manifest");
