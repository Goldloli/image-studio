import json

import pytest

from studio_zimage_cuda_worker import (
    MODEL_ID,
    expected_parameters,
    model_spec,
    worker_identity,
)
from studio_zimage_cuda_worker.__main__ import validate_envelope, validate_request


def request(tmp_path):
    model = tmp_path / "models" / MODEL_ID
    output = tmp_path / "runs" / "attempt"
    model.mkdir(parents=True, exist_ok=True)
    output.mkdir(parents=True, exist_ok=True)
    return {
        "model_id": MODEL_ID,
        "capability_id": model_spec()["capability"],
        "recipe_id": model_spec()["recipe"],
        "recipe_version": 1,
        "runtime_lock_id": worker_identity()["runtime_lock_id"],
        "model_snapshot_id": MODEL_ID,
        "instruction": "Windows 中文 prompt",
        "seed": 42,
        "sampling_steps": 9,
        "parameters": expected_parameters(42),
        "inputs": [],
        "outputs": [
            {
                "role": "primary",
                "data_type": "image_rgb",
                "width": 1024,
                "height": 1024,
                "max_bytes": 32 * 1024 * 1024,
            }
        ],
        "model_path": str(model.resolve()),
        "output_dir": str(output.resolve()),
    }


def test_fixed_request_is_accepted(tmp_path):
    validate_request(request(tmp_path), worker_identity(), model_spec())


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("model_id", "z-image-turbo-q4"),
        ("capability_id", "image.text-to-image.z-image-turbo.v1"),
        ("recipe_id", "z-image-turbo-q4-v1"),
        ("runtime_lock_id", "mflux-0.24.0"),
        ("model_snapshot_id", "z-image-turbo-q4"),
        ("sampling_steps", 8),
        ("seed", None),
    ],
)
def test_identity_and_sampling_mutations_are_rejected(tmp_path, field, value):
    value_request = request(tmp_path)
    value_request[field] = value
    with pytest.raises(ValueError):
        validate_request(value_request, worker_identity(), model_spec())


def test_parameters_and_output_are_strict(tmp_path):
    bad_parameters = request(tmp_path)
    bad_parameters["parameters"]["truncation"] = True
    with pytest.raises(ValueError):
        validate_request(bad_parameters, worker_identity(), model_spec())

    bad_output = request(tmp_path)
    bad_output["outputs"][0]["width"] = 768
    with pytest.raises(ValueError):
        validate_request(bad_output, worker_identity(), model_spec())


def test_envelope_unknown_fields_are_rejected():
    value = {
        "family": "image-studio.generation",
        "version": 2,
        "session_id": "session",
        "request_id": "request",
        "sequence": 0,
        "execution": None,
        "message": {"type": "hello"},
        "unexpected": json.loads("true"),
    }
    with pytest.raises(ValueError):
        validate_envelope(value)
