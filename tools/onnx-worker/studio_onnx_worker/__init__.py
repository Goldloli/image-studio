"""Managed, offline ONNX worker for BiRefNet general epoch_244."""

BUILD_ID = "studio-onnx-worker-0.1.0"
RUNTIME_LOCK_ID = "onnxruntime-1.31.1-py3.13.12-macos-arm64-cpu-v1"
MODEL_SNAPSHOT_ID = "birefnet-general-epoch-244-onnx"
CAPABILITY = "image.remove-background.birefnet-general-epoch-244.v1"
CAPABILITIES = [CAPABILITY]
IDENTITY = dict(
    build_id=BUILD_ID,
    runtime_lock_id=RUNTIME_LOCK_ID,
    model_snapshot_id=MODEL_SNAPSHOT_ID,
)
