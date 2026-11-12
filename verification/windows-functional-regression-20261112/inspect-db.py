"""Read-only snapshot of an Image Studio data root for the 2026-11-12 Windows round.

Copies <root>/data into a temporary directory before opening SQLite so the live
application never has to be stopped, and prints a JSON summary of the rows this
round inspects. Never modifies the source root.

usage: python inspect-db.py <data-root> [<out.json>]
"""

import json
import os
import shutil
import sqlite3
import sys
import tempfile


def column_names(connection, table):
    return [row[1] for row in connection.execute(f'PRAGMA table_info("{table}")')]


def dump(root):
    source = os.path.join(root, "data")
    staging = tempfile.mkdtemp(prefix="ist-db-")
    copy = os.path.join(staging, "data")
    try:
        shutil.copytree(source, copy, ignore_dangling_symlinks=True)
        db = sqlite3.connect(os.path.join(copy, "library.sqlite3"))
        db.row_factory = sqlite3.Row
        summary = {
            "data_root": root,
            "user_version": db.execute("PRAGMA user_version").fetchone()[0],
            "quick_check": db.execute("PRAGMA quick_check(1)").fetchone()[0],
            "tables": [r[0] for r in db.execute(
                "select name from sqlite_master where type='table' and name not like 'sqlite_%' order by name")],
            "conversations": [dict(r) for r in db.execute(
                "select id,title,title_source,deleted_at,activity_seq from conversations order by activity_seq desc")],
            "drafts": [dict(r) for r in db.execute(
                "select id,project_id,name,substr(prompt,1,80) as prompt,version,collapsed from drafts order by id")],
            "assets": [dict(r) for r in db.execute(
                "select id,relative_path,width,height,byte_count,is_favorite,deletion_state,deleted_at,deletion_error from assets order by created_at desc")],
            "jobs": db.execute("select count(*) from jobs").fetchone()[0],
            "attempts": db.execute("select count(*) from attempts").fetchone()[0],
            "generation_turns": db.execute("select count(*) from generation_turns").fetchone()[0],
            "projects": [dict(r) for r in db.execute(
                "select id,active_draft_id,active_conversation_id from projects")],
        }
        for table in ("conversations", "assets"):
            summary[f"{table}_columns"] = column_names(db, table)
        db.close()
        return summary
    finally:
        shutil.rmtree(staging, ignore_errors=True)


def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    root = sys.argv[1]
    text = json.dumps(dump(root), ensure_ascii=False, indent=2)
    if len(sys.argv) > 2:
        with open(sys.argv[2], "w", encoding="utf-8") as out:
            out.write(text + "\n")
        print(f"wrote {sys.argv[2]}")
    else:
        print(text)


if __name__ == "__main__":
    main()
