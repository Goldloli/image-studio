"""Builds a v5 data root for the v5 -> v6 upgrade check (2026-11-12 Windows round).

Copies an existing v5 data root, then adds three extra conversations and drafts
through the *v5* schema so the upgrade has multiple sessions to preserve. The
source root is never modified; the destination root is recreated.

usage: python make-v5-fixture.py <source-root> <destination-root>
"""

import json
import os
import shutil
import sqlite3
import sys


def main():
    source, destination = sys.argv[1], sys.argv[2]
    if os.path.abspath(source).lower() == os.path.abspath(destination).lower():
        raise SystemExit("source and destination must differ")
    if os.path.isdir(destination):
        shutil.rmtree(destination)
    os.makedirs(destination)
    shutil.copytree(os.path.join(source, "data"), os.path.join(destination, "data"))

    db = sqlite3.connect(os.path.join(destination, "data", "library.sqlite3"))
    version = db.execute("PRAGMA user_version").fetchone()[0]
    if version != 5:
        raise SystemExit(f"expected a v5 database, found user_version={version}")
    params = json.dumps(
        {"modelId": "z-image-turbo-diffusers-cuda-nf4-v1", "aspect": "1:1", "count": 1, "seed": ""}
    )
    created = []
    for index, title in enumerate(("验收-v5-会话1", "验收-v5-会话2", "验收-v5-会话3"), start=1):
        draft = db.execute(
            "INSERT INTO drafts(project_id,name,prompt,prompt_source,params_json,collapsed,version) "
            "VALUES('default',?,?,NULL,?,0,1)",
            (title, f"v5 升级前草稿 {index}", params),
        ).lastrowid
        conversation = f"conv-v5-accept-{index}"
        db.execute(
            "INSERT INTO conversations(id,project_id,draft_id,title,title_source,created_at,activity_seq) "
            "VALUES(?,?,?,?,'manual',?,?)",
            (conversation, "default", draft, title, 1_790_000_000_000 + index, 10 + index),
        )
        created.append({"conversation": conversation, "draft": draft, "title": title})
    db.commit()
    db.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    summary = {
        "user_version": db.execute("PRAGMA user_version").fetchone()[0],
        "conversations": db.execute("select count(*) from conversations").fetchone()[0],
        "drafts": db.execute("select count(*) from drafts").fetchone()[0],
        "assets": db.execute("select count(*) from assets").fetchone()[0],
        "added": created,
    }
    db.close()
    print(json.dumps(summary, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
