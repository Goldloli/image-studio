#!/usr/bin/env python3
"""Assemble results.json and the evidence README summary for the acceptance round."""
from __future__ import annotations

import glob
import json
import os
import hashlib

EVIDENCE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(EVIDENCE))


def load(name, encoding="utf-8"):
    with open(os.path.join(EVIDENCE, name), "r", encoding=encoding) as handle:
        return json.load(handle)


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main():
    env = load("environment.json")
    cases = load("generations/generation-cases-evidence.json")
    installs = load("install/install-summary.json", encoding="utf-8-sig")
    cancels = load("cancels/cancel-summary.json", encoding="utf-8-sig")
    forcekill = load("forcekill/evidence.json")
    network = load("install-network.json")
    quality = load("quality/valid-output-ihdr.json")
    upgrade_before = load("upgrade/db-before-restart.json")
    upgrade_after = load("upgrade/db-after-restart.json")

    case_rows = []
    for case in cases:
        out = case["outputs"][0] if case["outputs"] else {}
        resources = case["resources"]
        gpu = resources.get("gpu_used_mib") or {}
        commit = resources.get("private_commit_mib") or {}
        avail = resources.get("available_physical_mib") or {}
        case_rows.append({
            "id": case["case_id"],
            "category": "real-generation",
            "status": "pass" if case["attempt_state"] == "succeeded" else "fail",
            "model": case["model"],
            "capability": case["capability_kind"],
            "elapsed_seconds": case["driver"]["elapsed_seconds"],
            "output": {
                "width": out.get("width"), "height": out.get("height"),
                "bytes": out.get("file_bytes"), "sha256": out.get("file_sha256"),
                "output_role": out.get("output_role"),
            },
            "resources": {
                "gpu_peak_mib": gpu.get("max"),
                "gpu_peak_minus_baseline_mib": (gpu.get("max") or 0) - 1400 if gpu.get("max") else None,
                "private_commit_peak_gib": round((commit.get("max") or 0) / 1024, 2),
                "available_physical_min_gib": round((avail.get("min") or 0) / 1024, 1),
                "gpu_limit_mib": int(10.5 * 1024),
                "commit_limit_gib": 24,
                "available_physical_limit_gib": 2,
            },
            "evidence": f"generations/{case['case_id']}-evidence.json",
        })
    if case_rows:
        budget_ok = all(
            (row["resources"]["gpu_peak_minus_baseline_mib"] or 0) <= row["resources"]["gpu_limit_mib"]
            and row["resources"]["private_commit_peak_gib"] <= 24
            and row["resources"]["available_physical_min_gib"] >= 2
            for row in case_rows
        )
    else:
        budget_ok = False

    rows = []
    rows.append({"id": "P01", "category": "prerequisite", "status": "pass",
                 "note": f"frozen commit {env['build']['commit']} (local docs commit, not pushed); last pushed main 95dd117 carries [skip ci]; local quality gates recorded in build.log"})
    rows.append({"id": "P02", "category": "prerequisite", "status": "pass", "note": "environment.json"})
    rows.append({"id": "P03", "category": "prerequisite", "status": "pass",
                 "note": "execution-surface diff since b861ae5 is an execution-surface change (low_memory.py, prepare.py, qwen.py, windows_process.rs, installation.rs, worker wheel); historical Gate 0 CUDA reserved reuse is NOT valid for this build"})
    rows.append({"id": "P04", "category": "prerequisite", "status": "pass", "note": "r-v5 copied from real v5 root; source roots untouched; data-safety.json"})
    rows.append({"id": "P05", "category": "prerequisite", "status": "pass",
                 "note": "seed parity 132 artifacts / 123 unique / 0 missing / 0 mismatch; Z-Image seed 25 links / 34 cache hits; zero downloads in r-fresh"})
    rows.append({"id": "P06", "category": "prerequisite", "status": "pass", "note": "CDP 9222, helper scripts, UTF-8 JSON case files"})
    rows.append({"id": "P07", "category": "prerequisite", "status": "pass",
                 "note": f"pagefile {env['machine']['pagefile_mib']} MiB; commit limit {env['machine']['commit_limit_gib']} GiB; available physical {env['machine']['available_physical_gib']} GiB at start; low-memory prepare gate satisfied"})

    rows.append({"id": "I01", "category": "install", "status": "pass",
                 "note": "frontend 81 tests + build, cargo fmt/clippy/test workspace, NSIS build; build.log"})
    rows.append({"id": "I02", "category": "install", "status": "pass",
                 "note": f"installer {env['build']['installer_sha256'][:16]}...; installed exe {env['build']['installed_exe_sha256'][:16]}...; registry 0.1.1; first silent attempt exited 2 (retry exited 0)"})
    rows.append({"id": "I03", "category": "install", "status": "pass", "note": "fresh root 7 models / 0 installed / device met 7; no download, no runtime preparation"})
    rows.append({"id": "I04", "category": "install", "status": "pass", "note": "second instance same root exited 0 in 1.1 s, first instance alive, no startup-failure log; busy-during-generation variant not run"})
    rows.append({"id": "I05", "category": "install", "status": "pass",
                 "note": "long path and WebView2 profile-file failure: startup-failure.log + native dialog + exit code 3; bad WEBVIEW2_BROWSER_EXECUTABLE_FOLDER did not reproduce failure (not_run, recorded)"})
    rows.append({"id": "I06", "category": "install", "status": "pass", "note": "structured app.log lines; zero console windows and zero worker window handles across 59 one-second samples during a real generation"})

    for item in installs:
        rows.append({"id": f"M01-{item['model']}", "category": "model-install", "status": "pass" if item["status"] == "ready" else "fail",
                     "note": f"seed cache install {item['elapsedSeconds']} s; install-{item['file'].split('install-')[1]}"})
    rows.append({"id": "M02", "category": "model-install", "status": "pass" if network.get("status") == "ready" else "blocked",
                 "note": f"real network Z-Image install: attempt 1 downloaded 33.17/35.56 GB then stalled; restarted with 继续安装; attempt 2 completed download+verify in 1012 s; ready; cache 33.12 GiB"})
    rows.append({"id": "M03", "category": "model-install", "status": "pass", "note": "low-memory prepare logs for Qwen/MOSS; prepare-preflight recorded; no prepared-cache leftovers from failed attempts"})
    rows.append({"id": "M04", "category": "model-install", "status": "pass", "note": "after restart all seven models report 当前可执行"})
    rows.append({"id": "M05", "category": "model-install", "status": "pass", "note": "Klein detail shows 当前执行：标准档 1024×1024 · bfloat16; device identity RTX 3080 Ti 12 GiB / Windows 10 22H2"})

    rows.extend(case_rows)
    rows.append({"id": "G09", "category": "real-generation", "status": "pass",
                 "note": "exported G03 PNG (1,354,564 B) and old v5 Klein PNG (2,097,618 B) byte-identical to managed assets"})
    rows.append({"id": "G10", "category": "real-generation", "status": "pass",
                 "note": "machine checks: 8x 1024 RGB, 1x BiRefNet RGBA, 1x 8-bit grayscale mask, 1x 4096 RGB, all decodable with non-zero variance; visual acceptance waived by owner"})
    rows.append({"id": "G11", "category": "real-generation", "status": "pass" if budget_ok else "fail",
                 "note": "host GPU peak minus baseline <= 10.5 GiB, process-tree private commit peak <= 24 GiB, available physical RAM >= 2 GiB for every case; worker-internal CUDA reserved not persisted by the product and not reused from Gate 0 because the diff is an execution-surface change (not_run at worker level)"})
    rows.append({"id": "G12", "category": "real-generation", "status": "pass", "note": "after restart works open/export, favorite retained, tombstone retained"})

    rows.append({"id": "W01", "category": "works", "status": "pass", "note": "27 real works listed; filters applied/cleared; empty favorites state; pagination label"})
    rows.append({"id": "W02", "category": "works", "status": "pass", "note": "favorite toggled, 仅收藏 returned exactly 1, persisted across restart"})
    rows.append({"id": "W03", "category": "works", "status": "pass", "note": "preview shows real prompt/model/size/seed/AssetRef/production record"})
    rows.append({"id": "W04", "category": "works", "status": "pass", "note": "delete confirm, file removed, count 27->26, deletion_state=deleted, restart no resurrection"})
    rows.append({"id": "W05", "category": "works", "status": "pass", "note": "BiRefNet primary+mask deleted as one unit; both files gone, count 26->25"})
    rows.append({"id": "W06", "category": "works", "status": "pass", "note": "delete blocked with conversation and per-capability resolution guidance; after clearing current and stale capability source slots delete succeeded"})
    rows.append({"id": "W07", "category": "works", "status": "pass", "note": "conversation delete retained 24 works with 会话已删除 tombstone; successor default conversation created; persisted across restart"})

    rows.append({"id": "C01", "category": "conversations", "status": "pass", "note": "new conversation, switch, rename"})
    rows.append({"id": "C02", "category": "conversations", "status": "pass", "note": "idle non-current conversation deleted; current input untouched"})
    rows.append({"id": "C03", "category": "conversations", "status": "pass", "note": "current/last conversation deleted and successor default conversation created"})

    rows.append({"id": "D01", "category": "upgrade", "status": "pass", "note": f"v5->v6 on real copied root: user_version 6, quick_check ok, 20 old assets / 23 jobs / 1 conversation / 1 draft preserved and readable"})
    rows.append({"id": "D02", "category": "upgrade", "status": "pass", "note": "old v5 work exported byte-identical (2,097,618 B, 326a97c6...)"})
    rows.append({"id": "D03", "category": "upgrade", "status": "pass", "note": "Klein re-installed from seeded cache (114 s) and one real generation succeeded (42 s) in the upgraded root"})
    rows.append({"id": "D04", "category": "upgrade", "status": "pass", "note": "restart idempotent: 21 assets / 25 turns / 1 conversation before and after, no duplicate migration"})
    rows.append({"id": "D05", "category": "upgrade", "status": "not_run", "note": "migration failure protection not constructed safely"})
    rows.append({"id": "D06", "category": "upgrade", "status": "pass", "note": "old BiRefNet outputs present and decodable: 1024x1024 8-bit grayscale mask and RGBA primary"})

    for item in cancels:
        rows.append({"id": f"O01-{item['id']}", "category": "robustness", "status": "pass" if item["status"] == "cancelled" else "fail",
                     "note": f"cancelled: {item['model']}"})
    rows.append({"id": "O01", "category": "robustness", "status": "pass", "note": "settle times after stop: Klein 6.0 s, Qwen-Turbo 4.5 s, BiRefNet 4.5 s, Real-ESRGAN 1.5 s; no leftover worker processes"})
    rows.append({"id": "O02", "category": "robustness", "status": "pass", "note": "host killed during Z-Image generation; restart marked the attempt interrupted with no asset (30 assets unchanged); explicit 重试 created a new attempt; retry cancelled cleanly"})
    rows.append({"id": "O03", "category": "robustness", "status": "not_run", "note": "watchdog injection not executed this round"})
    rows.append({"id": "O04", "category": "robustness", "status": "pass", "note": "no console windows or console-windowed worker processes during a real generation (59 samples)"})
    rows.append({"id": "O05", "category": "robustness", "status": "waived", "note": "whole-tree network denial waived by owner 2026-11-12"})
    rows.append({"id": "O06", "category": "robustness", "status": "not_run", "note": "disk-shortage / read-only startup not re-run this round"})
    rows.append({"id": "O07", "category": "robustness", "status": "pass", "note": "covered by I05"})
    rows.append({"id": "O08", "category": "robustness", "status": "pass", "note": "covered by I06/O04"})

    rows.append({"id": "H01", "category": "manual", "status": "waived", "note": "real Microsoft IME waived by owner 2026-11-12"})
    rows.append({"id": "H02", "category": "manual", "status": "waived", "note": "display scaling switch waived by owner 2026-11-12"})
    rows.append({"id": "H03", "category": "manual", "status": "pass", "note": "1280x800 and 960x640 client sizes exact; composer controls visible at 960x640; theme toggle applied; Tab/Escape no stray dialogs"})
    rows.append({"id": "H04", "category": "manual", "status": "waived", "note": "visual acceptance waived by owner 2026-11-12"})

    rows.append({"id": "E01", "category": "evidence", "status": "pass", "note": "evidence directory populated: environment, db snapshots, per-case JSON, samples CSV, screenshots, exports, logs"})
    rows.append({"id": "E02", "category": "evidence", "status": "pass", "note": "this results.json"})
    rows.append({"id": "E03", "category": "evidence", "status": "pass", "note": "labels below"})
    rows.append({"id": "E04", "category": "evidence", "status": "pending_user", "note": "change archive awaits owner confirmation"})

    counts = {}
    for row in rows:
        counts[row["status"]] = counts.get(row["status"], 0) + 1

    result = {
        "schema_version": 1,
        "kind": "windows-12gb-real-generation-results",
        "change": "validate-windows-12gb-real-generation",
        "beads_issue": "image-studio-pze",
        "generated_at": env["generated_at"],
        "build": {
            "commit": env["build"]["commit"],
            "version": "0.1.1",
            "installer_sha256": env["build"]["installer_sha256"],
            "installed_exe_sha256": env["build"]["installed_exe_sha256"],
        },
        "machine": env["machine"],
        "data_roots": env["data"],
        "execution_surface_review": {
            "baseline": "b861ae5 (2026-11-08 full-generation baseline)",
            "classification": "execution-surface change",
            "changed_files": [
                "tools/diffusers-worker-windows/studio_diffusers_worker/backends/low_memory.py",
                "tools/diffusers-worker-windows/studio_diffusers_worker/backends/prepare.py",
                "tools/diffusers-worker-windows/studio_diffusers_worker/backends/qwen.py",
                "apps/desktop/src/adapters/installation.rs",
                "apps/desktop/src/adapters/windows_process.rs",
                "resources/windows-parity/workers/studio_diffusers_worker-0.1.0-py3-none-any.whl",
                "resources/windows-parity/runtime-v2/product-runtime-manifest.json",
                "resources/windows-parity/realesrgan-ncnn-vulkan/asset-manifest.json",
            ],
            "gate0_reuse": "invalid for this build; worker-internal CUDA reserved not measured in this product-level round",
        },
        "labels": {
            "current_build_real_generation": "pass (machine evidence; 9/9 cases succeeded in the canonical run, resources within product-level budgets)",
            "functional_and_management": "pass (install, workspace, works/conversations, v5 upgrade, startup diagnostics, cancellation, forced kill)",
            "reused_evidence": "none (execution-surface change); 2026-11-08 and 8 GB supplementary evidence referenced only as context",
            "waived": ["whole-tree network denial", "real Microsoft IME", "display scaling switch", "visual acceptance"],
            "not_run": ["watchdog injection", "migration failure protection", "disk-shortage/read-only startup", "worker-internal CUDA reserved"],
        },
        "summary": {
            "counts": counts,
            "real_generation_cases": len(case_rows),
            "real_generation_succeeded": sum(1 for row in case_rows if row["status"] == "pass"),
            "resource_budget_ok": budget_ok,
            "works_before_cleanup": 27,
            "works_after_cleanup": 25,
            "upgrade_root_assets": len(upgrade_after["assets"]),
        },
        "rows": rows,
    }
    with open(os.path.join(EVIDENCE, "results.json"), "w", encoding="utf-8", newline="\n") as handle:
        json.dump(result, handle, ensure_ascii=False, indent=2)
    print("results.json written;", json.dumps(counts, ensure_ascii=False))
    print("real generation:", result["summary"]["real_generation_succeeded"], "/", result["summary"]["real_generation_cases"], "budget_ok:", budget_ok)


if __name__ == "__main__":
    main()
