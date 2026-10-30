#!/usr/bin/env python3
"""Run the fixed local Z-Image tokenizer boundary probe without loading weights."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DATA_ROOT = Path.home() / "Library/Application Support/ImageStudio"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data-root")
    parser.add_argument("--report")
    parser.add_argument("--network-denied", action="store_true")
    parser.add_argument("--worker", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--model-path", help=argparse.SUPPRESS)
    return parser.parse_args()


def worker(model_path: Path) -> int:
    from mflux.models.common.tokenizer.tokenizer_loader import TokenizerLoader
    from mflux.models.z_image.weights.z_image_weight_definition import (
        ZImageWeightDefinition,
    )
    from studio_mflux_worker.backend import token_budget

    tokenizers = TokenizerLoader.load_all(
        ZImageWeightDefinition.get_tokenizers(), str(model_path)
    )
    tokenizer = tokenizers["z_image"]
    short_prompt = "一只橙色的小猫坐在窗边，窗外是秋天的树叶，温暖自然光，细腻的水彩插画。"
    short_tokens = token_budget(tokenizer, short_prompt)
    long_prompt = "秋" * 1700
    long_error = None
    try:
        token_budget(tokenizer, long_prompt)
    except ValueError as error:
        long_error = str(error)
    result = {
        "short_prompt_bytes": len(short_prompt.encode()),
        "short_prompt_template_tokens": short_tokens,
        "short_prompt_within_limit": short_tokens <= 512,
        "long_prompt_bytes": len(long_prompt.encode()),
        "long_prompt_rejected": long_error is not None,
        "long_prompt_rejection": long_error,
        "truncation_disabled": True,
        "model_weights_loaded": False,
    }
    print(json.dumps(result, ensure_ascii=False))
    return 0 if result["short_prompt_within_limit"] and result["long_prompt_rejected"] else 1


def main() -> int:
    args = parse_args()
    if args.worker:
        if not args.model_path:
            print("missing internal model path", file=sys.stderr)
            return 3
        return worker(Path(args.model_path).resolve())

    if not args.data_root or not args.report:
        print("--data-root and --report are required", file=sys.stderr)
        return 3
    data_root = Path(args.data_root).expanduser()
    report = Path(args.report).expanduser()
    if not data_root.is_absolute() or not report.is_absolute():
        print("--data-root and --report must be absolute", file=sys.stderr)
        return 3
    data_root = data_root.resolve()
    report = report.resolve()
    default = DEFAULT_DATA_ROOT.resolve()
    if data_root == default or default in data_root.parents or data_root in default.parents:
        print("--data-root must be isolated from the default ImageStudio root", file=sys.stderr)
        return 3
    if report == default or default in report.parents:
        print("--report cannot be inside the default ImageStudio root", file=sys.stderr)
        return 3

    ready_path = data_root / "data/installation-ready.json"
    try:
        ready = json.loads(ready_path.read_text())
    except (OSError, ValueError) as error:
        print(f"installation-ready record unavailable: {error}", file=sys.stderr)
        return 2
    runtime_id = ready.get("runtime_lock_id")
    model_id = ready.get("model_snapshot_id")
    if not isinstance(runtime_id, str) or not isinstance(model_id, str):
        print("installation-ready record lacks runtime/model identity", file=sys.stderr)
        return 2
    runtime_root = data_root / "data/runtimes" / runtime_id
    model_root = data_root / "data/models" / model_id
    private_python = runtime_root / "venv/bin/python3"
    if not private_python.is_file():
        print("private runtime Python is missing", file=sys.stderr)
        return 2
    if not model_root.is_dir():
        print("fixed model directory is missing", file=sys.stderr)
        return 2

    command = [
        str(private_python),
        str(Path(__file__).resolve()),
        "--worker",
        "--model-path",
        str(model_root),
    ]
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
    parsed = None
    for line in reversed(process.stdout.splitlines()):
        try:
            value = json.loads(line)
        except ValueError:
            continue
        if isinstance(value, dict) and "short_prompt_template_tokens" in value:
            parsed = value
            break
    if parsed is None:
        print(process.stderr.strip() or "tokenizer worker produced no JSON result", file=sys.stderr)
        return 1
    record = {
        "schema_version": 1,
        "kind": "actual-fixed-tokenizer-offline-preflight",
        "network_denied": args.network_denied,
        "is_full_model_inference": False,
        "data_root": str(data_root),
        "runtime_lock_id": runtime_id,
        "model_snapshot_id": model_id,
        "private_python": str(private_python),
        "worker_exit_code": process.returncode,
        **parsed,
        "worker_stderr_tail": process.stderr[-4096:],
    }
    report.parent.mkdir(parents=True, exist_ok=True)
    temporary = report.with_name(f".{report.name}.tmp")
    temporary.write_text(json.dumps(record, ensure_ascii=False, indent=2) + "\n")
    temporary.replace(report)
    print(report)
    return (
        0
        if process.returncode == 0
        and parsed.get("short_prompt_within_limit") is True
        and parsed.get("long_prompt_rejected") is True
        and parsed.get("truncation_disabled") is True
        and parsed.get("model_weights_loaded") is False
        else 1
    )


if __name__ == "__main__":
    raise SystemExit(main())
