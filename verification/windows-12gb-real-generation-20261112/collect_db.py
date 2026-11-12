#!/usr/bin/env python3
"""Read-only evidence collector for the Windows real-generation acceptance round.

Reads a data root's SQLite library and produces a JSON snapshot with turns,
jobs, attempts, assets (with file hashes), drafts, conversations and ready
records. Never writes to the data root.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import sqlite3
from datetime import datetime, timezone


def sha256(path: str) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("data_root")
    parser.add_argument("output")
    args = parser.parse_args()

    root = os.path.abspath(args.data_root)
    library = os.path.join(root, "data", "library.sqlite3")
    con = sqlite3.connect(f"file:{library.replace(os.sep, '/')}?mode=ro", uri=True)
    con.row_factory = sqlite3.Row
    cur = con.cursor()

    document: dict = {
        "schema_version": 1,
        "kind": "windows-real-generation-db-snapshot",
        "generated_at": datetime.now(timezone.utc).astimezone().isoformat(),
        "data_root": root,
        "user_version": cur.execute("PRAGMA user_version").fetchone()[0],
        "quick_check": cur.execute("PRAGMA quick_check").fetchone()[0],
    }

    conversations = []
    for row in cur.execute("SELECT * FROM conversations ORDER BY created_at"):
        conversations.append({
            "id": row["id"], "title": row["title"], "title_source": row["title_source"],
            "created_at": row["created_at"], "activity_seq": row["activity_seq"],
            "deleted_at": row["deleted_at"] if "deleted_at" in row.keys() else None,
        })
    document["conversations"] = conversations

    drafts = []
    for row in cur.execute("SELECT * FROM drafts ORDER BY version"):
        keys = row.keys()
        drafts.append({
            "id": row["id"], "name": row["name"], "prompt": row["prompt"],
            "params_json": json.loads(row["params_json"]) if row["params_json"] else None,
            "collapsed": row["collapsed"], "version": row["version"],
        })
    document["drafts"] = drafts

    assets = {}
    for row in cur.execute("SELECT * FROM assets ORDER BY created_at"):
        asset_json = json.loads(row["asset_json"])
        relative = row["relative_path"]
        path = os.path.join(root, "data", relative) if relative else None
        file_info = {"exists": bool(path and os.path.exists(path))}
        if file_info["exists"]:
            file_info["file"] = path
            file_info["bytes"] = os.path.getsize(path)
            file_info["sha256"] = sha256(path)
        assets[row["id"]] = {
            "id": row["id"], "job_id": row["job_id"], "attempt_id": row["attempt_id"],
            "output_role": row["output_role"], "width": row["width"], "height": row["height"],
            "byte_count": row["byte_count"], "content_hash": asset_json.get("content_hash"),
            "data_type": asset_json.get("data_type"),
            "relative_path": relative, "created_at": row["created_at"], "file": file_info,
        }
    document["assets"] = list(assets.values())

    turns = []
    turn_rows = cur.execute("SELECT * FROM generation_turns ORDER BY conversation_id, sequence").fetchall()
    nested = con.cursor()
    for row in turn_rows:
        keys = row.keys()
        jobs = []
        if row["job_id"]:
            job = nested.execute("SELECT * FROM jobs WHERE id=?", (row["job_id"],)).fetchone()
            attempts = []
            for attempt in nested.execute("SELECT * FROM attempts WHERE job_id=? ORDER BY created_at", (row["job_id"],)).fetchall():
                plan = json.loads(attempt["plan_json"]) if attempt["plan_json"] else None
                manifest = json.loads(attempt["manifest_json"]) if attempt["manifest_json"] else None
                manifest_outputs = []
                if manifest:
                    for output in manifest.get("outputs", []):
                        manifest_outputs.append({
                            "asset_id": output.get("asset_id"),
                            "content_hash": output.get("content_hash"),
                            "data_type": output.get("data_type"),
                            "relative_path": (output.get("location") or {}).get("relative_path"),
                        })
                attempts.append({
                    "id": attempt["id"], "state": attempt["state"],
                    "created_at": attempt["created_at"], "completed_at": attempt["completed_at"],
                    "error": attempt["error"],
                    "plan_summary": {
                        "plan_id": (plan or {}).get("plan_id"),
                        "recipe_id": ((plan or {}).get("recipe") or {}).get("recipe_id"),
                        "capability_id": ((plan or {}).get("capability") or {}).get("capability_id"),
                        "model_variant_id": (plan or {}).get("model_variant_id")
                        or ((plan or {}).get("model_variant") or {}).get("model_variant_id"),
                        "parameters": (plan or {}).get("parameters"),
                        "environment_device": ((plan or {}).get("environment") or {}).get("device"),
                    } if plan else None,
                    "manifest_outputs": manifest_outputs,
                })
            job_doc = {"id": job["id"], "created_at": job["created_at"], "attempts": attempts} if job else None
            if job:
                assets_for_job = [item for item in assets.values() if item["job_id"] == job["id"]]
                job_doc["assets"] = assets_for_job
            jobs.append(job_doc)
        turns.append({
            "id": row["id"], "conversation_id": row["conversation_id"], "sequence": row["sequence"],
            "prompt": row["prompt"], "model_id": row["model_id"],
            "capability_id": row["capability_id"] if "capability_id" in keys else None,
            "aspect": row["aspect"], "count": row["count"], "seed": row["seed"],
            "source_asset_id": row["source_asset_id"] if "source_asset_id" in keys else None,
            "status": row["status"], "error": row["error"],
            "progress_stage": row["progress_stage"], "progress_step": row["progress_step"],
            "progress_total": row["progress_total"],
            "received_at": row["received_at"], "jobs": jobs,
        })
    document["turns"] = turns

    ready_records = []
    installations_dir = os.path.join(root, "data", "installations", "models")
    if os.path.isdir(installations_dir):
        for name in sorted(os.listdir(installations_dir)):
            if not name.endswith(".json"):
                continue
            path = os.path.join(installations_dir, name)
            with open(path, "r", encoding="utf-8") as handle:
                record = json.load(handle)
            ready_records.append({
                "file": name,
                "runtime_lock_id": record.get("runtime_lock_id"),
                "model_snapshot_id": record.get("model_snapshot_id"),
                "model_manifest_sha256": record.get("model_manifest_sha256"),
                "runtime_manifest_sha256": record.get("runtime_manifest_sha256"),
                "runtime_files": len(record.get("runtime_files") or []),
            })
    document["ready_records"] = ready_records

    summary: dict = {"assets": len(assets), "turns": len(turns), "conversations": len(conversations)}
    by_model: dict = {}
    for turn in turns:
        key = turn["model_id"] or "unknown"
        by_model.setdefault(key, {"turns": 0, "succeeded": 0})
        by_model[key]["turns"] += 1
        if turn["status"] == "succeeded":
            by_model[key]["succeeded"] += 1
    summary["by_model"] = by_model
    document["summary"] = summary

    con.close()
    with open(args.output, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(document, handle, ensure_ascii=False, indent=2)
    print(json.dumps(summary, ensure_ascii=False))


if __name__ == "__main__":
    main()
