//! Platform-independent contracts for Image Studio.
//!
//! Contract fixtures validate data semantics; they are not evidence of model execution.
//!
//! IDs cannot be interchanged accidentally:
//! ```compile_fail
//! use studio_core::domain::{AssetId, JobId};
//! fn accepts_job(_: JobId) {}
//! accepts_job(AssetId::new("asset-example").unwrap());
//! ```

pub mod application;
pub mod contracts;
pub mod domain;
pub mod generation;
pub mod ports;

use thiserror::Error;

/// Stable categories distinguish version, structure, and semantic errors.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum ContractError {
    #[error("{document} schema version {found} is unsupported (expected {supported})")]
    UnsupportedVersion {
        document: &'static str,
        found: u64,
        supported: u32,
    },
    #[error("missing required field: {0}")]
    MissingField(String),
    #[error("unknown field: {0}")]
    UnknownField(String),
    #[error("invalid JSON: {0}")]
    InvalidJson(String),
    #[error("invalid value: {0}")]
    InvalidValue(String),
    #[error("event belongs to a different attempt")]
    StaleAttempt,
    #[error("event execution token is not current")]
    StaleToken,
    #[error("job has no active attempt")]
    NoActiveAttempt,
}

pub(crate) fn require(condition: bool, message: impl Into<String>) -> Result<(), ContractError> {
    if condition {
        Ok(())
    } else {
        Err(ContractError::InvalidValue(message.into()))
    }
}

pub(crate) fn nonempty(value: &str, field: &str) -> Result<(), ContractError> {
    require(
        !value.trim().is_empty() && !value.contains('\0'),
        format!("{field} must not be empty or contain NUL"),
    )
}
