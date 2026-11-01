"""Validate and summarize the fixed Windows Gate 0 evidence set."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path


def load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    evidence = args.evidence.resolve()
    run_names = [
        ("run1", "candidate-a-run1.json", "candidate-a-run1-cat.png"),
        ("run2", "candidate-a-run2.json", "candidate-a-run2-teapot.png"),
        ("run3", "candidate-a-run3.json", "candidate-a-run3-desk.png"),
    ]
    runs = []
    for name, report_name, image_name in run_names:
        report = load(evidence / report_name)
        image = evidence / image_name
        if report["status"] != "passed":
            raise RuntimeError(f"{name} did not pass")
        if report["candidate"] != "a" or not report["is_full_model_inference"]:
            raise RuntimeError(f"{name} is not candidate A full inference")
        if report["request"]["width"] != 1024 or report["request"]["height"] != 1024:
            raise RuntimeError(f"{name} is not 1024x1024")
        if report["request"]["steps"] != 9 or report["request"]["guidance"] != 0.0:
            raise RuntimeError(f"{name} has the wrong sampling parameters")
        if report["callback_steps"] != list(range(9)):
            raise RuntimeError(f"{name} callback coverage is incomplete")
        if report["load_seconds"] > 600 or report["generation_seconds"] > 900:
            raise RuntimeError(f"{name} exceeded a time limit")
        if not report["budget"]["passed"]:
            raise RuntimeError(f"{name} exceeded a resource budget")
        if sha256(image) != report["output"]["sha256"]:
            raise RuntimeError(f"{name} PNG hash does not match its report")
        runs.append(
            {
                "name": name,
                "prompt": report["request"]["prompt"],
                "seed": report["request"]["seed"],
                "templated_tokens": report["request"]["templated_tokens"],
                "load_seconds": report["load_seconds"],
                "generation_seconds": report["generation_seconds"],
                "stage_seconds": report["stage_seconds"],
                "cuda_peak_reserved_bytes": report["budget"][
                    "cuda_peak_reserved_bytes"
                ],
                "private_commit_peak_bytes": report["budget"][
                    "private_commit_peak_bytes"
                ],
                "available_ram_min_bytes": report["budget"]["available_ram_min_bytes"],
                "callback_total": len(report["callback_steps"]),
                "image": image.name,
                "image_bytes": image.stat().st_size,
                "image_sha256": report["output"]["sha256"],
                "visual_checks": report["output"]["checks"],
            }
        )

    runtime_probe = load(evidence / "runtime-probe.json")
    model_download = load(evidence / "model-download.json")
    modelscope = load(evidence / "modelscope-equivalence.json")
    candidate_b = load(evidence / "candidate-b-load.json")
    default_attention_failure = load(
        evidence / "candidate-a-default-attention-failed.json"
    )
    for name, report in [
        ("runtime probe", runtime_probe),
        ("model download", model_download),
        ("ModelScope equivalence", modelscope),
    ]:
        if report["status"] != "passed":
            raise RuntimeError(f"{name} did not pass")
    if candidate_b["status"] != "load_only_passed":
        raise RuntimeError("candidate B load did not pass")
    if default_attention_failure["status"] != "failed":
        raise RuntimeError("default-attention failure evidence is missing")
    if modelscope["verified_file_count"] != 20:
        raise RuntimeError("ModelScope did not verify the entire closure")

    selected = load(evidence / "candidate-a-run1.json")
    result = {
        "schema_version": 1,
        "kind": "windows-gate0-result",
        "status": "passed",
        "product_activation": "not_run",
        "target": {
            "os": "Windows 10 Pro 22H2 10.0.19045",
            "gpu": runtime_probe["cuda"]["external_uuid_match"],
            "ram_total_bytes": runtime_probe["host"]["ram_total_bytes"],
        },
        "runtime": {
            "python": runtime_probe["platform"]["python_version"],
            "versions": runtime_probe["versions"],
            "bf16_matmul": runtime_probe["cuda"]["bf16_matmul"],
            "nf4_linear_forward_backward": runtime_probe["cuda"][
                "nf4_linear_forward_backward"
            ],
        },
        "model": {
            "repository": model_download["repository"],
            "revision": model_download["revision"],
            "component_bytes": model_download["component_bytes"],
            "verified_files": len(model_download["verified_files"]),
            "modelscope_verified_files": modelscope["verified_file_count"],
            "modelscope_equivalence": True,
        },
        "selected_candidate": {
            "id": "a",
            "configuration": selected["configuration"],
            "scheduler": selected["scheduler"]["class"],
            "scheduler_config": selected["scheduler"]["config"],
            "reason": (
                "Candidate A passed all budgets; candidate B was loaded but not needed "
                "for a full second sequence."
            ),
        },
        "runs": runs,
        "budget": {
            "cuda_peak_reserved_limit_bytes": int(10.5 * 1024**3),
            "cuda_peak_reserved_max_bytes": max(
                run["cuda_peak_reserved_bytes"] for run in runs
            ),
            "private_commit_limit_bytes": 24 * 1024**3,
            "private_commit_peak_max_bytes": max(
                run["private_commit_peak_bytes"] for run in runs
            ),
            "available_ram_minimum_bytes": 2 * 1024**3,
            "available_ram_min_bytes": min(
                run["available_ram_min_bytes"] for run in runs
            ),
            "load_limit_seconds": 600,
            "generation_limit_seconds": 900,
            "passed": True,
        },
        "quality_review": {
            "status": "passed",
            "review": [
                "Run 1 matches the orange cat, sunlit wood window, urban background, and realistic photo intent.",
                "Run 2 has coherent glass teapot structure, reflections, steam, and the requested shallow depth of field.",
                "Run 3 follows the requested overhead composition and places mug, book, brass pen, and hydrangea in the requested regions with clear whitespace.",
            ],
        },
        "evidence": {
            "runtime_probe": "runtime-probe.json",
            "model_download": "model-download.json",
            "modelscope_equivalence": "modelscope-equivalence.json",
            "candidate_a_load": "candidate-a-load.json",
            "candidate_b_load": "candidate-b-load.json",
            "successful_runs": [image for _, _, image in run_names],
            "retained_failures": ["candidate-a-default-attention-failed.json"],
        },
        "measurement_limitations": [
            "CUDA reserved is sampled at stage boundaries and callback boundaries.",
            "Process private commit and system available RAM are sampled every 50 ms.",
            "nvidia-smi did not expose per-process compute-apps memory under this WDDM configuration; global NVML memory and Torch allocator data are retained.",
            "WDDM shared-memory peak is unavailable from the selected tooling.",
            "Gate 0 passing does not certify the installer, native UI, cancellation, packaging, or Mac regression.",
        ],
    }
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_suffix(output.suffix + ".pending")
    temporary.write_text(
        json.dumps(result, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    os.replace(temporary, output)
    print(
        json.dumps(
            {
                "status": result["status"],
                "runs": len(runs),
                "cuda_peak": result["budget"]["cuda_peak_reserved_max_bytes"],
                "private_peak": result["budget"]["private_commit_peak_max_bytes"],
                "output": str(output),
            }
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
