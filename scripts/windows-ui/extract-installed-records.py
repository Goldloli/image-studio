import json
import sqlite3
import sys
from pathlib import Path

root = Path(sys.argv[1])
con = sqlite3.connect(root / "data/library.sqlite3")
con.row_factory = sqlite3.Row

jobs = []
for job in con.execute("select * from jobs order by created_at"):
    job_id = job["id"]
    attempt = con.execute(
        "select * from attempts where job_id=? order by created_at desc limit 1", (job_id,)
    ).fetchone()
    turn = con.execute(
        "select * from generation_turns where job_id=? order by sequence", (job_id,)
    ).fetchone()
    asset = con.execute("select * from assets where job_id=?", (job_id,)).fetchone()
    plan = json.loads(attempt["plan_json"]) if attempt and attempt["plan_json"] else None
    manifest = json.loads(attempt["manifest_json"]) if attempt and attempt["manifest_json"] else None
    plan_summary = None
    if plan:
        recipe = plan.get("recipe", {})
        parameters = plan.get("parameters", {})
        environment = plan.get("environment", {})
        plan_summary = {
            "plan_id": plan.get("plan_id"),
            "request_id": plan.get("request_id"),
            "recipe_id": recipe.get("recipe_id") or recipe.get("id"),
            "capability_id": plan.get("capability", {}).get("capability_id"),
            "model_variant_id": plan.get("model_variant", {}).get("model_variant_id")
            or plan.get("model_variant_id"),
            "parameters": parameters,
            "environment_device": environment.get("device"),
            "environment_runtime": environment.get("runtime", {}).get("package_id"),
            "components": len(environment.get("components", [])),
        }
    asset_row = None
    if asset:
        asset_json = json.loads(asset["asset_json"])
        asset_row = {
            "asset_id": asset["id"],
            "relative_path": asset["relative_path"],
            "width": asset["width"],
            "height": asset["height"],
            "byte_count": asset["byte_count"],
            "content_hash": asset_json.get("content_hash"),
            "output_role": asset["output_role"],
        }
    jobs.append(
        {
            "job_id": job_id,
            "submission_key": job["submission_key"],
            "created_at": job["created_at"],
            "prompt": turn["prompt"] if turn else None,
            "seed": turn["seed"] if turn else None,
            "aspect": turn["aspect"] if turn else None,
            "count": turn["count"] if turn else None,
            "status": turn["status"] if turn else None,
            "attempt": {
                "attempt_id": attempt["id"],
                "token": attempt["token"],
                "state": attempt["state"],
                "storage_key": attempt["storage_key"],
                "created_at": attempt["created_at"],
                "completed_at": attempt["completed_at"],
            }
            if attempt
            else None,
            "plan": plan_summary,
            "manifest_outputs": len(manifest.get("outputs", [])) if manifest else None,
            "asset": asset_row,
        }
    )

ready = json.loads((root / "data/installations/models/z-image-turbo-diffusers-cuda-nf4-v1.json").read_text(encoding="utf-8"))
document = {
    "jobs": jobs,
    "ready": {
        "runtime_lock_id": ready["runtime_lock_id"],
        "model_snapshot_id": ready["model_snapshot_id"],
        "runtime_files": len(ready["runtime_files"]),
        "runtime_manifest_sha256": ready["runtime_manifest_sha256"],
        "model_manifest_sha256": ready["model_manifest_sha256"],
        "probe": ready["probe"],
    },
}
output = json.dumps(document, ensure_ascii=False, indent=2)
if len(sys.argv) > 2:
    Path(sys.argv[2]).write_text(output, encoding="utf-8")
else:
    print(output)
