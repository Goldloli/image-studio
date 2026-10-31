//! Generation v1 is a separate wire family; diagnostic envelopes cannot decode as generation.
//! A candidate proves neither process cleanup nor durable success. Those are host responsibilities.

pub use crate::VersionRange;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufRead, Write},
    path::Path,
};
use thiserror::Error;

pub const FAMILY: &str = "image-studio.generation";
pub const PROTOCOL_VERSION: u16 = 1;
pub const PROTOCOL_VERSION_V2: u16 = 2;
pub const VERSIONS: VersionRange = VersionRange {
    min: PROTOCOL_VERSION,
    max: PROTOCOL_VERSION,
};
pub const VERSIONS_V2: VersionRange = VersionRange {
    min: PROTOCOL_VERSION_V2,
    max: PROTOCOL_VERSION_V2,
};
pub const CAPABILITY: &str = "image.text-to-image.z-image-turbo.v1";
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
pub const MAX_PROMPT_BYTES: usize = 16 * 1024;
pub const MAX_OUTPUT_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_UPSCALE_OUTPUT_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerIdentity {
    pub build_id: String,
    pub runtime_lock_id: String,
    pub model_snapshot_id: String,
}

impl WorkerIdentity {
    pub fn validate(&self) -> Result<(), Error> {
        for value in [
            &self.build_id,
            &self.runtime_lock_id,
            &self.model_snapshot_id,
        ] {
            identifier(value)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionContext {
    pub job_id: String,
    pub attempt_id: String,
    pub execution_token: String,
}

impl ExecutionContext {
    pub fn validate(&self) -> Result<(), Error> {
        for value in [&self.job_id, &self.attempt_id, &self.execution_token] {
            identifier(value)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecuteRequest {
    pub prompt: String,
    pub width: u32,
    pub height: u32,
    pub seed: u32,
    pub num_inference_steps: u32,
    pub quantization_bits: u8,
    /// Trusted host-resolved paths. Never accept these from the webview.
    pub model_path: String,
    pub output_dir: String,
}

impl ExecuteRequest {
    pub fn validate(&self) -> Result<(), Error> {
        if self.prompt.trim().is_empty()
            || self.prompt.len() > MAX_PROMPT_BYTES
            || self.prompt.contains('\0')
        {
            return Err(Error::Invalid("invalid prompt byte budget"));
        }
        if !matches!(
            (self.width, self.height),
            (1024, 1024) | (1024, 768) | (768, 1024)
        ) || self.num_inference_steps != 9
            || self.quantization_bits != 4
        {
            return Err(Error::Invalid("unsupported generation recipe"));
        }
        for path in [&self.model_path, &self.output_dir] {
            if path.len() > 4096 || path.contains('\0') || !Path::new(path).is_absolute() {
                return Err(Error::Invalid(
                    "generation paths must be absolute local paths",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputAssetDescriptor {
    pub role: String,
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub sha256: String,
}

impl InputAssetDescriptor {
    fn validate(&self, output_dir: &str) -> Result<(), Error> {
        if self.role != "source"
            || self.width == 0
            || self.height == 0
            || u64::from(self.width) * u64::from(self.height) > 4 * 1024 * 1024
            || !valid_sha256(&self.sha256)
            || !safe_absolute_child_path(&self.path, output_dir)
        {
            return Err(Error::Invalid("invalid source asset descriptor"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputSpec {
    pub role: String,
    pub data_type: String,
    pub width: u32,
    pub height: u32,
    pub max_bytes: u64,
}

impl OutputSpec {
    fn validate(&self) -> Result<(), Error> {
        if !matches!(self.data_type.as_str(), "image_rgb" | "image_rgba" | "mask")
            || !matches!(self.role.as_str(), "primary" | "mask")
            || self.width == 0
            || self.height == 0
            || self.max_bytes == 0
            || self.max_bytes > MAX_UPSCALE_OUTPUT_BYTES
        {
            return Err(Error::Invalid("invalid output specification"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecuteRequestV2 {
    pub model_id: String,
    pub capability_id: String,
    pub recipe_id: String,
    pub recipe_version: u16,
    pub runtime_lock_id: String,
    pub model_snapshot_id: String,
    pub instruction: String,
    pub seed: Option<u32>,
    pub sampling_steps: Option<u32>,
    pub parameters: BTreeMap<String, serde_json::Value>,
    pub inputs: Vec<InputAssetDescriptor>,
    pub outputs: Vec<OutputSpec>,
    /// Trusted host-resolved paths. Never accept these from the webview.
    pub model_path: String,
    pub output_dir: String,
}

impl ExecuteRequestV2 {
    pub fn validate(&self) -> Result<(), Error> {
        for value in [
            &self.model_id,
            &self.capability_id,
            &self.recipe_id,
            &self.runtime_lock_id,
            &self.model_snapshot_id,
        ] {
            identifier(value)?;
        }
        if self.recipe_version != 1 {
            return Err(Error::Invalid("unsupported recipe version"));
        }
        for path in [&self.model_path, &self.output_dir] {
            if path.len() > 4096 || path.contains('\0') || !Path::new(path).is_absolute() {
                return Err(Error::Invalid(
                    "generation paths must be absolute local paths",
                ));
            }
        }
        if self.instruction.len() > MAX_PROMPT_BYTES || self.instruction.contains('\0') {
            return Err(Error::Invalid("invalid instruction byte budget"));
        }
        if self.parameters.len() > 32 {
            return Err(Error::Invalid("too many recipe parameters"));
        }
        if self.inputs.len() > 1 || self.outputs.is_empty() || self.outputs.len() > 2 {
            return Err(Error::Invalid("invalid v2 input or output cardinality"));
        }
        for input in &self.inputs {
            input.validate(&self.output_dir)?;
        }
        let mut output_roles = BTreeSet::new();
        for output in &self.outputs {
            output.validate()?;
            if !output_roles.insert(output.role.as_str()) {
                return Err(Error::Invalid("duplicate v2 output role"));
            }
        }
        match (self.model_id.as_str(), self.capability_id.as_str()) {
            ("flux2-klein-4b-distilled", "image.text-to-image.flux2-klein-4b.v1")
                if self.recipe_id == "klein4b-text-v1" =>
            {
                if self.instruction.trim().is_empty()
                    || self.seed.is_none()
                    || self.sampling_steps != Some(4)
                    || !self.inputs.is_empty()
                {
                    return Err(Error::Invalid("invalid Klein text recipe"));
                }
                expect_outputs(&self.outputs, &output_roles, &["primary"], "image_rgb")?;
                if self
                    .outputs
                    .iter()
                    .any(|output| (output.width, output.height) != (1024, 1024))
                {
                    return Err(Error::Invalid("Klein output must be 1024 square"));
                }
            }
            ("flux2-klein-4b-distilled", "image.edit.flux2-klein-4b.v1")
                if self.recipe_id == "klein4b-edit-v1" =>
            {
                if self.instruction.trim().is_empty()
                    || self.seed.is_none()
                    || self.sampling_steps != Some(4)
                    || self.inputs.len() != 1
                {
                    return Err(Error::Invalid("invalid Klein edit recipe"));
                }
                expect_outputs(&self.outputs, &output_roles, &["primary"], "image_rgb")?;
                if self
                    .outputs
                    .iter()
                    .any(|output| (output.width, output.height) != (1024, 1024))
                {
                    return Err(Error::Invalid("Klein output must be 1024 square"));
                }
            }
            (
                "birefnet-general-epoch-244",
                "image.remove-background.birefnet-general-epoch-244.v1",
            ) if self.recipe_id == "birefnet-general-cpu-v1" => {
                if !self.instruction.is_empty()
                    || self.seed.is_some()
                    || self.sampling_steps.is_some()
                    || self.inputs.len() != 1
                {
                    return Err(Error::Invalid("invalid BiRefNet recipe"));
                }
                expect_two_outputs(&self.outputs, &output_roles, "image_rgba", "mask")?;
                let source = &self.inputs[0];
                if self
                    .outputs
                    .iter()
                    .any(|output| (output.width, output.height) != (source.width, source.height))
                {
                    return Err(Error::Invalid("BiRefNet output must preserve source size"));
                }
            }
            ("realesrgan-x4plus", "image.upscale.realesrgan-x4plus.v1")
                if self.recipe_id == "realesrgan-x4plus-t128-v1" =>
            {
                if !self.instruction.is_empty()
                    || self.seed.is_some()
                    || self.sampling_steps.is_some()
                    || self.inputs.len() != 1
                {
                    return Err(Error::Invalid("invalid Real-ESRGAN recipe"));
                }
                expect_outputs(&self.outputs, &output_roles, &["primary"], "image_rgb")?;
                let source = &self.inputs[0];
                let width = source.width.checked_mul(4);
                let height = source.height.checked_mul(4);
                if width.is_none()
                    || height.is_none()
                    || self.outputs.iter().any(|output| {
                        Some((output.width, output.height)) != width.zip(height)
                            || output.max_bytes > MAX_UPSCALE_OUTPUT_BYTES
                    })
                {
                    return Err(Error::Invalid("invalid upscale output size or limit"));
                }
            }
            _ => return Err(Error::Invalid("unsupported v2 model recipe")),
        }
        Ok(())
    }

    fn output_spec(&self, role: &str) -> Option<&OutputSpec> {
        self.outputs.iter().find(|output| output.role == role)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputCandidateV2 {
    pub role: String,
    pub data_type: String,
    pub relative_path: String,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
    pub sha256: String,
}

impl OutputCandidateV2 {
    fn validate(&self) -> Result<(), Error> {
        if !matches!(self.role.as_str(), "primary" | "mask")
            || !matches!(self.data_type.as_str(), "image_rgb" | "image_rgba" | "mask")
            || safe_relative_output_path(&self.relative_path).is_err()
            || self.width == 0
            || self.height == 0
            || self.size_bytes == 0
            || self.size_bytes > MAX_UPSCALE_OUTPUT_BYTES
            || !valid_sha256(&self.sha256)
        {
            return Err(Error::Invalid("invalid v2 output candidate"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressStage {
    Loading,
    Encoding,
    Sampling,
    Decoding,
    Writing,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputCandidate {
    pub relative_path: String,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
    pub sha256: String,
}

impl OutputCandidate {
    pub fn validate(&self) -> Result<(), Error> {
        if self.relative_path != "result.png"
            || self.size_bytes == 0
            || self.size_bytes > MAX_OUTPUT_BYTES
            || self.sha256.len() != 64
            || !self.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(Error::Invalid("invalid output candidate descriptor"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Message {
    Hello {
        versions: VersionRange,
        build_id: String,
        expected_worker: WorkerIdentity,
        capabilities: Vec<String>,
    },
    HelloAccepted {
        versions: VersionRange,
        selected_version: u16,
        identity: WorkerIdentity,
        capabilities: Vec<String>,
    },
    Execute {
        request: ExecuteRequest,
    },
    ExecuteV2 {
        request: ExecuteRequestV2,
    },
    Progress {
        stage: ProgressStage,
        completed: Option<u32>,
        total: Option<u32>,
    },
    Cancel,
    OutputCandidate {
        candidate: OutputCandidate,
    },
    OutputCandidateV2 {
        candidate: OutputCandidateV2,
    },
    Cancelled,
    Error {
        code: String,
        summary: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub family: String,
    pub version: Option<u16>,
    pub session_id: String,
    pub request_id: String,
    pub sequence: u64,
    pub execution: Option<ExecutionContext>,
    pub message: Message,
}

impl Envelope {
    pub fn decode_frame(frame: &[u8]) -> Result<Self, Error> {
        let frame = frame.strip_suffix(b"\n").unwrap_or(frame);
        let frame = frame.strip_suffix(b"\r").unwrap_or(frame);
        if frame.len() > MAX_FRAME_BYTES {
            return Err(Error::FrameTooLarge);
        }
        let envelope: Self = serde_json::from_slice(frame)?;
        envelope.validate()?;
        Ok(envelope)
    }

    pub fn encode_line(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let mut bytes = serde_json::to_vec(self)?;
        if bytes.len() > MAX_FRAME_BYTES {
            return Err(Error::FrameTooLarge);
        }
        bytes.push(b'\n');
        Ok(bytes)
    }

    pub fn validate(&self) -> Result<(), Error> {
        if self.family != FAMILY {
            return Err(Error::WrongFamily);
        }
        identifier(&self.session_id)?;
        identifier(&self.request_id)?;
        if let Some(context) = &self.execution {
            context.validate()?;
        }
        match &self.message {
            Message::Hello {
                versions,
                build_id,
                expected_worker,
                capabilities,
            } => {
                if self.version.is_some() || self.execution.is_some() || self.sequence != 0 {
                    return Err(Error::UnexpectedMessage);
                }
                versions.validate().map_err(|_| Error::VersionMismatch)?;
                identifier(build_id)?;
                expected_worker.validate()?;
                validate_capabilities(capabilities)?;
            }
            Message::HelloAccepted {
                versions,
                selected_version,
                identity,
                capabilities,
            } => {
                versions.validate().map_err(|_| Error::VersionMismatch)?;
                if self.execution.is_some()
                    || self.sequence != 0
                    || self.version != Some(*selected_version)
                {
                    return Err(Error::UnexpectedMessage);
                }
                identity.validate()?;
                validate_capabilities(capabilities)?;
            }
            message => {
                if self.execution.is_none()
                    || !matches!(self.version, Some(PROTOCOL_VERSION | PROTOCOL_VERSION_V2))
                {
                    return Err(Error::WrongExecution);
                }
                match message {
                    Message::Execute { request } if self.version == Some(PROTOCOL_VERSION) => {
                        request.validate()?;
                    }
                    Message::ExecuteV2 { request } if self.version == Some(PROTOCOL_VERSION_V2) => {
                        request.validate()?;
                    }
                    Message::Progress {
                        stage,
                        completed,
                        total,
                    } => {
                        if *stage == ProgressStage::Sampling
                            && self.version == Some(PROTOCOL_VERSION)
                        {
                            if !matches!((completed,total),(Some(n),Some(9)) if *n <= 9) {
                                return Err(Error::Invalid("invalid sampling progress"));
                            }
                        } else if *stage == ProgressStage::Sampling {
                            if !matches!((completed,total),(Some(n),Some(total)) if *total > 0 && *n <= *total)
                            {
                                return Err(Error::Invalid("invalid v2 sampling progress"));
                            }
                        } else if completed.is_some() || total.is_some() {
                            return Err(Error::Invalid("indeterminate stage cannot claim steps"));
                        }
                    }
                    Message::OutputCandidate { candidate }
                        if self.version == Some(PROTOCOL_VERSION) =>
                    {
                        candidate.validate()?;
                    }
                    Message::OutputCandidateV2 { candidate }
                        if self.version == Some(PROTOCOL_VERSION_V2) =>
                    {
                        candidate.validate()?;
                    }
                    Message::Error { code, summary } => {
                        identifier(code)?;
                        if summary.is_empty() || summary.len() > 4096 || summary.contains('\0') {
                            return Err(Error::Invalid("invalid error summary"));
                        }
                    }
                    Message::Cancel | Message::Cancelled => {}
                    _ => return Err(Error::Invalid("message does not match protocol version")),
                }
            }
        }
        Ok(())
    }
}

pub fn read_frame(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>, Error> {
    crate::read_frame(reader).map_err(|error| match error {
        crate::ProtocolError::FrameTooLarge { .. } => Error::FrameTooLarge,
        crate::ProtocolError::TruncatedFrame => Error::TruncatedFrame,
        crate::ProtocolError::Io(error) => Error::Io(error),
        _ => Error::Invalid("invalid frame"),
    })
}

pub fn write_message(writer: &mut impl Write, envelope: &Envelope) -> Result<(), Error> {
    writer.write_all(&envelope.encode_line()?)?;
    writer.flush()?;
    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("generation control frame exceeds 64 KiB")]
    FrameTooLarge,
    #[error("control stream ended inside a frame")]
    TruncatedFrame,
    #[error("control stream IO failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid generation JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("wrong protocol family")]
    WrongFamily,
    #[error("incompatible protocol versions")]
    VersionMismatch,
    #[error("worker identity or capability mismatch")]
    IdentityMismatch,
    #[error("wrong session")]
    WrongSession,
    #[error("wrong request")]
    WrongRequest,
    #[error("wrong execution identity")]
    WrongExecution,
    #[error("duplicate or decreasing sequence")]
    Sequence,
    #[error("unexpected protocol message")]
    UnexpectedMessage,
    #[error("invalid generation message: {0}")]
    Invalid(&'static str),
}

impl Error {
    pub fn is_ignorable_rejection(&self) -> bool {
        matches!(
            self,
            Self::WrongSession | Self::WrongRequest | Self::WrongExecution | Self::Sequence
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerEvent {
    HandshakeAccepted,
    Progress,
    OutputCandidate,
    Cancelled,
    Error,
    CleanupIncomplete,
}

#[derive(Clone, Debug)]
enum ActiveRequest {
    V1 {
        request_id: String,
        context: ExecutionContext,
        request: ExecuteRequest,
    },
    V2 {
        request_id: String,
        context: ExecutionContext,
        request: ExecuteRequestV2,
    },
}

impl ActiveRequest {
    fn request_id(&self) -> &str {
        match self {
            Self::V1 { request_id, .. } | Self::V2 { request_id, .. } => request_id,
        }
    }

    fn context(&self) -> &ExecutionContext {
        match self {
            Self::V1 { context, .. } | Self::V2 { context, .. } => context,
        }
    }
}

/// Per-attempt identity/ordering gate. Persistence and cancellation/success arbitration stay in Rust core.
#[derive(Clone, Debug)]
pub struct HostSession {
    session_id: String,
    expected: WorkerIdentity,
    supported_versions: VersionRange,
    protocol_version: u16,
    expected_capabilities: Vec<String>,
    maximum_candidates: usize,
    hello_request: Option<String>,
    handshaken: bool,
    send_sequence: u64,
    last_received: Option<u64>,
    active: Option<ActiveRequest>,
    cancelled: bool,
    candidate_roles: BTreeSet<String>,
    terminal: bool,
}

impl HostSession {
    pub fn new(session_id: impl Into<String>, expected: WorkerIdentity) -> Result<Self, Error> {
        Self::new_for(
            session_id,
            expected,
            VERSIONS,
            PROTOCOL_VERSION,
            vec![CAPABILITY.into()],
            1,
        )
    }

    pub fn new_v2(
        session_id: impl Into<String>,
        expected: WorkerIdentity,
        capabilities: Vec<String>,
    ) -> Result<Self, Error> {
        Self::new_for(
            session_id,
            expected,
            VERSIONS_V2,
            PROTOCOL_VERSION_V2,
            capabilities,
            2,
        )
    }

    fn new_for(
        session_id: impl Into<String>,
        expected: WorkerIdentity,
        supported_versions: VersionRange,
        protocol_version: u16,
        expected_capabilities: Vec<String>,
        maximum_candidates: usize,
    ) -> Result<Self, Error> {
        let session_id = session_id.into();
        identifier(&session_id)?;
        expected.validate()?;
        supported_versions
            .validate()
            .map_err(|_| Error::VersionMismatch)?;
        if expected_capabilities.is_empty() {
            return Err(Error::IdentityMismatch);
        }
        let mut unique_capabilities = BTreeSet::new();
        for capability in &expected_capabilities {
            identifier(capability)?;
            if !unique_capabilities.insert(capability) {
                return Err(Error::IdentityMismatch);
            }
        }
        if supported_versions.highest_common(VersionRange {
            min: protocol_version,
            max: protocol_version,
        }) != Some(protocol_version)
            || maximum_candidates == 0
            || maximum_candidates > 2
        {
            return Err(Error::VersionMismatch);
        }
        Ok(Self {
            session_id,
            expected,
            supported_versions,
            protocol_version,
            expected_capabilities,
            maximum_candidates,
            hello_request: None,
            handshaken: false,
            send_sequence: 0,
            last_received: None,
            active: None,
            cancelled: false,
            candidate_roles: BTreeSet::new(),
            terminal: false,
        })
    }
    pub fn is_handshaken(&self) -> bool {
        self.handshaken
    }
    pub fn cancel_requested(&self) -> bool {
        self.cancelled
    }
    pub fn hello(
        &mut self,
        request_id: impl Into<String>,
        build_id: impl Into<String>,
    ) -> Result<Envelope, Error> {
        if self.hello_request.is_some() {
            return Err(Error::UnexpectedMessage);
        }
        let request_id = request_id.into();
        identifier(&request_id)?;
        let message = Message::Hello {
            versions: self.supported_versions,
            build_id: build_id.into(),
            expected_worker: self.expected.clone(),
            capabilities: self.expected_capabilities.clone(),
        };
        let envelope = self.make(request_id.clone(), None, message);
        envelope.validate()?;
        self.hello_request = Some(request_id);
        self.send_sequence += 1;
        Ok(envelope)
    }
    pub fn execute(
        &mut self,
        request_id: impl Into<String>,
        context: ExecutionContext,
        request: ExecuteRequest,
    ) -> Result<Envelope, Error> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(Error::UnexpectedMessage);
        }
        if !self.handshaken || self.active.is_some() || self.terminal {
            return Err(Error::UnexpectedMessage);
        }
        let request_id = request_id.into();
        identifier(&request_id)?;
        context.validate()?;
        request.validate()?;
        let envelope = self.make(
            request_id.clone(),
            Some(context.clone()),
            Message::Execute {
                request: request.clone(),
            },
        );
        self.active = Some(ActiveRequest::V1 {
            request_id,
            context,
            request,
        });
        self.send_sequence += 1;
        Ok(envelope)
    }

    pub fn execute_v2(
        &mut self,
        request_id: impl Into<String>,
        context: ExecutionContext,
        request: ExecuteRequestV2,
    ) -> Result<Envelope, Error> {
        if self.protocol_version != PROTOCOL_VERSION_V2 {
            return Err(Error::UnexpectedMessage);
        }
        if !self.handshaken || self.active.is_some() || self.terminal {
            return Err(Error::UnexpectedMessage);
        }
        let request_id = request_id.into();
        identifier(&request_id)?;
        context.validate()?;
        request.validate()?;
        if request.runtime_lock_id != self.expected.runtime_lock_id
            || request.model_snapshot_id != self.expected.model_snapshot_id
        {
            return Err(Error::IdentityMismatch);
        }
        let envelope = self.make(
            request_id.clone(),
            Some(context.clone()),
            Message::ExecuteV2 {
                request: request.clone(),
            },
        );
        self.active = Some(ActiveRequest::V2 {
            request_id,
            context,
            request,
        });
        self.send_sequence += 1;
        Ok(envelope)
    }
    pub fn cancel(&mut self) -> Result<Envelope, Error> {
        if self.cancelled {
            return Err(Error::UnexpectedMessage);
        }
        let active = self.active.as_ref().ok_or(Error::UnexpectedMessage)?;
        let envelope = self.make(
            active.request_id().into(),
            Some(active.context().clone()),
            Message::Cancel,
        );
        self.cancelled = true;
        self.send_sequence += 1;
        Ok(envelope)
    }
    /// The native proxy uses the same gate to validate host traffic. Rejections are transactional.
    pub fn observe_outbound(&mut self, envelope: &Envelope) -> Result<(), Error> {
        envelope.validate()?;
        if envelope.session_id != self.session_id {
            return Err(Error::WrongSession);
        }
        if envelope.sequence != self.send_sequence {
            return Err(Error::Sequence);
        }
        let mut next = self.clone();
        let expected = match &envelope.message {
            Message::Hello { build_id, .. } => {
                next.hello(envelope.request_id.clone(), build_id.clone())?
            }
            Message::Execute { request } => next.execute(
                envelope.request_id.clone(),
                envelope.execution.clone().ok_or(Error::WrongExecution)?,
                request.clone(),
            )?,
            Message::ExecuteV2 { request } => next.execute_v2(
                envelope.request_id.clone(),
                envelope.execution.clone().ok_or(Error::WrongExecution)?,
                request.clone(),
            )?,
            Message::Cancel => next.cancel()?,
            _ => return Err(Error::UnexpectedMessage),
        };
        if expected != *envelope {
            return Err(Error::Invalid(
                "outbound fields differ from negotiated session",
            ));
        }
        *self = next;
        Ok(())
    }
    /// Rejected late events never advance sequence, change identity, or create a terminal state.
    pub fn accept(&mut self, envelope: &Envelope) -> Result<WorkerEvent, Error> {
        envelope.validate()?;
        if envelope.session_id != self.session_id {
            return Err(Error::WrongSession);
        }
        if self.last_received.is_none() && envelope.sequence != 0
            || self
                .last_received
                .is_some_and(|last| envelope.sequence <= last)
        {
            return Err(Error::Sequence);
        }
        let event = if !self.handshaken {
            if self.hello_request.as_deref() != Some(envelope.request_id.as_str()) {
                return Err(Error::WrongRequest);
            }
            match &envelope.message {
                Message::HelloAccepted {
                    versions,
                    selected_version,
                    identity,
                    capabilities,
                } => {
                    if self.supported_versions.highest_common(*versions) != Some(*selected_version)
                        || *selected_version != self.protocol_version
                    {
                        return Err(Error::VersionMismatch);
                    }
                    if identity != &self.expected {
                        return Err(Error::IdentityMismatch);
                    }
                    capabilities_match(capabilities, &self.expected_capabilities)?;
                    WorkerEvent::HandshakeAccepted
                }
                _ => return Err(Error::UnexpectedMessage),
            }
        } else {
            if self.terminal {
                return Err(Error::UnexpectedMessage);
            }
            let active = self.active.as_ref().ok_or(Error::UnexpectedMessage)?;
            if envelope.request_id != active.request_id() {
                return Err(Error::WrongRequest);
            }
            if envelope.execution.as_ref() != Some(active.context()) {
                return Err(Error::WrongExecution);
            }
            match (active, &envelope.message) {
                (ActiveRequest::V1 { request, .. }, Message::Progress { .. })
                    if self.candidate_roles.is_empty() =>
                {
                    WorkerEvent::Progress
                }
                (ActiveRequest::V2 { request, .. }, Message::Progress { stage, total, .. })
                    if self.candidate_roles.is_empty() =>
                {
                    if *stage == ProgressStage::Sampling && request.sampling_steps != *total {
                        return Err(Error::Invalid(
                            "v2 progress total differs from frozen recipe",
                        ));
                    }
                    WorkerEvent::Progress
                }
                (ActiveRequest::V1 { request, .. }, Message::OutputCandidate { candidate })
                    if self.candidate_roles.is_empty() =>
                {
                    if (candidate.width, candidate.height) != (request.width, request.height) {
                        return Err(Error::Invalid("candidate dimensions differ from plan"));
                    }
                    self.candidate_roles.insert(candidate.relative_path.clone());
                    WorkerEvent::OutputCandidate
                }
                (ActiveRequest::V2 { request, .. }, Message::OutputCandidateV2 { candidate })
                    if self.candidate_roles.len() < self.maximum_candidates =>
                {
                    if self.candidate_roles.contains(&candidate.role) {
                        return Err(Error::Invalid("duplicate v2 output role"));
                    }
                    let output = request
                        .output_spec(&candidate.role)
                        .ok_or(Error::Invalid("unexpected v2 output role"))?;
                    if candidate.data_type != output.data_type
                        || candidate.width != output.width
                        || candidate.height != output.height
                        || candidate.size_bytes > output.max_bytes
                    {
                        return Err(Error::Invalid(
                            "v2 candidate differs from frozen output specification",
                        ));
                    }
                    self.candidate_roles.insert(candidate.role.clone());
                    WorkerEvent::OutputCandidate
                }
                (_, Message::Cancelled) if self.cancelled => WorkerEvent::Cancelled,
                (_, Message::Error { code, .. }) if code == "CLEANUP_INCOMPLETE" => {
                    WorkerEvent::CleanupIncomplete
                }
                (_, Message::Error { .. }) => WorkerEvent::Error,
                _ => return Err(Error::UnexpectedMessage),
            }
        };
        self.last_received = Some(envelope.sequence);
        match event {
            WorkerEvent::HandshakeAccepted => self.handshaken = true,
            WorkerEvent::OutputCandidate => {}
            WorkerEvent::Cancelled | WorkerEvent::Error => self.terminal = true,
            _ => {}
        }
        Ok(event)
    }
    fn make(
        &self,
        request_id: String,
        execution: Option<ExecutionContext>,
        message: Message,
    ) -> Envelope {
        Envelope {
            family: FAMILY.into(),
            version: self.handshaken.then_some(self.protocol_version),
            session_id: self.session_id.clone(),
            request_id,
            sequence: self.send_sequence,
            execution,
            message,
        }
    }
}

fn identifier(value: &str) -> Result<(), Error> {
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(Error::Invalid("invalid identifier"));
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn safe_absolute_child_path(value: &str, root: &str) -> bool {
    let path = Path::new(value);
    let root = Path::new(root);
    path.is_absolute()
        && value.len() <= 4096
        && !value.contains('\0')
        && path != root
        && path.starts_with(root)
        && path.strip_prefix(root).is_ok_and(|relative| {
            relative
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)))
        })
}

fn safe_relative_output_path(value: &str) -> Result<(), Error> {
    if value.is_empty()
        || value.len() > 256
        || value.contains(['\\', ':', '\0'])
        || !Path::new(value)
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
    {
        return Err(Error::Invalid("unsafe relative output path"));
    }
    Ok(())
}

fn expect_outputs(
    outputs: &[OutputSpec],
    roles: &BTreeSet<&str>,
    expected_roles: &[&str],
    primary_type: &str,
) -> Result<(), Error> {
    if roles.len() != expected_roles.len()
        || !expected_roles.iter().all(|role| roles.contains(role))
        || outputs.len() != 1
        || outputs[0].data_type != primary_type
    {
        return Err(Error::Invalid("recipe output roles do not match"));
    }
    Ok(())
}

fn expect_two_outputs(
    outputs: &[OutputSpec],
    roles: &BTreeSet<&str>,
    primary_type: &str,
    mask_type: &str,
) -> Result<(), Error> {
    if roles.len() != 2
        || !roles.contains("primary")
        || !roles.contains("mask")
        || outputs.len() != 2
        || outputs
            .iter()
            .find(|output| output.role == "primary")
            .is_none_or(|output| output.data_type != primary_type)
        || outputs
            .iter()
            .find(|output| output.role == "mask")
            .is_none_or(|output| output.data_type != mask_type)
    {
        return Err(Error::Invalid("BiRefNet output roles do not match"));
    }
    Ok(())
}

fn capabilities_match(values: &[String], expected: &[String]) -> Result<(), Error> {
    if values != expected {
        return Err(Error::IdentityMismatch);
    }
    Ok(())
}

fn validate_capabilities(values: &[String]) -> Result<(), Error> {
    if values.is_empty() || values.len() > 2 {
        return Err(Error::IdentityMismatch);
    }
    let mut unique = BTreeSet::new();
    for value in values {
        identifier(value)?;
        if !unique.insert(value) {
            return Err(Error::IdentityMismatch);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
