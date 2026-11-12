"""Builds a v6 data root with three synthetic works for the 2026-11-12 Windows round.

The product creates works only by generating images, which this functional round
must not do. This script therefore derives a root from an existing v6 root and
inserts three `assets` rows whose file bytes are a real 2026-11-05 product output
from `verification/windows-8gb-tier/ui-qwen-turbo-768.png`. The fixture is a
constructed library used only to exercise listing, export and deletion; it is
never presented as a model result of this round.

Work 1 exercises list/favourite/preview/export/delete, work 2 the read-only
attribute, work 3 an open file handle.

usage: python make-works-fixture.py <source-v6-root> <destination-root>
"""

import hashlib
import json
import os
import shutil
import sqlite3
import sys

IMAGE = os.path.join("verification", "windows-8gb-tier", "ui-qwen-turbo-768.png")
DEFAULT_MODEL_ID = "z-image-turbo-diffusers-cuda-nf4-v1"
PROMPTS = {
    1: "验收构造作品一：雨后的上海街角",
    2: "验收构造作品二：只读属性",
    3: "验收构造作品三：占用句柄",
}


def sha256_of(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main():
    source, destination = sys.argv[1], sys.argv[2]
    if os.path.isdir(destination):
        shutil.rmtree(destination)
    shutil.copytree(source, destination, ignore=shutil.ignore_patterns("logs"))
    os.makedirs(os.path.join(destination, "logs"), exist_ok=True)

    template = os.path.join(destination, "data", "assets", "..", "template.png")
    template = os.path.abspath(template)
    shutil.copyfile(IMAGE, template)
    digest = sha256_of(template)
    size = os.path.getsize(template)

    db = sqlite3.connect(os.path.join(destination, "data", "library.sqlite3"))
    version = db.execute("PRAGMA user_version").fetchone()[0]
    if version != 6:
        raise SystemExit(f"expected a v6 database, found user_version={version}")

    conversation = db.execute(
        "SELECT id FROM conversations WHERE deleted_at IS NULL ORDER BY activity_seq DESC LIMIT 1"
    ).fetchone()[0]
    now = 1_790_000_100_000
    works = []
    for index in (1, 2, 3):
        asset_id = f"asset-accept-{index}"
        relative_path = f"assets/{asset_id}.png"
        target = os.path.join(destination, "data", relative_path)
        os.makedirs(os.path.dirname(target), exist_ok=True)
        shutil.copyfile(template, target)
        asset = {
            "asset_id": asset_id,
            "content_hash": digest,
            "data_type": "image_rgb",
            "location": {
                "kind": "managed",
                "storage_id": "library",
                "relative_path": relative_path,
            },
        }
        request = {
            "schema_version": 1,
            "request_id": f"request-accept-{index}",
            "project_id": "default",
            "capability": {
                "capability_id": "image.text2image",
                "instruction_required": True,
                "input_roles": [],
            },
            "instruction": PROMPTS[index],
            "inputs": [],
            "model_variant_id": DEFAULT_MODEL_ID,
            "requested_output": {"width": 768, "height": 768, "count": 1},
            "policy": {
                "allow_model_substitution": False,
                "allow_resolution_change": False,
                "allow_cpu_fallback": False,
            },
            "parameters": {"seed": 3148150817},
            "idempotency_key": f"accept-sub-{index}",
        }
        plan = {
            "schema_version": 1,
            "plan_id": f"plan-accept-{index}",
            "request_id": f"request-accept-{index}",
            "model_variant_id": DEFAULT_MODEL_ID,
            "components": [
                {
                    "role": "transformer",
                    "source": {"source": "acceptance-fixture", "revision": "1"},
                    "content_hash": digest,
                }
            ],
            "runtime": {
                "package_id": "runtime-acceptance",
                "engine": "diffusers",
                "source": {"source": "acceptance-fixture", "revision": "1"},
            },
            "device": {"device_id": "cuda:0", "precision": "float16"},
            "effective_output": {"width": 768, "height": 768, "count": 1},
            "effective_parameters": {"seed": 3148150817},
            "adjustments": [],
        }
        db.execute(
            "INSERT INTO jobs(id,project_id,submission_key,request_json,current_attempt_id,created_at) "
            "VALUES(?,?,?,?,?,?)",
            (
                f"job-accept-{index}",
                "default",
                f"accept-sub-{index}",
                json.dumps(request, ensure_ascii=False),
                f"attempt-accept-{index}",
                now + index,
            ),
        )
        db.execute(
            "INSERT INTO attempts(id,job_id,token,storage_key,state,plan_json,manifest_json,created_at,completed_at) "
            "VALUES(?,?,?,?,'succeeded',?,NULL,?,?)",
            (
                f"attempt-accept-{index}",
                f"job-accept-{index}",
                f"token-accept-{index}",
                f"library-accept-{index}",
                json.dumps(plan),
                now + index,
                now + index,
            ),
        )
        db.execute(
            "INSERT INTO generation_turns(id,project_id,conversation_id,sequence,submission_key,source_draft_id,"
            "prompt,model_id,aspect,count,seed,status,job_id,received_at) "
            "VALUES(?,?,?,?,?,NULL,?,?,'1:1',1,'3148150817','job',?,?)",
            (
                f"turn-accept-{index}",
                "default",
                conversation,
                index,
                f"accept-sub-{index}",
                PROMPTS[index],
                DEFAULT_MODEL_ID,
                f"job-accept-{index}",
                now + index,
            ),
        )
        db.execute(
            "INSERT INTO assets(id,job_id,attempt_id,output_role,asset_json,relative_path,width,height,byte_count,"
            "created_at,is_favorite,deletion_state) VALUES(?,?,?,'primary',?,?,768,768,?,?,0,'active')",
            (
                asset_id,
                f"job-accept-{index}",
                f"attempt-accept-{index}",
                json.dumps(asset),
                relative_path,
                size,
                now + index,
            ),
        )
        works.append({"asset": asset_id, "file": target, "prompt": PROMPTS[index]})
    os.remove(template)
    db.commit()
    db.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    summary = {
        "image_sha256": digest,
        "image_bytes": size,
        "conversation": conversation,
        "works": works,
        "assets": db.execute("select count(*) from assets").fetchone()[0],
        "user_version": db.execute("PRAGMA user_version").fetchone()[0],
    }
    db.close()
    print(json.dumps(summary, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
