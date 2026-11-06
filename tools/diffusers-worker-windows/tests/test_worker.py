"""Frozen identity, recipe, protocol-shell and vendored-module unit tests."""

import hashlib
import json
from pathlib import Path

import pytest

from studio_diffusers_worker import (
    BUILD_ID,
    CAP_KLEIN_EDIT,
    CAP_KLEIN_TEXT,
    CAP_MOSS,
    CAP_QWEN,
    CAP_QWEN_TURBO,
    KLEIN_SNAPSHOT,
    MOSS_SNAPSHOT,
    QWEN_SNAPSHOT,
    QWEN_TURBO_SNAPSHOT,
    RUNTIME_ID,
    SNAPSHOTS,
    expected_parameters,
    model_spec,
    output_specs,
    recipe_for,
    worker_identity,
)
from studio_diffusers_worker.__main__ import (
    parse_arguments,
    validate_envelope,
    validate_request,
)
from studio_diffusers_worker.vendored import (
    MOSS_PIPELINE_BYTES,
    MOSS_PIPELINE_SHA256,
    verify_pipeline,
)

SNAPSHOT_CAPABILITY = [
    (KLEIN_SNAPSHOT, CAP_KLEIN_TEXT),
    (KLEIN_SNAPSHOT, CAP_KLEIN_EDIT),
    (QWEN_SNAPSHOT, CAP_QWEN),
    (QWEN_TURBO_SNAPSHOT, CAP_QWEN_TURBO),
    (MOSS_SNAPSHOT, CAP_MOSS),
]


def source_file(output_dir: Path) -> dict:
    path = output_dir / "source.png"
    path.write_bytes(b"\x89PNG\r\n\x1a\nunit-test-source")
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    return {
        "role": "source",
        "path": str(path.resolve()),
        "width": 64,
        "height": 64,
        "sha256": digest,
    }


def request_for(tmp_path, snapshot_id, capability):
    spec = model_spec(snapshot_id)
    model = tmp_path / "models" / snapshot_id
    model.mkdir(parents=True, exist_ok=True)
    (model / "model_index.json").write_text("{}", encoding="utf-8")
    output = tmp_path / "attempt" / snapshot_id
    output.mkdir(parents=True, exist_ok=True)
    seed = 3148150817
    inputs = []
    if capability == CAP_KLEIN_EDIT:
        inputs = [source_file(output)]
    return {
        "model_id": spec["model_id"],
        "capability_id": capability,
        "recipe_id": recipe_for(snapshot_id, capability)[0],
        "recipe_version": 1,
        "runtime_lock_id": RUNTIME_ID,
        "model_snapshot_id": snapshot_id,
        "instruction": "Windows 中文 prompt for a real scene",
        "seed": seed,
        "sampling_steps": recipe_for(snapshot_id, capability)[1],
        "parameters": expected_parameters(snapshot_id, capability, seed),
        "inputs": inputs,
        "outputs": output_specs(snapshot_id),
        "model_path": str(model.resolve()),
        "output_dir": str(output.resolve()),
    }


def test_snapshot_identity_and_specs_are_frozen():
    assert BUILD_ID == "studio-diffusers-worker-windows-0.1.0"
    assert RUNTIME_ID == "windows-python31210-torch2100-cu126-diff041-transformers53-v2"
    assert set(SNAPSHOTS) == {
        KLEIN_SNAPSHOT,
        QWEN_SNAPSHOT,
        QWEN_TURBO_SNAPSHOT,
        MOSS_SNAPSHOT,
    }
    for snapshot_id in SNAPSHOTS:
        identity = worker_identity(snapshot_id)
        assert identity == {
            "build_id": BUILD_ID,
            "runtime_lock_id": RUNTIME_ID,
            "model_snapshot_id": snapshot_id,
        }
    assert model_spec(KLEIN_SNAPSHOT)["capabilities"] == [
        CAP_KLEIN_TEXT,
        CAP_KLEIN_EDIT,
    ]
    assert model_spec(QWEN_SNAPSHOT)["capabilities"] == [CAP_QWEN]
    assert model_spec(QWEN_TURBO_SNAPSHOT)["capabilities"] == [CAP_QWEN_TURBO]
    assert model_spec(MOSS_SNAPSHOT)["capabilities"] == [CAP_MOSS]
    with pytest.raises(ValueError):
        worker_identity("flux2-klein-4b-distilled")
    with pytest.raises(ValueError):
        model_spec("unknown-windows-snapshot")


def test_effective_parameters_match_design_section_11():
    klein = expected_parameters(KLEIN_SNAPSHOT, CAP_KLEIN_TEXT, 7)
    assert klein == {
        "recipe": "klein4b-text-windows-v1",
        "recipe_version": 1,
        "seed": 7,
        "steps": 4,
        "guidance": 1.0,
        "scheduler": "flow_match_euler_discrete",
        "token_budget": 512,
        "thinking": False,
        "truncation": False,
        "quantization": "nf4",
        "load_strategy": "nf4-component-offload-v1",
    }
    edit = expected_parameters(KLEIN_SNAPSHOT, CAP_KLEIN_EDIT, 7)
    assert edit["recipe"] == "klein4b-edit-windows-v1"
    qwen = expected_parameters(QWEN_SNAPSHOT, CAP_QWEN, 7)
    assert qwen["steps"] == 40 and qwen["sampling"] == "linear"
    assert "sampling_schedule" not in qwen
    turbo = expected_parameters(QWEN_TURBO_SNAPSHOT, CAP_QWEN_TURBO, 7)
    assert turbo["steps"] == 8
    assert turbo["sampling_schedule"] == "qwen-turbo-model-index-v1"
    moss = expected_parameters(MOSS_SNAPSHOT, CAP_MOSS, 7)
    assert moss["steps"] == 50 and moss["guidance"] == 3.0
    assert moss["negative_prompt"] == ""
    assert moss["uncond_skip_layers"] == [22]
    assert moss["text_encoder_precision"] == "nf4"
    assert moss["vae_precision"] == "float32"


def test_compact_parameters_match_the_frozen_protocol_maps():
    klein = expected_parameters(
        KLEIN_SNAPSHOT, CAP_KLEIN_TEXT, 7, "klein4b-text-windows-compact-v1"
    )
    assert klein == {
        "recipe": "klein4b-text-windows-compact-v1",
        "recipe_version": 1,
        "seed": 7,
        "steps": 4,
        "guidance": 1.0,
        "scheduler": "flow_match_euler_discrete",
        "token_budget": 512,
        "thinking": False,
        "truncation": False,
        "quantization": "nf4",
        "load_strategy": "nf4-sequential-offload-v1",
        "resolution": 768,
        "compute_dtype": "float16",
        "vae_precision": "float16",
    }
    edit = expected_parameters(
        KLEIN_SNAPSHOT, CAP_KLEIN_EDIT, 7, "klein4b-edit-windows-compact-v1"
    )
    assert edit["recipe"] == "klein4b-edit-windows-compact-v1"
    qwen = expected_parameters(
        QWEN_SNAPSHOT, CAP_QWEN, 7, "qwen21-windows-txt2img-nf4-40-compact-v1"
    )
    assert qwen["resolution"] == 768
    assert qwen["compute_dtype"] == "float16"
    assert qwen["load_strategy"] == "staged-nf4-sequential-v1"
    assert len(qwen) == 11
    turbo = expected_parameters(
        QWEN_TURBO_SNAPSHOT,
        CAP_QWEN_TURBO,
        7,
        "qwen21-turbo-windows-txt2img-nf4-8-compact-v1",
    )
    assert turbo["sampling_schedule"] == "qwen-turbo-model-index-v1"
    assert len(turbo) == 12
    moss = expected_parameters(
        MOSS_SNAPSHOT, CAP_MOSS, 7, "moss-image-sft-windows-txt2img-nf4-50-compact-v1"
    )
    assert moss["vae_precision"] == "float32"
    assert len(moss) == 16
    # A compact-shaped id that does not match the frozen mapping is rejected instead of
    # silently producing a standard map.
    with pytest.raises(ValueError):
        expected_parameters(
            QWEN_SNAPSHOT, CAP_QWEN, 7, "qwen21-windows-txt2img-nf4-40-windows-compact-v1"
        )
    assert output_specs(QWEN_SNAPSHOT, 768)[0]["width"] == 768
    assert output_specs(QWEN_SNAPSHOT)[0]["width"] == 1024


@pytest.mark.parametrize(("snapshot_id", "capability"), SNAPSHOT_CAPABILITY)
def test_frozen_requests_are_accepted(tmp_path, snapshot_id, capability):
    validate_request(request_for(tmp_path, snapshot_id, capability), snapshot_id)


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("model_id", "flux2-klein-4b-distilled"),
        ("capability_id", "image.text-to-image.flux2-klein-4b.v1"),
        ("recipe_id", "klein4b-text-v1"),
        ("recipe_version", 2),
        ("runtime_lock_id", "windows-python31210-torch2100-cu126-bnb0482-v1"),
        ("model_snapshot_id", QWEN_TURBO_SNAPSHOT),
        ("sampling_steps", 8),
        ("seed", None),
        ("instruction", ""),
    ],
)
def test_identity_and_instruction_mutations_are_rejected(
    tmp_path, field, value
):
    request = request_for(tmp_path, KLEIN_SNAPSHOT, CAP_KLEIN_TEXT)
    request[field] = value
    with pytest.raises(ValueError):
        validate_request(request, KLEIN_SNAPSHOT)


def test_parameters_and_outputs_are_strict(tmp_path):
    request = request_for(tmp_path, QWEN_TURBO_SNAPSHOT, CAP_QWEN_TURBO)
    request["parameters"]["steps"] = 40
    with pytest.raises(ValueError):
        validate_request(request, QWEN_TURBO_SNAPSHOT)

    request = request_for(tmp_path, QWEN_TURBO_SNAPSHOT, CAP_QWEN_TURBO)
    request["outputs"][0]["width"] = 768
    with pytest.raises(ValueError):
        validate_request(request, QWEN_TURBO_SNAPSHOT)

    request = request_for(tmp_path, QWEN_TURBO_SNAPSHOT, CAP_QWEN_TURBO)
    request["inputs"] = [source_file(Path(request["output_dir"]))]
    with pytest.raises(ValueError):
        validate_request(request, QWEN_TURBO_SNAPSHOT)


def test_klein_edit_source_descriptor_is_strict(tmp_path):
    request = request_for(tmp_path, KLEIN_SNAPSHOT, CAP_KLEIN_EDIT)
    request["inputs"][0]["sha256"] = "0" * 64
    with pytest.raises(ValueError):
        validate_request(request, KLEIN_SNAPSHOT)

    request = request_for(tmp_path, KLEIN_SNAPSHOT, CAP_KLEIN_EDIT)
    outside = tmp_path / "outside.png"
    outside.write_bytes(b"outside")
    request["inputs"][0]["path"] = str(outside.resolve())
    with pytest.raises(ValueError):
        validate_request(request, KLEIN_SNAPSHOT)

    request = request_for(tmp_path, KLEIN_SNAPSHOT, CAP_KLEIN_EDIT)
    request["inputs"] = []
    with pytest.raises(ValueError):
        validate_request(request, KLEIN_SNAPSHOT)


def test_incomplete_model_directory_is_rejected(tmp_path):
    request = request_for(tmp_path, MOSS_SNAPSHOT, CAP_MOSS)
    broken = tmp_path / "models" / "broken"
    broken.mkdir()
    request["model_path"] = str(broken.resolve())
    with pytest.raises(ValueError):
        validate_request(request, MOSS_SNAPSHOT)


def test_envelope_unknown_fields_are_rejected():
    value = {
        "family": "image-studio.generation",
        "version": 2,
        "session_id": "session",
        "request_id": "request",
        "sequence": 0,
        "execution": None,
        "message": {"type": "hello"},
        "unexpected": True,
    }
    with pytest.raises(ValueError):
        validate_envelope(value)


def test_installer_argument_shapes():
    probe = parse_arguments(
        ["--model-snapshot-id", KLEIN_SNAPSHOT, "--probe", "C:\\models\\klein"]
    )
    assert probe["probe"] and probe["snapshot_id"] == KLEIN_SNAPSHOT
    run = parse_arguments(["--model-snapshot-id", QWEN_TURBO_SNAPSHOT])
    assert not run["probe"] and not run["prepare"]
    prepare = parse_arguments(
        [
            "--model-snapshot-id",
            QWEN_TURBO_SNAPSHOT,
            "--prepare",
            "--model-dir",
            "C:\\models\\source",
            "--prepared-dir",
            "C:\\models\\prepared",
        ]
    )
    assert prepare["prepare"]
    assert prepare["model_dir"] == Path("C:\\models\\source")
    with pytest.raises(ValueError):
        parse_arguments(["--probe"])
    with pytest.raises(ValueError):
        parse_arguments(["--model-snapshot-id", KLEIN_SNAPSHOT, "--unknown"])


def test_vendored_moss_pipeline_is_the_reviewed_frozen_file():
    assert MOSS_PIPELINE_BYTES == 25060
    assert (
        MOSS_PIPELINE_SHA256
        == "1dac1aca9feb1bfdfc727a401e4b67d61a505d3190706d1b7d2d784f88678b31"
    )
    assert verify_pipeline() == MOSS_PIPELINE_SHA256
    import studio_diffusers_worker.vendored as vendored

    license_path = Path(vendored.__file__).parent / "LICENSE-MOSS-Image.txt"
    assert license_path.is_file()
    assert license_path.read_text(encoding="utf-8").lstrip().startswith(
        "Apache License"
    )


def test_protocol_constants_are_stable():
    assert json.loads(json.dumps(expected_parameters(MOSS_SNAPSHOT, CAP_MOSS, 1)))
    for capability in (CAP_QWEN, CAP_QWEN_TURBO, CAP_MOSS):
        assert output_specs(QWEN_SNAPSHOT if capability == CAP_QWEN else QWEN_TURBO_SNAPSHOT if capability == CAP_QWEN_TURBO else MOSS_SNAPSHOT) == [
            {
                "role": "primary",
                "data_type": "image_rgb",
                "width": 1024,
                "height": 1024,
                "max_bytes": 32 * 1024 * 1024,
            }
        ]

def test_prepare_klein_reports_not_required_and_missing_model_fails(tmp_path):
    from studio_diffusers_worker.backends.prepare import prepare_cache

    model = tmp_path / "models" / "klein"
    model.mkdir(parents=True)
    (model / "model_index.json").write_text("{}", encoding="utf-8")
    result = prepare_cache(KLEIN_SNAPSHOT, model, tmp_path / "prepared", True)
    assert result["status"] == "not_required"

    missing = prepare_cache(
        KLEIN_SNAPSHOT, tmp_path / "missing-model", tmp_path / "prepared2", True
    )
    assert missing["status"] == "error"
    assert "incomplete" in missing["error"]


def test_process_memory_api_reports_real_values():
    from studio_diffusers_worker.resources import (
        process_private_bytes,
        process_commit_peak_bytes,
        system_memory,
    )

    assert process_private_bytes() > 0
    assert process_commit_peak_bytes() > 0
    memory = system_memory()
    assert memory["physical_total_bytes"] > 0
    assert 0 < memory["physical_available_bytes"] <= memory["physical_total_bytes"]
