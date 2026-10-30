"""Managed, offline, single-attempt MFLUX worker."""
BUILD_ID = 'studio-mflux-worker-0.1.0'
RUNTIME_LOCK_ID = 'mflux-0.24.0-py3.13.12-macos-arm64-v1'
MODEL_SNAPSHOT_ID = 'z-image-turbo-f332072aa78be7aecdf3ee76d5c247082da564a6'
CAPABILITY = 'image.text-to-image.z-image-turbo.v1'
IDENTITY = dict(build_id=BUILD_ID, runtime_lock_id=RUNTIME_LOCK_ID, model_snapshot_id=MODEL_SNAPSHOT_ID)
