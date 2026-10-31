#!/usr/bin/env python3
"""Kill a live Klein acceptance host after real sampling and verify recovery."""

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
DEFAULT_DATA_ROOT = Path.home() / "Library/Application Support/ImageStudio"
HELPER = ROOT / "target/debug/examples/klein-check"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data-root", required=True)
    parser.add_argument("--prompt", required=True)
    parser.add_argument("--source")
    parser.add_argument("--seed", default="42")
    parser.add_argument("--report")
    parser.add_argument("--log")
    return parser.parse_args()


def parse_json_line(line: str) -> dict[str, Any] | None:
    try:
        value = json.loads(line)
    except ValueError:
        return None
    return value if isinstance(value, dict) else None


def snapshot_from_output(output: str) -> dict[str, Any]:
    for line in reversed(output.splitlines()):
        value = parse_json_line(line)
        if value is None:
            continue
        snapshot = value.get("snapshot", value)
        if isinstance(snapshot, dict) and isinstance(snapshot.get("jobs"), list):
            return snapshot
    raise RuntimeError("helper output contained no product snapshot")


def attempts(data_root: Path, job_id: str) -> list[dict[str, Any]]:
    connection = sqlite3.connect(
        f"file:{data_root / 'data/library.sqlite3'}?mode=ro", uri=True
    )
    connection.row_factory = sqlite3.Row
    try:
        return [
            dict(row)
            for row in connection.execute(
                "SELECT a.id,a.token,a.state,a.manifest_json,a.error,"
                "(SELECT count(*) FROM assets s WHERE s.attempt_id=a.id) AS asset_count "
                "FROM attempts a WHERE a.job_id=? ORDER BY a.created_at,a.rowid",
                (job_id,),
            )
        ]
    finally:
        connection.close()


def processes_mentioning(data_root: Path) -> list[dict[str, Any]]:
    output = subprocess.run(
        ["/bin/ps", "-axo", "pid=,ppid=,command="],
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    needle = str(data_root)
    matches = []
    for line in output.splitlines():
        owned_process = any(
            marker in line
            for marker in (
                "/examples/klein-check",
                "/inference-supervisor",
                "studio-mflux-worker-v2",
                "studio_mflux_worker_v2",
            )
        )
        if owned_process and needle in line:
            fields = line.strip().split(None, 2)
            if len(fields) == 3:
                matches.append(
                    {"pid": int(fields[0]), "ppid": int(fields[1]), "command": fields[2]}
                )
    return matches


def main() -> int:
    args = parse_args()
    data_root = Path(args.data_root).expanduser().resolve()
    default = DEFAULT_DATA_ROOT.resolve()
    if (
        data_root == default
        or default in data_root.parents
        or data_root in default.parents
    ):
        raise RuntimeError("--data-root must be isolated from the default ImageStudio root")
    if not HELPER.is_file():
        raise RuntimeError("Klein acceptance helper is missing")
    source = Path(args.source).expanduser().resolve() if args.source else None
    if source is not None and not source.is_file():
        raise RuntimeError("--source does not exist")

    report = (
        Path(args.report).expanduser().resolve()
        if args.report
        else ROOT / "verification/planned-models-20261031/klein-host-crash.json"
    )
    log_path = (
        Path(args.log).expanduser().resolve()
        if args.log
        else ROOT / "verification/planned-models-20261031/logs/klein-host-crash.log"
    )
    report.parent.mkdir(parents=True, exist_ok=True)
    log_path.parent.mkdir(parents=True, exist_ok=True)

    command = [
        str(HELPER),
        "--data-dir",
        str(data_root),
        "--mode",
        "edit" if source else "text",
        "--prompt",
        args.prompt,
        "--seed",
        args.seed,
        "--poll-milliseconds",
        "100",
    ]
    if source:
        command.extend(["--source", str(source)])
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
    submitted_turn = None
    observed = None
    killed = False
    try:
        with log_path.open("w") as log:
            deadline = started + 300
            while time.monotonic() < deadline and process.poll() is None:
                readable, _, _ = select.select([process.stdout], [], [], 0.5)
                if not readable:
                    continue
                line = process.stdout.readline()
                if not line:
                    break
                log.write(line)
                log.flush()
                value = parse_json_line(line)
                if value is None:
                    continue
                accepted = value.get("accepted")
                if isinstance(accepted, dict) and isinstance(accepted.get("turn"), dict):
                    submitted_turn = accepted["turn"]
                execution = value.get("execution")
                if (
                    isinstance(execution, dict)
                    and execution.get("stage") == "sampling"
                    and isinstance(execution.get("step"), int)
                    and execution["step"] >= 1
                ):
                    observed = execution
                    os.kill(process.pid, signal.SIGKILL)
                    killed = True
                    log.write(
                        json.dumps(
                            {
                                "acceptance_control": {
                                    "action": "SIGKILL",
                                    "pid": process.pid,
                                    "stage": execution["stage"],
                                    "step": execution["step"],
                                }
                            }
                        )
                        + "\n"
                    )
                    log.flush()
                    break
        if not killed:
            raise RuntimeError("real sampling was not observed before the deadline")
        returncode = process.wait(timeout=15)
        if submitted_turn is None or not isinstance(submitted_turn.get("id"), str):
            raise RuntimeError("submitted Turn identity was not observed")
        job_id = (
            (observed or {}).get("jobId")
            or submitted_turn.get("jobId")
            or next(
                (
                    line.get("turn", {}).get("jobId")
                    for line in map(
                        parse_json_line, log_path.read_text().splitlines()
                    )
                    if line
                ),
                None,
            )
        )
        if not isinstance(job_id, str):
            raise RuntimeError("Job identity was not observed")

        lease_path = data_root / "data/execution.lock"
        lease_was_busy = False
        lease_release_started = time.monotonic()
        with lease_path.open("a+b") as lease:
            deadline = time.monotonic() + 20
            while True:
                try:
                    fcntl.flock(lease.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                    fcntl.flock(lease.fileno(), fcntl.LOCK_UN)
                    break
                except BlockingIOError:
                    lease_was_busy = True
                    if time.monotonic() > deadline:
                        raise RuntimeError("supervisor did not release the execution lease")
                    time.sleep(0.05)
        lease_release_seconds = time.monotonic() - lease_release_started

        lingering = processes_mentioning(data_root)
        if lingering:
            raise RuntimeError(
                "owned process tree still present after lease release: "
                + json.dumps(lingering, ensure_ascii=False)
            )

        recovered = subprocess.run(
            [str(HELPER), "--data-dir", str(data_root), "--snapshot-only"],
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=30,
            check=True,
        )
        snapshot = snapshot_from_output(recovered.stdout)
        recovered_job = next(
            (job for job in snapshot["jobs"] if job.get("id") == job_id), None
        )
        if recovered_job is None:
            raise RuntimeError("recovered snapshot did not contain the killed Job")
        interrupted_attempts = attempts(data_root, job_id)
        if not interrupted_attempts or interrupted_attempts[-1]["state"] != "interrupted":
            raise RuntimeError("host crash did not persist an Interrupted Attempt")
        interrupted_manifest_json = interrupted_attempts[-1]["manifest_json"]
        if interrupted_manifest_json is None:
            raise RuntimeError("Interrupted Attempt lost its execution record")
        interrupted_manifest = json.loads(interrupted_manifest_json)
        if interrupted_manifest.get("outputs") != []:
            raise RuntimeError("Interrupted Attempt retained output metadata")
        if interrupted_manifest.get("final_result", {}).get("status") != "interrupted":
            raise RuntimeError("Interrupted execution record has the wrong final result")
        if interrupted_attempts[-1]["asset_count"] != 0:
            raise RuntimeError("Interrupted Attempt registered an AssetRef")
        if recovered_job.get("state") != "Interrupted" or recovered_job.get("assetId"):
            raise RuntimeError("recovered Job was not Interrupted without an AssetRef")

        export = report.with_suffix(".png")
        retry_command = [
            str(HELPER),
            "--data-dir",
            str(data_root),
            "--retry-job",
            job_id,
            "--export",
            str(export),
        ]
        retry = subprocess.run(
            retry_command,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=900,
            check=False,
        )
        (log_path.parent / "klein-host-crash-retry.log").write_text(
            retry.stdout + retry.stderr
        )
        if retry.returncode != 0:
            raise RuntimeError(
                "explicit retry failed: "
                + (retry.stderr.strip() or retry.stdout[-2048:])
            )
        retried = subprocess.run(
            [str(HELPER), "--data-dir", str(data_root), "--snapshot-only"],
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=30,
            check=True,
        )
        retry_snapshot = snapshot_from_output(retried.stdout)
        retry_job = next(
            (job for job in retry_snapshot["jobs"] if job.get("id") == job_id), None
        )
        retry_attempts = attempts(data_root, job_id)
        if len(retry_attempts) != 2:
            raise RuntimeError("retry did not create exactly one new Attempt")
        if retry_attempts[0]["state"] != "interrupted":
            raise RuntimeError("original Attempt did not retain Interrupted")
        if retry_attempts[-1]["state"] != "succeeded":
            raise RuntimeError("retry Attempt did not succeed")
        if len({row["token"] for row in retry_attempts}) != 2:
            raise RuntimeError("retry did not use a unique execution token")
        if (
            retry_job is None
            or retry_job.get("state") != "Succeeded"
            or not retry_job.get("assetId")
        ):
            raise RuntimeError("retried Job did not produce a formal AssetRef")
        if not export.is_file():
            raise RuntimeError("retried Job did not export a PNG")

        result = {
            "schemaVersion": 1,
            "runId": "planned-models-20261031",
            "caseId": "P07",
            "configurationKey": "K-E" if source else "K-T",
            "required": True,
            "status": "passed",
            "evidenceKind": "product_execution",
            "reason": "SIGKILL after a real sampling step; the supervisor reaped the tree, preserved Interrupted without an asset, and an explicit retry succeeded.",
            "command": command,
            "host_returncode": returncode,
            "observed_execution": observed,
            "job_id": job_id,
            "turn_id": submitted_turn["id"],
            "execution_lease_was_busy_during_recovery": lease_was_busy,
            "execution_lease_release_seconds": lease_release_seconds,
            "lingering_owned_processes_after_lease_release": lingering,
            "recovered_job": recovered_job,
            "attempts": [
                {
                    "id": row["id"],
                    "token": row["token"],
                    "state": row["state"],
                    "has_execution_record": row["manifest_json"] is not None,
                    "asset_count": row["asset_count"],
                    "error": row["error"],
                }
                for row in retry_attempts
            ],
            "retry_command": retry_command,
            "retry_job_state": retry_job["state"],
            "retry_asset_id": retry_job["assetId"],
            "export_path": str(export),
            "log_path": str(log_path),
            "elapsed_seconds": time.monotonic() - started,
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
