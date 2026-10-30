use crate::ports::WorkspaceQuery;
pub use crate::ports::WorkspaceSnapshot;

#[derive(Default)]
pub struct WorkspaceService;

impl WorkspaceQuery for WorkspaceService {
    fn snapshot(&self) -> WorkspaceSnapshot {
        WorkspaceSnapshot {
            inference_available: false,
            status: "尚未配置推理".into(),
            supported_models: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_never_claims_model_support() {
        let state = WorkspaceService.snapshot();
        assert!(!state.inference_available);
        assert_eq!(state.supported_models, 0);
        assert_eq!(state.status, "尚未配置推理");
    }
}
