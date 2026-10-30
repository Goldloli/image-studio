//! Generation v1 is a separate wire family; diagnostic envelopes cannot decode as generation.
//! A candidate proves neither process cleanup nor durable success. Those are host responsibilities.

pub use crate::VersionRange;
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, Write},
    path::Path,
};
use thiserror::Error;

pub const FAMILY: &str = "image-studio.generation";
pub const PROTOCOL_VERSION: u16 = 1;
pub const VERSIONS: VersionRange = VersionRange {
    min: PROTOCOL_VERSION,
    max: PROTOCOL_VERSION,
};
pub const CAPABILITY: &str = "image.text-to-image.z-image-turbo.v1";
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
pub const MAX_PROMPT_BYTES: usize = 16 * 1024;
pub const MAX_OUTPUT_BYTES: u64 = 32 * 1024 * 1024;

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
    Progress {
        stage: ProgressStage,
        completed: Option<u32>,
        total: Option<u32>,
    },
    Cancel,
    OutputCandidate {
        candidate: OutputCandidate,
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
                capability(capabilities)?;
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
                capability(capabilities)?;
            }
            message => {
                if self.execution.is_none() || self.version != Some(PROTOCOL_VERSION) {
                    return Err(Error::WrongExecution);
                }
                match message {
                    Message::Execute { request } => request.validate()?,
                    Message::Progress {
                        stage,
                        completed,
                        total,
                    } => {
                        if *stage == ProgressStage::Sampling {
                            if !matches!((completed,total),(Some(n),Some(9)) if *n <= 9) {
                                return Err(Error::Invalid("invalid sampling progress"));
                            }
                        } else if completed.is_some() || total.is_some() {
                            return Err(Error::Invalid("indeterminate stage cannot claim steps"));
                        }
                    }
                    Message::OutputCandidate { candidate } => candidate.validate()?,
                    Message::Error { code, summary } => {
                        identifier(code)?;
                        if summary.is_empty() || summary.len() > 4096 || summary.contains('\0') {
                            return Err(Error::Invalid("invalid error summary"));
                        }
                    }
                    _ => {}
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

/// Per-attempt identity/ordering gate. Persistence and cancellation/success arbitration stay in Rust core.
#[derive(Clone, Debug)]
pub struct HostSession {
    session_id: String,
    expected: WorkerIdentity,
    hello_request: Option<String>,
    handshaken: bool,
    send_sequence: u64,
    last_received: Option<u64>,
    active: Option<(String, ExecutionContext, ExecuteRequest)>,
    cancelled: bool,
    candidate_seen: bool,
    terminal: bool,
}

impl HostSession {
    pub fn new(session_id: impl Into<String>, expected: WorkerIdentity) -> Result<Self, Error> {
        let session_id = session_id.into();
        identifier(&session_id)?;
        expected.validate()?;
        Ok(Self {
            session_id,
            expected,
            hello_request: None,
            handshaken: false,
            send_sequence: 0,
            last_received: None,
            active: None,
            cancelled: false,
            candidate_seen: false,
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
            versions: VERSIONS,
            build_id: build_id.into(),
            expected_worker: self.expected.clone(),
            capabilities: vec![CAPABILITY.into()],
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
        self.active = Some((request_id, context, request));
        self.send_sequence += 1;
        Ok(envelope)
    }
    pub fn cancel(&mut self) -> Result<Envelope, Error> {
        if self.cancelled {
            return Err(Error::UnexpectedMessage);
        }
        let (request_id, context, _) = self.active.as_ref().ok_or(Error::UnexpectedMessage)?;
        let envelope = self.make(request_id.clone(), Some(context.clone()), Message::Cancel);
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
                    if VERSIONS.highest_common(*versions) != Some(*selected_version) {
                        return Err(Error::VersionMismatch);
                    }
                    if identity != &self.expected {
                        return Err(Error::IdentityMismatch);
                    }
                    capability(capabilities)?;
                    WorkerEvent::HandshakeAccepted
                }
                _ => return Err(Error::UnexpectedMessage),
            }
        } else {
            if self.terminal {
                return Err(Error::UnexpectedMessage);
            }
            let (request_id, context, request) =
                self.active.as_ref().ok_or(Error::UnexpectedMessage)?;
            if envelope.request_id != *request_id {
                return Err(Error::WrongRequest);
            }
            if envelope.execution.as_ref() != Some(context) {
                return Err(Error::WrongExecution);
            }
            match &envelope.message {
                Message::Progress { .. } if !self.candidate_seen => WorkerEvent::Progress,
                Message::OutputCandidate { candidate } if !self.candidate_seen => {
                    if (candidate.width, candidate.height) != (request.width, request.height) {
                        return Err(Error::Invalid("candidate dimensions differ from plan"));
                    }
                    WorkerEvent::OutputCandidate
                }
                Message::Cancelled if self.cancelled => WorkerEvent::Cancelled,
                Message::Error { code, .. } if code == "CLEANUP_INCOMPLETE" => {
                    WorkerEvent::CleanupIncomplete
                }
                Message::Error { .. } => WorkerEvent::Error,
                _ => return Err(Error::UnexpectedMessage),
            }
        };
        self.last_received = Some(envelope.sequence);
        match event {
            WorkerEvent::HandshakeAccepted => self.handshaken = true,
            WorkerEvent::OutputCandidate => self.candidate_seen = true,
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
            version: self.handshaken.then_some(PROTOCOL_VERSION),
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
fn capability(values: &[String]) -> Result<(), Error> {
    if values.len() != 1 || values[0] != CAPABILITY {
        return Err(Error::IdentityMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
