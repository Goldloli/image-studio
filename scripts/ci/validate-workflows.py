#!/usr/bin/env python3
"""Validate the repository's GitHub Actions workflow structure without network access.

This is a local sanity check, not a substitute for actionlint or a real Actions
run: it parses the YAML (PyYAML) and asserts the expected trigger/jobs/steps
shape and the workflow file inventory. Run from the repository root:

    python scripts/ci/validate-workflows.py

Requires PyYAML (``python -m pip install PyYAML``).
"""
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent.parent
WORKFLOWS = ROOT / ".github" / "workflows"
EXPECTED = {"quality.yml", "build.yml", "release.yml"}


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
    missing = EXPECTED - seen
    if missing:
        fail(errors, f"missing workflow files: {sorted(missing)}")
    if errors:
        print("\n".join(errors))
        return 1
    print("workflow YAML structure ok:", ", ".join(sorted(seen)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
