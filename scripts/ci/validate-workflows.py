#!/usr/bin/env python3
"""Validate the repository's GitHub Actions workflow structure without network access.

This is a local sanity check, not a substitute for actionlint or a real Actions
run: it parses the YAML (PyYAML) and asserts the expected trigger/jobs/steps
shape and the workflow file inventory. Run from the repository root:

    python scripts/ci/validate-workflows.py

Requires PyYAML (``python -m pip install PyYAML``).
"""
import sys
import re
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent.parent
WORKFLOWS = ROOT / ".github" / "workflows"
EXPECTED = {"quality.yml", "build.yml", "release.yml"}
PINNED_ACTION = re.compile(r"^[^@\s]+@[0-9a-f]{40}$")


def fail(errors: list[str], message: str) -> None:
    errors.append(message)


def main() -> int:
    errors: list[str] = []
    seen: set[str] = set()
    if not WORKFLOWS.is_dir():
        print(f"missing workflows directory: {WORKFLOWS}")
        return 1
    for path in sorted(WORKFLOWS.glob("*.yml")):
        seen.add(path.name)
        try:
            data = yaml.safe_load(path.read_text(encoding="utf-8"))
        except yaml.YAMLError as error:
            fail(errors, f"{path.name}: YAML error: {error}")
            continue
        if not isinstance(data, dict):
            fail(errors, f"{path.name}: top level is not a mapping")
            continue
        # PyYAML resolves the YAML 1.1 key `on` to True; accept both spellings.
        triggers = data.get("on", data.get(True))
        if not isinstance(triggers, dict):
            fail(errors, f"{path.name}: missing mapping trigger (`on`)")
        jobs = data.get("jobs")
        if not isinstance(jobs, dict) or not jobs:
            fail(errors, f"{path.name}: missing jobs")
            continue
        for job_name, job in jobs.items():
            if not isinstance(job, dict):
                fail(errors, f"{path.name}: job {job_name} is not a mapping")
                continue
            if "runs-on" not in job and "uses" not in job:
                fail(errors, f"{path.name}: job {job_name} has neither runs-on nor uses")
            steps = job.get("steps")
            if steps is not None:
                if not isinstance(steps, list) or not steps:
                    fail(errors, f"{path.name}: job {job_name} steps must be a non-empty list")
                for index, step in enumerate(steps):
                    if not isinstance(step, dict):
                        fail(errors, f"{path.name}: job {job_name} step {index} is not a mapping")
                        continue
                    if "uses" not in step and "run" not in step:
                        fail(
                            errors,
                            f"{path.name}: job {job_name} step {index} has neither uses nor run",
                        )
                    action = step.get("uses")
                    if isinstance(action, str) and not action.startswith(("./", "$/")):
                        if not PINNED_ACTION.fullmatch(action):
                            fail(
                                errors,
                                f"{path.name}: job {job_name} step {index} action is not pinned: {action}",
                            )
                    if isinstance(action, str) and action.startswith("actions/checkout@"):
                        with_values = step.get("with")
                        if not (
                            isinstance(with_values, dict)
                            and with_values.get("persist-credentials") is False
                        ):
                            fail(
                                errors,
                                f"{path.name}: job {job_name} step {index} checkout persists credentials",
                            )
    missing = EXPECTED - seen
    if missing:
        fail(errors, f"missing workflow files: {sorted(missing)}")
    quality = yaml.safe_load((WORKFLOWS / "quality.yml").read_text(encoding="utf-8"))
    quality_runners = quality["jobs"]["quality"]["strategy"]["matrix"]["os"]
    if quality_runners != ["windows-latest", "macos-15"]:
        fail(errors, f"quality runners must be Windows latest and macOS 15 arm64: {quality_runners}")
    build = yaml.safe_load((WORKFLOWS / "build.yml").read_text(encoding="utf-8"))
    if build["jobs"]["windows"].get("runs-on") != "windows-latest":
        fail(errors, "Windows build must use windows-latest")
    if build["jobs"]["macos"].get("runs-on") != "macos-15":
        fail(errors, "macOS build must use macos-15 arm64")
    release = yaml.safe_load((WORKFLOWS / "release.yml").read_text(encoding="utf-8"))
    if release.get("permissions", {}).get("contents") != "read":
        fail(errors, "release workflow must default to contents: read")
    if release["jobs"]["release"].get("permissions", {}).get("contents") != "write":
        fail(errors, "only the final release job may receive contents: write")
    if errors:
        print("\n".join(errors))
        return 1
    print("workflow YAML structure ok:", ", ".join(sorted(seen)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
