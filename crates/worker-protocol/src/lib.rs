//! Bounded, versioned diagnostic communication. No message in this protocol performs inference.

/// Independent image generation protocol. The diagnostic v1 types above remain unchanged.
pub mod generation;

use std::io::{BufRead, Write};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
pub const DIAGNOSTIC_CAPABILITY: &str = "diagnostic.probe.v1";
pub const DIAGNOSTIC_SCOPE: &str = "diagnostic_communication_only";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionRange {
    pub min: u16,
    pub max: u16,
}

impl VersionRange {
    pub const CURRENT: Self = Self {
        min: PROTOCOL_VERSION,
        max: PROTOCOL_VERSION,
    };

    pub fn new(min: u16, max: u16) -> Result<Self, ProtocolError> {
        let value = Self { min, max };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(self) -> Result<(), ProtocolError> {
        if self.min == 0 || self.max < self.min {
            return Err(ProtocolError::InvalidEnvelope("invalid version range"));
        }
        Ok(())
    }

    pub fn highest_common(self, other: Self) -> Option<u16> {
        if self.validate().is_err() || other.validate().is_err() {
            return None;
        }
        let highest = self.max.min(other.max);
        (highest >= self.min.max(other.min)).then_some(highest)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionContext {
    pub attempt_id: String,
    pub fencing_token: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Message {
    Hello {
        versions: VersionRange,
        build_id: String,
        capabilities: Vec<String>,
    },
    HelloAccepted {
        versions: VersionRange,
        selected_version: u16,
        build_id: String,
        capabilities: Vec<String>,
    },
    Probe {
        challenge: String,
    },
    ProbeResult {
        challenge: String,
        build_id: String,
        scope: String,
        child_pid: Option<u32>,
    },
    Progress {
        completed: u64,
        total: u64,
    },
    Heartbeat,
    ProtocolError {
        code: String,
        summary: String,
        versions: Option<VersionRange>,
    },
    Shutdown,
    ShutdownAck,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub version: Option<u16>,
    pub session_id: String,
    pub request_id: String,
    pub sequence: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<ExecutionContext>,
    pub message: Message,
}

impl Envelope {
    pub fn new(
        version: Option<u16>,
        session_id: impl Into<String>,
        request_id: impl Into<String>,
        sequence: u64,
        message: Message,
    ) -> Self {
        Self {
            version,
            session_id: session_id.into(),
            request_id: request_id.into(),
            sequence,
            execution: None,
            message,
        }
    }

    pub fn decode_frame(frame: &[u8]) -> Result<Self, ProtocolError> {
        let frame = frame.strip_suffix(b"\n").unwrap_or(frame);
        let frame = frame.strip_suffix(b"\r").unwrap_or(frame);
        if frame.len() > MAX_FRAME_BYTES {
            return Err(ProtocolError::FrameTooLarge {
                limit: MAX_FRAME_BYTES,
            });
        }
        Ok(serde_json::from_slice(frame)?)
    }

    pub fn encode_line(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut bytes = serde_json::to_vec(self)?;
        if bytes.len() > MAX_FRAME_BYTES {
            return Err(ProtocolError::FrameTooLarge {
                limit: MAX_FRAME_BYTES,
            });
        }
        bytes.push(b'\n');
        Ok(bytes)
    }

    pub fn validate_diagnostic_context(&self) -> Result<(), ProtocolError> {
        validate_id(&self.session_id)?;
        validate_id(&self.request_id)?;
        if self.execution.is_some() {
            return Err(ProtocolError::UnexpectedExecution);
        }
        Ok(())
    }
}

/// Read a complete line while checking the accumulated limit before extending the buffer.
/// A partial line at EOF is an interrupted frame, not a valid message.
pub fn read_frame(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>, ProtocolError> {
    let mut bytes = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return if bytes.is_empty() {
                Ok(None)
            } else {
                Err(ProtocolError::TruncatedFrame)
            };
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.unwrap_or(available.len());
        if bytes.len().saturating_add(take) > MAX_FRAME_BYTES {
            return Err(ProtocolError::FrameTooLarge {
                limit: MAX_FRAME_BYTES,
            });
        }
        bytes.extend_from_slice(&available[..take]);
        reader.consume(take + usize::from(newline.is_some()));
        if newline.is_some() {
            return Ok(Some(bytes));
        }
    }
}

pub fn write_message(writer: &mut impl Write, envelope: &Envelope) -> Result<(), ProtocolError> {
    writer.write_all(&envelope.encode_line()?)?;
    writer.flush()?;
    Ok(())
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("control frame exceeds {limit} bytes")]
    FrameTooLarge { limit: usize },
    #[error("control stream ended inside a frame")]
    TruncatedFrame,
    #[error("invalid JSON control message: {0}")]
    MalformedJson(#[from] serde_json::Error),
    #[error("control stream IO failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid envelope: {0}")]
    InvalidEnvelope(&'static str),
    #[error("protocol version is incompatible")]
    VersionMismatch,
    #[error("message is not allowed in the current protocol state")]
    UnexpectedMessage,
    #[error("message belongs to another session")]
    WrongSession,
    #[error("response does not belong to an active request")]
    WrongRequest,
    #[error("non-monotonic sequence: previous {last:?}, received {received}")]
    Sequence { last: Option<u64>, received: u64 },
    #[error("diagnostic message must not contain execution context")]
    UnexpectedExecution,
    #[error("worker must declare only the diagnostic capability")]
    CapabilityMismatch,
    #[error("probe challenge does not match")]
    ChallengeMismatch,
    #[error("probe result is not limited to diagnostic communication")]
    ScopeMismatch,
    #[error("worker error {code}: {summary}")]
    Remote { code: String, summary: String },
}

impl ProtocolError {
    /// Rejected late/cross-request traffic does not alter the current session.
    pub fn is_ignorable_rejection(&self) -> bool {
        matches!(
            self,
            Self::WrongSession
                | Self::WrongRequest
                | Self::Sequence { .. }
                | Self::UnexpectedExecution
        )
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::FrameTooLarge { .. } => "FRAME_TOO_LARGE",
            Self::TruncatedFrame => "TRUNCATED_FRAME",
            Self::MalformedJson(_) => "MALFORMED_JSON",
            Self::Io(_) => "PROTOCOL_IO",
            Self::InvalidEnvelope(_) => "INVALID_ENVELOPE",
            Self::VersionMismatch => "PROTOCOL_VERSION_MISMATCH",
            Self::UnexpectedMessage => "UNEXPECTED_MESSAGE",
            Self::WrongSession => "WRONG_SESSION",
            Self::WrongRequest => "WRONG_REQUEST",
            Self::Sequence { .. } => "INVALID_SEQUENCE",
            Self::UnexpectedExecution => "UNEXPECTED_EXECUTION_CONTEXT",
            Self::CapabilityMismatch => "CAPABILITY_MISMATCH",
            Self::ChallengeMismatch => "CHALLENGE_MISMATCH",
            Self::ScopeMismatch => "SCOPE_MISMATCH",
            Self::Remote { .. } => "WORKER_PROTOCOL_ERROR",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostEvent {
    HandshakeAccepted,
    Progress,
    Heartbeat,
    ProbeCompleted,
    ShutdownAcknowledged,
}

#[derive(Debug)]
pub struct HostSession {
    session_id: String,
    versions: VersionRange,
    selected_version: Option<u16>,
    worker_build_id: Option<String>,
    send_sequence: u64,
    last_received: Option<u64>,
    hello_request: Option<String>,
    probe_request: Option<(String, String)>,
    shutdown_request: Option<String>,
    closed: bool,
}

impl HostSession {
    pub fn new(
        session_id: impl Into<String>,
        versions: VersionRange,
    ) -> Result<Self, ProtocolError> {
        let session_id = session_id.into();
        validate_id(&session_id)?;
        versions.validate()?;
        Ok(Self {
            session_id,
            versions,
            selected_version: None,
            worker_build_id: None,
            send_sequence: 0,
            last_received: None,
            hello_request: None,
            probe_request: None,
            shutdown_request: None,
            closed: false,
        })
    }

    pub fn selected_version(&self) -> Option<u16> {
        self.selected_version
    }

    pub fn is_handshaken(&self) -> bool {
        self.selected_version.is_some() && !self.closed
    }

    pub fn hello(
        &mut self,
        request_id: impl Into<String>,
        build_id: impl Into<String>,
    ) -> Result<Envelope, ProtocolError> {
        if self.hello_request.is_some() || self.selected_version.is_some() || self.closed {
            return Err(ProtocolError::UnexpectedMessage);
        }
        let request_id = request_id.into();
        let build_id = build_id.into();
        validate_id(&request_id)?;
        validate_text(&build_id)?;
        self.hello_request = Some(request_id.clone());
        Ok(self.outbound(
            request_id,
            Message::Hello {
                versions: self.versions,
                build_id,
                capabilities: diagnostic_capabilities(),
            },
        ))
    }

    pub fn probe(
        &mut self,
        request_id: impl Into<String>,
        challenge: impl Into<String>,
    ) -> Result<Envelope, ProtocolError> {
        if !self.is_handshaken() || self.probe_request.is_some() || self.shutdown_request.is_some()
        {
            return Err(ProtocolError::UnexpectedMessage);
        }
        let request_id = request_id.into();
        let challenge = challenge.into();
        validate_id(&request_id)?;
        validate_text(&challenge)?;
        self.probe_request = Some((request_id.clone(), challenge.clone()));
        Ok(self.outbound(request_id, Message::Probe { challenge }))
    }

    pub fn shutdown(&mut self, request_id: impl Into<String>) -> Result<Envelope, ProtocolError> {
        if !self.is_handshaken() || self.shutdown_request.is_some() {
            return Err(ProtocolError::UnexpectedMessage);
        }
        let request_id = request_id.into();
        validate_id(&request_id)?;
        self.probe_request = None;
        self.shutdown_request = Some(request_id.clone());
        Ok(self.outbound(request_id, Message::Shutdown))
    }

    /// Validation is transactional: rejected messages never change accepted sequence/state.
    pub fn accept(&mut self, envelope: &Envelope) -> Result<HostEvent, ProtocolError> {
        envelope.validate_diagnostic_context()?;
        if envelope.session_id != self.session_id {
            return Err(ProtocolError::WrongSession);
        }
        if self.closed {
            return Err(ProtocolError::UnexpectedMessage);
        }
        if self.last_received.is_none() && envelope.sequence != 0
            || self
                .last_received
                .is_some_and(|previous| envelope.sequence <= previous)
        {
            return Err(ProtocolError::Sequence {
                last: self.last_received,
                received: envelope.sequence,
            });
        }
        if let Some(version) = self.selected_version
            && envelope.version != Some(version)
        {
            return Err(ProtocolError::VersionMismatch);
        }
        if let Message::ProtocolError { code, summary, .. } = &envelope.message {
            if self.hello_request.as_deref() != Some(envelope.request_id.as_str())
                && self.probe_request.as_ref().map(|(id, _)| id.as_str())
                    != Some(envelope.request_id.as_str())
                && self.shutdown_request.as_deref() != Some(envelope.request_id.as_str())
            {
                return Err(ProtocolError::WrongRequest);
            }
            validate_text(code)?;
            validate_text(summary)?;
            return Err(ProtocolError::Remote {
                code: code.clone(),
                summary: summary.clone(),
            });
        }
        let event = match &envelope.message {
            Message::HelloAccepted {
                versions,
                selected_version,
                build_id,
                capabilities,
            } if self.selected_version.is_none() => {
                if self.hello_request.as_deref() != Some(&envelope.request_id) {
                    return Err(ProtocolError::WrongRequest);
                }
                if self.versions.highest_common(*versions) != Some(*selected_version)
                    || envelope.version != Some(*selected_version)
                {
                    return Err(ProtocolError::VersionMismatch);
                }
                validate_text(build_id)?;
                validate_capabilities(capabilities)?;
                self.selected_version = Some(*selected_version);
                self.worker_build_id = Some(build_id.clone());
                self.hello_request = None;
                HostEvent::HandshakeAccepted
            }
            _ if self.selected_version.is_none() => {
                return Err(ProtocolError::UnexpectedMessage);
            }
            Message::Progress { completed, total } => {
                self.check_probe_request(&envelope.request_id)?;
                if *total == 0 || completed > total {
                    return Err(ProtocolError::InvalidEnvelope("invalid progress units"));
                }
                HostEvent::Progress
            }
            Message::Heartbeat => HostEvent::Heartbeat,
            Message::ProbeResult {
                challenge,
                build_id,
                scope,
                ..
            } => {
                self.check_probe_request(&envelope.request_id)?;
                if self.probe_request.as_ref().map(|(_, value)| value) != Some(challenge) {
                    return Err(ProtocolError::ChallengeMismatch);
                }
                if scope != DIAGNOSTIC_SCOPE {
                    return Err(ProtocolError::ScopeMismatch);
                }
                validate_text(build_id)?;
                if self.worker_build_id.as_deref() != Some(build_id.as_str()) {
                    return Err(ProtocolError::InvalidEnvelope(
                        "probe build identity differs from handshake",
                    ));
                }
                self.probe_request = None;
                HostEvent::ProbeCompleted
            }
            Message::ShutdownAck => {
                if self.shutdown_request.as_deref() != Some(&envelope.request_id) {
                    return Err(ProtocolError::WrongRequest);
                }
                self.shutdown_request = None;
                self.closed = true;
                HostEvent::ShutdownAcknowledged
            }
            _ => return Err(ProtocolError::UnexpectedMessage),
        };
        self.last_received = Some(envelope.sequence);
        Ok(event)
    }

    fn check_probe_request(&self, request_id: &str) -> Result<(), ProtocolError> {
        if self.probe_request.as_ref().map(|(id, _)| id.as_str()) != Some(request_id) {
            return Err(ProtocolError::WrongRequest);
        }
        Ok(())
    }

    fn outbound(&mut self, request_id: String, message: Message) -> Envelope {
        let envelope = Envelope::new(
            self.selected_version,
            &self.session_id,
            request_id,
            self.send_sequence,
            message,
        );
        self.send_sequence += 1;
        envelope
    }
}

pub fn diagnostic_capabilities() -> Vec<String> {
    vec![DIAGNOSTIC_CAPABILITY.to_owned()]
}

pub fn validate_capabilities(values: &[String]) -> Result<(), ProtocolError> {
    if values.len() != 1 || values[0] != DIAGNOSTIC_CAPABILITY {
        return Err(ProtocolError::CapabilityMismatch);
    }
    Ok(())
}

fn validate_id(value: &str) -> Result<(), ProtocolError> {
    if value.is_empty()
        || value.len() > 256
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
    {
        return Err(ProtocolError::InvalidEnvelope("invalid identifier"));
    }
    Ok(())
}

fn validate_text(value: &str) -> Result<(), ProtocolError> {
    if value.trim().is_empty() || value.len() > 1024 {
        return Err(ProtocolError::InvalidEnvelope(
            "empty or oversized text field",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
