#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSnapshot {
    pub inference_available: bool,
    pub status: String,
    pub supported_models: usize,
}

/// Read-only boundary used by the desktop shell.
pub trait WorkspaceQuery {
    fn snapshot(&self) -> WorkspaceSnapshot;
}
