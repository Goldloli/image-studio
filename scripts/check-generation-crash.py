#!/usr/bin/env python3
"""Explicit real-model host SIGKILL acceptance and explicit recovery retry."""

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


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DATA_ROOT = Path.home() / "Library/Application Support/ImageStudio"
HELPER = ROOT / "target/debug/examples/generation-check"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--data-root",
        default=str(ROOT / "target/first-generation-validation"),
    )
    parser.add_argument("--report")
    parser.add_argument("--log")
    parser.add_argument("--seed", default="77")
    parser.add_argument(
        "--prompt",
        default=(
            "Warm clay architecture, an arched window, terracotta steps and a green "
            "plant, soft morning light, no text."
        ),
    )
    return parser.parse_args()


def final_json(output: str) -> dict:
    for line in reversed(output.splitlines()):
        try:
            value = json.loads(line)
        except ValueError:
            continue
        if isinstance(value, dict) and isinstance(value.get("jobs"), list):
            return value
    raise RuntimeError("helper output contained no final snapshot")


def attempts(data: Path, job_id: str) -> list[dict]:
    connection = sqlite3.connect(f"file:{data / 'data/library.sqlite3'}?mode=ro", uri=True)
    connection.row_factory = sqlite3.Row
    try:
        return [
            dict(row)
            for row in connection.execute(
                "SELECT id,token,state,plan_json,manifest_json,error FROM attempts "
                "WHERE job_id=? ORDER BY created_at,rowid",
                (job_id,),
            )
        ]
    finally:
        connection.close()


def main() -> int:
    args = parse_args()
    data = Path(args.data_root).expanduser().resolve()
    default = DEFAULT_DATA_ROOT.resolve()
    if data == default or data in default.parents or default in data.parents:
        raise SystemExit("--data-root must be isolated from the default ImageStudio root")
    if not HELPER.is_file():
        raise SystemExit("generation-check helper is missing")
    if not (data / "data/installation-ready.json").is_file():
        raise SystemExit("isolated data root has no installation-ready record")

    report = (
        Path(args.report).expanduser().resolve()
        if args.report
        else ROOT / "verification/first-generation-real-host-crash.json"
    )
    log_path = (
        Path(args.log).expanduser().resolve()
        if args.log
        else ROOT / "verification/first-generation-real-host-crash.log"
    )
    report.parent.mkdir(parents=True, exist_ok=True)
    log_path.parent.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    command = [
        str(HELPER),
        "--data-dir",
        str(data),
        "--prompt",
        args.prompt,
        "--aspect",
        "1:1",
        "--seed",
        args.seed,
    ]
    process = subprocess.Popen(
        command,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        start_new_session=True,
    )
    killed = False
    observed = None
    submitted = None
    try:
        with log_path.open("w") as log:
            while time.monotonic() - started < 240 and process.poll() is None:
                ready, _, _ = select.select([process.stdout], [], [], 0.5)
                if not ready:
                    continue
                line = process.stdout.readline()
                log.write(line)
                log.flush()
                if not line:
                    break
                try:
                    value = json.loads(line)
                except ValueError:
                    continue
                if isinstance(value.get("submitted"), dict):
                    submitted = value["submitted"]
                execution = value.get("execution", {})
                # Step 2 proves at least one real GPU evaluation has returned.
                if execution.get("stage") == "sampling" and (execution.get("step") or 0) >= 2:
                    observed = execution
                    os.kill(process.pid, signal.SIGKILL)
                    killed = True
                    break
        if not killed:
            raise RuntimeError("real sampling was not observed before deadline")
        returncode = process.wait(timeout=10)
        job_id = (submitted or {}).get("id") or observed.get("jobId")
        if not job_id:
            raise RuntimeError("submitted job identity was not observed")

        lease_path = data / "data/execution.lock"
        recovery_started = time.monotonic()
        recovered = subprocess.run(
            [str(HELPER), "--data-dir", str(data)],
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=30,
            check=True,
        )
        snapshot = final_json(recovered.stdout)
        interrupted_attempts = attempts(data, job_id)
        if not interrupted_attempts or interrupted_attempts[-1]["state"] != "interrupted":
            raise RuntimeError("host crash did not persist Interrupted")
        recovered_job = next(job for job in snapshot["jobs"] if job["id"] == job_id)
        if recovered_job["state"] != "Interrupted" or recovered_job["assetId"] is not None:
            raise RuntimeError("recovered job was not Interrupted without an asset")

        lease_was_busy = False
        with lease_path.open("a+b") as lease:
            deadline = time.monotonic() + 15
            while True:
                try:
                    fcntl.flock(lease.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                    fcntl.flock(lease.fileno(), fcntl.LOCK_UN)
                    break
                except BlockingIOError:
                    lease_was_busy = True
                    if time.monotonic() > deadline:
                        raise RuntimeError("native supervisor did not release its execution lease")
                    time.sleep(0.1)

        retry_command = [
            str(HELPER),
            "--data-dir",
            str(data),
            "--retry-job",
            job_id,
        ]
        retry = subprocess.run(
            retry_command,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=900,
            check=False,
        )
        if retry.returncode != 0:
            raise RuntimeError(
                "explicit retry failed: "
                + (retry.stderr.strip() or retry.stdout[-2048:])
            )
        retry_snapshot = final_json(retry.stdout)
        retry_job = next(job for job in retry_snapshot["jobs"] if job["id"] == job_id)
        retry_attempts = attempts(data, job_id)
        if len(retry_attempts) != 2:
            raise RuntimeError("retry did not create exactly one new Attempt")
        if retry_attempts[0]["state"] != "interrupted":
            raise RuntimeError("original Attempt did not retain Interrupted")
        if retry_attempts[-1]["state"] != "succeeded":
            raise RuntimeError("retry Attempt did not succeed")
        if len({row["token"] for row in retry_attempts}) != 2:
            raise RuntimeError("retry did not use a unique execution token")
        if retry_job["state"] != "Succeeded" or retry_job["assetId"] is None:
            raise RuntimeError("retried job did not produce a formal asset")

        result = {
            "method": "SIGKILL of owned live host process after real sampling step >=2",
            "is_full_model_execution": True,
            "command": command,
            "returncode": returncode,
            "observed_execution": observed,
            "recovered_job": recovered_job,
            "execution_lease_was_busy_during_recovery": lease_was_busy,
            "execution_lease_released": True,
            "first_restart_execution": snapshot["execution"],
            "retry_command": retry_command,
            "retry_job": {
                "state": retry_job["state"],
                "attempt_id": retry_job["attemptId"],
                "asset_id": retry_job["assetId"],
            },
            "attempts": [
                {key: row[key] for key in ("id", "token", "state", "error")}
                for row in retry_attempts
            ],
            "recovery_elapsed_seconds": time.monotonic() - recovery_started,
            "elapsed_seconds": time.monotonic() - started,
            "log": str(log_path),
        }
        report.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
        print(report)
        return 0
    finally:
        if process.poll() is None:
            process.kill()
            process.wait(timeout=10)


if __name__ == "__main__":
    raise SystemExit(main())
