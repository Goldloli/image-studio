"""Managed, offline MFLUX worker for FLUX.2 Klein 4B."""

BUILD_ID = "studio-mflux-worker-0.2.0"
RUNTIME_LOCK_ID = "mflux-0.24.0-py3.13.12-macos-arm64-recipes-v2"
MODEL_SNAPSHOT_ID = (
    "flux2-klein-4b-e7b7dc27f91deacad38e78976d1f2b499d76a294"
)
TEXT_CAPABILITY = "image.text-to-image.flux2-klein-4b.v1"
EDIT_CAPABILITY = "image.edit.flux2-klein-4b.v1"
CAPABILITIES = [TEXT_CAPABILITY, EDIT_CAPABILITY]
IDENTITY = dict(
    build_id=BUILD_ID,
    runtime_lock_id=RUNTIME_LOCK_ID,
    model_snapshot_id=MODEL_SNAPSHOT_ID,
)
