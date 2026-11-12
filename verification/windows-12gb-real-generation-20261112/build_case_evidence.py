#!/usr/bin/env python3
"""Build per-case evidence for the Windows real-generation acceptance round.

Combines the read-only database snapshot, the per-case driver results (from the
UI run), the resource-sampler CSV, and the case configuration. Writes one JSON
per case plus a combined file. ASCII-only console output so Windows consoles do
not need a CJK codepage.
"""
from __future__ import annotations

import csv
import json
import os
import re

EVIDENCE = os.path.dirname(os.path.abspath(__file__))


def load(path: str):
    with open(path, "r", encoding="utf-8") as handle:
        return json.load(handle)


def sample_summary(path: str) -> dict:
    if not os.path.exists(path):
        return {"rows": 0, "error": "missing samples"}
    rows = []
    with open(path, "r", encoding="utf-8", errors="replace") as handle:
        reader = csv.DictReader(handle)
        for row in reader:
            try:
                rows.append(row)
            except Exception:
                continue
    if not rows:
        return {"rows": 0}
    def numbers(key):
        values = []
        for row in rows:
            try:
                values.append(float(row[key]))
            except (KeyError, TypeError, ValueError):
                pass
        return values
    gpu = numbers("gpu_used_mib")
    commit = numbers("private_commit_mib")
    avphys = numbers("available_physical_mib")
    gpu_total = numbers("gpu_total_mib")
    def stat(values):
        return {"min": min(values), "max": max(values), "mean": round(sum(values) / len(values), 2)} if values else None
    return {
        "rows": len(rows),
        "interval_seconds": 1,
        "gpu_used_mib": stat(gpu),
        "gpu_total_mib": stat(gpu_total),
        "private_commit_mib": stat(commit),
        "available_physical_mib": stat(avphys),
    }


def main() -> None:
    cases_doc = load(os.path.join(EVIDENCE, "generation-cases.json"))
    cases = cases_doc["cases"]
    db = load(os.path.join(EVIDENCE, "db-fresh-final.json"))
    turns = db["turns"]
    assets = {asset["id"]: asset for asset in db["assets"]}
    results_dir = os.path.join(EVIDENCE, "generations")
    samples_dir = os.path.join(EVIDENCE, "samples")

    def find_turn(case: dict):
        expected = case.get("prompt") or ""
        match = re.search(r"\[([A-Za-z0-9]+)\]\s*$", expected)
        tag = match.group(0) if match else None
        model_key = None
        if case["id"] == "G07":
            model_key = "birefnet"
        elif case["id"] == "G08":
            model_key = "realesrgan"
        candidates = []
        for turn in turns:
            if tag is not None:
                if tag not in (turn["prompt"] or ""):
                    continue
            elif model_key is not None:
                if model_key not in (turn["model_id"] or ""):
                    continue
            else:
                continue
            job = turn["jobs"][0] if turn["jobs"] else None
            attempt = job["attempts"][0] if job and job["attempts"] else None
            candidates.append((turn["sequence"], turn, attempt))
        if not candidates:
            return None, None
        candidates.sort(key=lambda item: item[0], reverse=True)
        return candidates[0][1], candidates[0][2]

    combined = []
    for case in cases:
        turn, attempt = find_turn(case)
        driver = None
        driver_path = os.path.join(results_dir, f"{case['id']}.json")
        if os.path.exists(driver_path):
            driver = load(driver_path)
        samples = sample_summary(os.path.join(samples_dir, f"{case['id']}.csv"))
        plan = (attempt or {}).get("plan_summary") if attempt else None
        outputs = []
        if attempt:
            for manifest_output in attempt.get("manifest_outputs", []):
                asset = assets.get(manifest_output.get("asset_id")) or {}
                outputs.append({
                    "asset_id": manifest_output.get("asset_id"),
                    "content_hash": manifest_output.get("content_hash"),
                    "data_type": manifest_output.get("data_type"),
                    "relative_path": manifest_output.get("relative_path"),
                    "width": asset.get("width"),
                    "height": asset.get("height"),
                    "byte_count": asset.get("byte_count"),
                    "file_bytes": (asset.get("file") or {}).get("bytes"),
                    "file_sha256": (asset.get("file") or {}).get("sha256"),
                    "output_role": asset.get("output_role"),
                    "file_exists": (asset.get("file") or {}).get("exists"),
                })
        expected_prompt = case.get("prompt") or ""
        actual_prompt = (turn or {}).get("prompt") or ""
        prompt_ok = actual_prompt == expected_prompt if expected_prompt else actual_prompt == ""
        elapsed = (attempt or {}).get("completed_at") - (attempt or {}).get("created_at") if attempt else None
        budget = {
            "gpu_baseline_mib": 1400,
            "gpu_peak_minus_baseline_mib": (samples.get("gpu_used_mib") or {}).get("max", 0) - 1400 if samples.get("gpu_used_mib") else None,
            "gpu_used_limit_mib": int(10.5 * 1024),
            "private_commit_limit_mib": 24 * 1024,
            "available_physical_min_mib": 2048,
        }
        entry = {
            "schema_version": 1,
            "kind": "windows-real-generation-case",
            "case_id": case["id"],
            "model": case["model"],
            "capability_kind": case["kind"],
            "prompt": expected_prompt,
            "source_tag": case.get("source"),
            "data_root": "C:/ist-rg/r-fresh",
            "sequence": (turn or {}).get("sequence"),
            "model_id": (turn or {}).get("model_id"),
            "capability_id": (turn or {}).get("capability_id"),
            "attempt_id": (attempt or {}).get("id"),
            "attempt_state": (attempt or {}).get("state"),
            "attempt_error": (attempt or {}).get("error"),
            "prompt_matches_expected": prompt_ok,
            "plan": plan,
            "elapsed_ms_from_db_attempt": elapsed,
            "driver": {
                "status": (driver or {}).get("status"),
                "elapsed_seconds": (driver or {}).get("elapsedSeconds"),
                "stages": (driver or {}).get("stages"),
                "turn_text": (driver or {}).get("turnText"),
                "error": (driver or {}).get("error"),
            },
            "outputs": outputs,
            "resources": samples,
            "budget": budget,
        }
        combined.append(entry)
        out_path = os.path.join(results_dir, f"{case['id']}-evidence.json")
        with open(out_path, "w", encoding="utf-8", newline="\n") as handle:
            json.dump(entry, handle, ensure_ascii=False, indent=2)
        print(case["id"], "state=", entry["attempt_state"], "prompt_ok=", prompt_ok, "outputs=", len(outputs))

    with open(os.path.join(results_dir, "generation-cases-evidence.json"), "w", encoding="utf-8", newline="\n") as handle:
        json.dump(combined, handle, ensure_ascii=False, indent=2)
    print("cases:", len(combined))


if __name__ == "__main__":
    main()
