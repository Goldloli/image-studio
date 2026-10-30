use crate::{ContractError, nonempty, require};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

macro_rules! opaque_id {
    ($($name:ident),+ $(,)?) => {$(
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
                let value = value.into();
                require(
                    !value.is_empty() && value.len() <= 128
                        && value.bytes().all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c)),
                    concat!(stringify!($name), " must be an opaque 1..128 byte identifier, not a path"),
                )?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str { &self.0 }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
            }
        }
    )+};
}

opaque_id!(
    ProjectId,
    ConversationId,
    TurnId,
    AssetId,
    ModelVariantId,
    JobId,
    AttemptId,
    PlanId,
    RequestId,
    StorageLocationId,
    ExecutionToken,
    IdempotencyKey,
);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ContentHash(String);

impl ContentHash {
    /// A SHA-256 digest identifies bytes; no filesystem operation is performed.
    pub fn new(hex: impl Into<String>) -> Result<Self, ContractError> {
        let hex = hex.into();
        require(
            hex.len() == 64 && hex.bytes().all(|c| c.is_ascii_hexdigit()),
            "SHA-256 digest must contain 64 hexadecimal characters",
        )?;
        Ok(Self(hex.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StorageLocation {
    Managed {
        storage_id: StorageLocationId,
        relative_path: String,
    },
    External {
        path: String,
    },
}

impl StorageLocation {
    pub fn validate(&self) -> Result<(), ContractError> {
        match self {
            Self::Managed { relative_path, .. } => require(
                !relative_path.is_empty()
                    && !relative_path.starts_with('/')
                    && !relative_path.contains(['\\', ':', '\0'])
                    && relative_path
                        .split('/')
                        .all(|part| !part.is_empty() && part != "." && part != ".."),
                "managed location must be a platform-neutral relative path without traversal",
            ),
            Self::External { path } => nonempty(path, "external path"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    ImageRgb,
    ImageRgba,
    Mask,
    Depth,
    StructuredData,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetRef {
    pub asset_id: AssetId,
    pub content_hash: ContentHash,
    pub data_type: AssetKind,
    pub location: StorageLocation,
}

impl AssetRef {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.location.validate()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    pub attempt_id: AttemptId,
    pub job_id: JobId,
    pub execution_token: ExecutionToken,
}

impl Attempt {
    pub fn validate_event(&self, event: &Self) -> Result<(), ContractError> {
        if self.job_id != event.job_id || self.attempt_id != event.attempt_id {
            return Err(ContractError::StaleAttempt);
        }
        if self.execution_token != event.execution_token {
            return Err(ContractError::StaleToken);
        }
        Ok(())
    }
}

/// A work identity and append-only attempt history, without a scheduler/state machine.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub job_id: JobId,
    attempts: Vec<Attempt>,
}

impl Job {
    pub fn new(job_id: JobId) -> Self {
        Self {
            job_id,
            attempts: Vec::new(),
        }
    }
    pub fn attempts(&self) -> &[Attempt] {
        &self.attempts
    }

    pub fn add_attempt(&mut self, attempt: Attempt) -> Result<(), ContractError> {
        require(
            attempt.job_id == self.job_id,
            "attempt must belong to its job",
        )?;
        require(
            self.attempts.iter().all(|old| {
                old.attempt_id != attempt.attempt_id
                    && old.execution_token != attempt.execution_token
            }),
            "retry requires a new attempt ID and execution token",
        )?;
        self.attempts.push(attempt);
        Ok(())
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        let mut checked = Self::new(self.job_id.clone());
        for attempt in &self.attempts {
            checked.add_attempt(attempt.clone())?;
        }
        Ok(())
    }

    pub fn validate_event(&self, event: &Attempt) -> Result<(), ContractError> {
        self.validate()?;
        self.attempts
            .last()
            .ok_or(ContractError::NoActiveAttempt)?
            .validate_event(event)
    }
}
