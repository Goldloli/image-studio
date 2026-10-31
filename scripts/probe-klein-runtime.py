#!/usr/bin/env python3
"""Probe the installed Klein tokenizer and upstream reference preprocessing."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from typing import Any


ROOT = Path(__file__).resolve().parent.parent
INSTALL_ID = "flux2-klein-4b-distilled"
DEFAULT_DATA_ROOT = Path.home() / "Library/Application Support/ImageStudio"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("tokenizer", "reference"))
    parser.add_argument("--data-root")
    parser.add_argument("--report")
    parser.add_argument("--network-denied", action="store_true")
    parser.add_argument("--source", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--worker", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--model-path", help=argparse.SUPPRESS)
    parser.add_argument("--source-path", help=argparse.SUPPRESS)
    parser.add_argument("--output-path", help=argparse.SUPPRESS)
    return parser.parse_args()


def tokenizer_worker(model_path: Path) -> int:
    from mflux.models.common.tokenizer.tokenizer_loader import TokenizerLoader
    from mflux.models.flux2.weights.flux2_weight_definition import (
        Flux2KleinWeightDefinition,
    )
    from studio_mflux_worker_v2.backend import token_budget

    tokenizer = TokenizerLoader.load_all(
        Flux2KleinWeightDefinition.get_tokenizers(), str(model_path)
    )["qwen3"]

    token_counts: dict[int, int] = {}

    def count(characters: int) -> int:
        if characters not in token_counts:
            token_counts[characters] = token_budget(tokenizer, "秋" * characters)
        return token_counts[characters]

    low, high = 1, 4096
    while low < high:
        middle = (low + high + 1) // 2
        try:
            tokens = count(middle)
        except ValueError:
            high = middle - 1
        else:
            if tokens <= 512:
                low = middle
            else:
                high = middle - 1
    boundary_characters = low
    boundary_tokens = count(boundary_characters)
    over_characters = boundary_characters + 1
    if boundary_tokens != 512:
        raise RuntimeError(
            f"expected an exact 512-token boundary, found {boundary_tokens}"
        )
    try:
        token_budget(tokenizer, "秋" * over_characters)
    except ValueError as error:
        over_error = str(error)
    else:
        raise RuntimeError("the next character was not rejected")

    boundary_prompt = "秋" * boundary_characters
    over_prompt = "秋" * over_characters
    result = {
        "boundary_prompt_characters": boundary_characters,
        "boundary_prompt_bytes": len(boundary_prompt.encode()),
        "boundary_prompt_sha256": hashlib.sha256(boundary_prompt.encode()).hexdigest(),
        "boundary_prompt_template_tokens": boundary_tokens,
        "boundary_prompt_within_limit": boundary_tokens <= 512,
        "over_prompt_characters": over_characters,
        "over_prompt_bytes": len(over_prompt.encode()),
        "over_prompt_sha256": hashlib.sha256(over_prompt.encode()).hexdigest(),
        "over_prompt_rejected": True,
        "over_prompt_rejection": over_error,
        "thinking_disabled": tokenizer.chat_template_kwargs.get("enable_thinking")
        is False,
        "truncation_disabled": True,
        "model_weights_loaded": False,
    }
    print(json.dumps(result, ensure_ascii=False))
    return 0


def reference_worker(source: Path, output: Path) -> int:
    import PIL.Image

    from mflux.models.flux2.variants.edit.flux2_klein_edit_helpers import (
        MAX_REFERENCE_AREA,
        REFERENCE_DIM_MULTIPLE,
        _Flux2KleinEditHelpers,
    )

    source_before = sha256(source)
    with PIL.Image.open(source) as image:
        image.load()
        source_width, source_height = image.size
        source_mode = image.mode
        target_width, target_height = _Flux2KleinEditHelpers.reference_dims(
            source_width, source_height
        )
        prepared = _Flux2KleinEditHelpers.prepare_reference_image(image)
        output.parent.mkdir(parents=True, exist_ok=True)
        prepared.save(output, format="PNG")
        output_width, output_height = prepared.size
    source_after = sha256(source)
    if source_before != source_after:
        raise RuntimeError("reference preprocessing changed the source file")
    if (output_width, output_height) != (target_width, target_height):
        raise RuntimeError("prepared reference dimensions do not match the pinned contract")

    scaled_width = source_width
    scaled_height = source_height
    if source_width * source_height > MAX_REFERENCE_AREA:
        scale = (MAX_REFERENCE_AREA / (source_width * source_height)) ** 0.5
        scaled_width = round(source_width * scale)
        scaled_height = round(source_height * scale)
    result = {
        "source_width": source_width,
        "source_height": source_height,
        "source_mode": source_mode,
        "source_area": source_width * source_height,
        "source_sha256_before": source_before,
        "source_sha256_after": source_after,
        "source_unchanged": source_before == source_after,
        "max_reference_area": MAX_REFERENCE_AREA,
        "reference_dim_multiple": REFERENCE_DIM_MULTIPLE,
        "area_downscaled": source_width * source_height > MAX_REFERENCE_AREA,
        "scaled_width": scaled_width,
        "scaled_height": scaled_height,
        "center_crop_left": (scaled_width - output_width) // 2,
        "center_crop_top": (scaled_height - output_height) // 2,
        "center_crop_right": (scaled_width - output_width)
        - (scaled_width - output_width) // 2,
        "center_crop_bottom": (scaled_height - output_height)
        - (scaled_height - output_height) // 2,
        "output_width": output_width,
        "output_height": output_height,
        "output_aspect_ratio": output_width / output_height,
        "source_aspect_ratio": source_width / source_height,
        "output_sha256": sha256(output),
        "output_bytes": output.stat().st_size,
        "algorithm": "mflux-0.24.0 Flux2KleinEditHelpers.prepare_reference_image",
        "model_weights_loaded": False,
    }
    print(json.dumps(result, ensure_ascii=False))
    return 0


def installed_runtime(data_root: Path) -> tuple[dict[str, Any], Path, Path]:
    activation = (
        data_root / "data/installations/models" / f"{INSTALL_ID}.json"
    )
    record = json.loads(activation.read_text())
    runtime_id = record.get("runtime_lock_id")
    snapshot_id = record.get("model_snapshot_id")
    if not isinstance(runtime_id, str) or not isinstance(snapshot_id, str):
        raise RuntimeError("Klein activation record lacks runtime/model identity")
    runtime_root = data_root / "data/runtimes" / runtime_id
    model_root = data_root / "data/models" / snapshot_id
    private_python = runtime_root / "venv/bin/python3"
    if not private_python.is_file() or not model_root.is_dir():
        raise RuntimeError("the installed Klein runtime or model snapshot is missing")
    return record, private_python, model_root


def last_json_line(stdout: str) -> dict[str, Any]:
    for line in reversed(stdout.splitlines()):
        try:
            value = json.loads(line)
        except ValueError:
            continue
        if isinstance(value, dict):
            return value
    raise RuntimeError("runtime probe produced no JSON result")


def main() -> int:
    args = parse_args()
    if args.worker:
        if not args.model_path:
            raise RuntimeError("missing internal model path")
        if args.mode == "tokenizer":
            return tokenizer_worker(Path(args.model_path).resolve())
        if not args.source_path or not args.output_path:
            raise RuntimeError("missing internal reference paths")
        return reference_worker(
            Path(args.source_path).resolve(), Path(args.output_path).resolve()
        )

    if not args.data_root or not args.report:
        raise RuntimeError("--data-root and --report are required")
    data_root = Path(args.data_root).expanduser().resolve()
    report = Path(args.report).expanduser().resolve()
    default = DEFAULT_DATA_ROOT.resolve()
    if data_root == default or default in data_root.parents or data_root in default.parents:
        raise RuntimeError("--data-root must be isolated from the default ImageStudio root")
    if report == default or default in report.parents:
        raise RuntimeError("--report cannot be inside the default ImageStudio root")

    activation, private_python, model_root = installed_runtime(data_root)
    command = [
        str(private_python),
        str(Path(__file__).resolve()),
        args.mode,
        "--worker",
        "--model-path",
        str(model_root),
    ]
    source = None
    output = None
    if args.mode == "reference":
        if not args.source or not args.output:
            raise RuntimeError("reference mode requires --source and --output")
        source = args.source.expanduser().resolve()
        output = args.output.expanduser().resolve()
        if not source.is_file() or source.is_symlink():
            raise RuntimeError("--source must be an absolute regular file")
        if output == source:
            raise RuntimeError("--output must differ from --source")
        command.extend(
            ["--source-path", str(source), "--output-path", str(output)]
        )

    environment = os.environ.copy()
    for key in (
        "HF_HUB_OFFLINE",
        "TRANSFORMERS_OFFLINE",
        "HF_HUB_DISABLE_TELEMETRY",
        "DO_NOT_TRACK",
    ):
        environment[key] = "1"
    process = subprocess.run(
        command,
        cwd=ROOT,
        env=environment,
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
    )
    result: dict[str, Any] = {
        "schemaVersion": 1,
        "runId": "planned-models-20261031",
        "configurationKey": "K-T/K-E",
        "evidenceKind": "device_probe",
        "modelId": INSTALL_ID,
        "modelSnapshotId": activation["model_snapshot_id"],
        "runtimeLockId": activation["runtime_lock_id"],
        "runtimeManifestSha256": activation["runtime_manifest_sha256"],
        "modelManifestSha256": activation["model_manifest_sha256"],
        "dataRoot": str(data_root),
        "privatePython": str(private_python),
        "networkDenied": args.network_denied,
        "workerExitCode": process.returncode,
        "workerStderrTail": process.stderr[-4096:],
    }
    if result["workerExitCode"] != 0:
        result["status"] = "failed"
        result["reason"] = "runtime probe failed"
    else:
        probe = last_json_line(process.stdout)
        result.update(probe)
        if args.mode == "reference":
            result["sourcePath"] = str(source)
            result["outputPath"] = str(output)
            result["sourceAssetSha256"] = sha256(source)
            result["outputAssetSha256"] = sha256(output)
            result["status"] = (
                "passed"
                if probe["source_unchanged"]
                and probe["source_sha256_before"] == probe["source_sha256_after"]
                else "failed"
            )
        else:
            result["status"] = (
                "passed"
                if probe["boundary_prompt_within_limit"]
                and probe["over_prompt_rejected"]
                and probe["thinking_disabled"]
                else "failed"
            )
    report.parent.mkdir(parents=True, exist_ok=True)
    temporary = report.with_name(f".{report.name}.tmp")
    temporary.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    temporary.replace(report)
    print(report)
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
