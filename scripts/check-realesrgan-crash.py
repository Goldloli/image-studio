#!/usr/bin/env python3
"""Kill a live Real-ESRGAN host during Vulkan inference and verify recovery."""

from __future__ import annotations

import argparse
import fcntl
import json
import os
from pathlib import Path
import select
import signal
import sqlite3
import subprocess
import time
from typing import Any


ROOT = Path(__file__).resolve().parent.parent
HELPER = ROOT / "target/debug/examples/realesrgan-check"
RETRY = ROOT / "target/debug/examples/realesrgan-retry-check"


def args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data-root", required=True)
    parser.add_argument("--source", required=True)
    parser.add_argument(
        "--report",
        default=str(
            ROOT / "verification/planned-models-20261031/P07-realesrgan-host-crash.json"
        ),
    )
    parser.add_argument(
        "--log",
        default=str(
            ROOT / "verification/planned-models-20261031/logs/realesrgan-host-crash.log"
        ),
    )
    return parser.parse_args()


def json_line(line: str) -> dict[str, Any] | None:
    try:
        value = json.loads(line)
    except ValueError:
        return None
    return value if isinstance(value, dict) else None


def owned_processes(data_root: Path) -> list[dict[str, Any]]:
    output = subprocess.run(
        ["/bin/ps", "-axo", "pid=,ppid=,command="],
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    result = []
    for line in output.splitlines():
        if str(data_root) not in line:
            continue
        if not any(
            marker in line
            for marker in (
                "/examples/realesrgan-check",
                "/inference-supervisor",
                "/realesrgan-worker",
                "realesrgan-ncnn-vulkan",
            )
        ):
            continue
        fields = line.strip().split(None, 2)
        if len(fields) == 3:
            result.append(
                {"pid": int(fields[0]), "ppid": int(fields[1]), "command": fields[2]}
            )
    return result


def rows(data_root: Path, turn_id: str) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    connection = sqlite3.connect(
        f"file:{data_root / 'data/library.sqlite3'}?mode=ro", uri=True
    )
    connection.row_factory = sqlite3.Row
    try:
        turn = connection.execute(
            "SELECT id,status,error,job_id FROM generation_turns WHERE id=?",
            (turn_id,),
        ).fetchone()
        attempts = connection.execute(
            "SELECT a.id,a.token,a.state,a.manifest_json,a.error,"
            "(SELECT count(*) FROM assets s WHERE s.attempt_id=a.id) AS asset_count "
            "FROM attempts a JOIN jobs j ON j.id=a.job_id "
            "JOIN generation_turns t ON t.job_id=j.id "
            "WHERE t.id=? ORDER BY a.created_at,a.rowid",
            (turn_id,),
        ).fetchall()
        if turn is None:
            raise RuntimeError("Turn disappeared after crash")
        return dict(turn), [dict(item) for item in attempts]
    finally:
        connection.close()


def main() -> int:
    options = args()
    data_root = Path(options.data_root).expanduser().resolve()
    source = Path(options.source).expanduser().resolve()
    report = Path(options.report).expanduser().resolve()
    log_path = Path(options.log).expanduser().resolve()
    if not HELPER.is_file() or not RETRY.is_file():
        raise RuntimeError("acceptance helpers are missing")
    if not source.is_file():
        raise RuntimeError("--source does not exist")
    report.parent.mkdir(parents=True, exist_ok=True)
    log_path.parent.mkdir(parents=True, exist_ok=True)
    export = report.with_suffix(".png")
    command = [
        str(HELPER),
        "--data-dir",
        str(data_root),
        "--source",
        str(source),
        "--export-dir",
        str(report.parent),
    ]
    process = subprocess.Popen(
        command,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        start_new_session=True,
    )
    if process.stdout is None:
        raise RuntimeError("failed to capture helper output")
    started = time.monotonic()
    turn_id = None
    observed = None
    try:
        with log_path.open("w") as log:
            deadline = time.monotonic() + 300
            while time.monotonic() < deadline and process.poll() is None:
                readable, _, _ = select.select([process.stdout], [], [], 0.5)
                if not readable:
                    continue
                line = process.stdout.readline()
                if not line:
                    break
                log.write(line)
                log.flush()
                value = json_line(line)
                if not value:
                    continue
                if isinstance(value.get("turnId"), str):
                    turn_id = value["turnId"]
                if value.get("stage") == "inference" and turn_id:
                    observed = value
                    os.kill(process.pid, signal.SIGKILL)
                    log.write(
                        json.dumps(
                            {
                                "acceptanceControl": {
                                    "action": "SIGKILL",
                                    "pid": process.pid,
                                    "stage": "inference",
                                    "turnId": turn_id,
                                }
                            }
                        )
                        + "\n"
                    )
                    log.flush()
                    break
        if observed is None or turn_id is None:
            raise RuntimeError("Real-ESRGAN inference was not observed before SIGKILL")
        returncode = process.wait(timeout=15)
        lease_path = data_root / "data/execution.lock"
        lease_started = time.monotonic()
        lease_busy = False
        with lease_path.open("a+b") as lease:
            deadline = time.monotonic() + 20
            while True:
                try:
                    fcntl.flock(lease.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                    fcntl.flock(lease.fileno(), fcntl.LOCK_UN)
                    break
                except BlockingIOError:
                    lease_busy = True
                    if time.monotonic() > deadline:
                        raise RuntimeError("execution lease was not released")
                    time.sleep(0.05)
        lease_seconds = time.monotonic() - lease_started
        lingering = owned_processes(data_root)
        if lingering:
            raise RuntimeError(f"owned process tree remains: {lingering}")
        recovery = subprocess.run(
            [
                str(RETRY),
                "--data-dir",
                str(data_root),
                "--snapshot-only",
            ],
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
        if recovery.returncode != 0:
            raise RuntimeError(
                "recovery open failed: "
                + (recovery.stderr.strip() or recovery.stdout[-2048:])
            )
        _, attempts = rows(data_root, turn_id)
        if len(attempts) != 1 or attempts[0]["state"] != "interrupted":
            raise RuntimeError("killed host did not leave one Interrupted Attempt")
        manifest = json.loads(attempts[0]["manifest_json"])
        if manifest.get("outputs") != []:
            raise RuntimeError("Interrupted Attempt retained outputs")
        if manifest.get("final_result", {}).get("status") != "interrupted":
            raise RuntimeError("Interrupted record has the wrong terminal state")
        recovered_snapshot = None
        for line in reversed(recovery.stdout.splitlines()):
            value = json_line(line)
            if value and isinstance(value.get("snapshot"), dict):
                recovered_snapshot = value["snapshot"]
                break
        if recovered_snapshot is None:
            raise RuntimeError("recovery helper returned no product snapshot")
        recovered_turn = next(
            (
                item
                for item in recovered_snapshot.get("turns", [])
                if item.get("id") == turn_id
            ),
            None,
        )
        if (
            recovered_turn is None
            or recovered_turn.get("state") != "Interrupted"
            or recovered_turn.get("assetId")
        ):
            raise RuntimeError("recovered Turn is not Interrupted without an asset")
        retry = subprocess.run(
            [
                str(RETRY),
                "--data-dir",
                str(data_root),
                "--job-id",
                manifest["job_id"],
                "--export",
                str(export),
            ],
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=900,
            check=False,
        )
        (log_path.parent / "realesrgan-host-crash-retry.log").write_text(
            retry.stdout + retry.stderr
        )
        if retry.returncode != 0:
            raise RuntimeError(
                "explicit retry failed: "
                + (retry.stderr.strip() or retry.stdout[-2048:])
            )
        _, final_attempts = rows(data_root, turn_id)
        if len(final_attempts) != 2:
            raise RuntimeError("retry did not create exactly one new Attempt")
        if final_attempts[0]["state"] != "interrupted":
            raise RuntimeError("original Attempt lost its Interrupted state")
        if final_attempts[1]["state"] != "succeeded":
            raise RuntimeError("retry Attempt did not succeed")
        if final_attempts[1]["asset_count"] != 1:
            raise RuntimeError("retry did not atomically register one output asset")
        if len({item["token"] for item in final_attempts}) != 2:
            raise RuntimeError("retry reused the execution token")
        if not export.is_file():
            raise RuntimeError("retry did not export its primary output")
        result = {
            "schemaVersion": 1,
            "runId": "planned-models-20261031",
            "caseId": "P07",
            "configurationKey": "R-U",
            "required": True,
            "status": "passed",
            "evidenceKind": "product_execution",
            "reason": "Host SIGKILL during real ncnn Vulkan inference left the supervisor to reclaim the wrapper and ncnn tree, persisted Interrupted without output, and an explicit new-token retry committed one primary PNG.",
            "command": command,
            "hostReturncode": returncode,
            "observedExecution": observed,
            "turnId": turn_id,
            "jobId": manifest["job_id"],
            "executionLeaseWasBusyDuringRecovery": lease_busy,
            "executionLeaseReleaseSeconds": lease_seconds,
            "lingeringOwnedProcesses": lingering,
            "recoveredTurn": recovered_turn,
            "attempts": [
                {
                    "id": item["id"],
                    "token": item["token"],
                    "state": item["state"],
                    "assetCount": item["asset_count"],
                    "error": item["error"],
                }
                for item in final_attempts
            ],
            "retryExport": str(export),
            "log": str(log_path),
            "elapsedSeconds": time.monotonic() - started,
        }
        temporary = report.with_name(f".{report.name}.tmp")
        temporary.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
        temporary.replace(report)
        print(report)
        return 0
    finally:
        if process.poll() is None:
            process.kill()
            process.wait(timeout=10)


if __name__ == "__main__":
    raise SystemExit(main())
