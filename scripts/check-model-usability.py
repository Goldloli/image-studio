#!/usr/bin/env python3
"""Run and summarize the current image-model acceptance profiles.

The tool deliberately keeps model execution behind explicit commands. It never
downloads a model merely because a profile was selected, never operates on the
default user data root for automated/fault tests, and never derives a passed
case from a historical verification record.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass, field
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import signal
import shutil
import sqlite3
import stat
import subprocess
import sys
import time
from typing import Any, Iterable
import uuid
import zlib


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DATA_ROOT = Path.home() / "Library/Application Support/ImageStudio"
TEST_PLAN = ROOT / "openspec/changes/certify-current-model-usability/test-plan.md"
REPORT_SCHEMA_VERSION = 1
CASE_SET_VERSION = "certify-current-model-usability-2026-10-30"
VALID_STATES = {"passed", "failed", "not_run", "blocked"}
VALID_EVIDENCE_KINDS = {
    "contract_fixture",
    "device_probe",
    "real_execution",
    "native_ui",
    "historical_reference",
}
LOG_LIMIT_BYTES = 16 * 1024 * 1024


CASES: dict[str, dict[str, Any]] = {
    "C01": {
        "priority": "P0",
        "profile": "contracts",
        "evidence_kind": "contract_fixture",
        "description": "toolchain, frontend, Rust, worker and fixed-lock gates",
    },
    "C02": {
        "priority": "P0",
        "profile": "contracts",
        "evidence_kind": "contract_fixture",
        "description": "explicit native-supervisor SIGKILL fixture",
    },
    "C03": {
        "priority": "P0",
        "profile": "contracts",
        "evidence_kind": "contract_fixture",
        "description": "offline production runtime installation without model activation",
    },
    "C04": {
        "priority": "P0",
        "profile": "contracts",
        "evidence_kind": "contract_fixture",
        "description": "catalog separates implemented and planned entries",
    },
    "I04": {
        "priority": "P0",
        "profile": "contracts",
        "evidence_kind": "contract_fixture",
        "description": "range, full response, digest, interruption and space fixtures",
    },
    "I06": {
        "priority": "P0",
        "profile": "contracts",
        "evidence_kind": "contract_fixture",
        "description": "runtime links, inventory identity and path-escape fixtures",
    },
    "L07": {
        "priority": "P0",
        "profile": "contracts",
        "evidence_kind": "contract_fixture",
        "description": "cancel, token, framing, PNG and commit-window fixtures",
    },
    "L08": {
        "priority": "P0",
        "profile": "contracts",
        "evidence_kind": "real_execution",
        "description": "input boundaries and real tokenizer limit",
    },
    "L09": {
        "priority": "P0",
        "profile": "contracts",
        "evidence_kind": "contract_fixture",
        "description": "OOM, exception, timeout and channel failure classification",
    },
    "I01": {
        "priority": "P0",
        "profile": "model",
        "evidence_kind": "real_execution",
        "description": "empty-cache fixed-source network installation",
    },
    "I02": {
        "priority": "P0",
        "profile": "model",
        "evidence_kind": "real_execution",
        "description": "verified-cache offline installation",
    },
    "I03": {
        "priority": "P0",
        "profile": "model",
        "evidence_kind": "real_execution",
        "description": "offline full-model generation",
        "prompt": "一只橙色的小猫坐在窗边，窗外是秋天的树叶，温暖自然光，细腻的水彩插画。",
        "aspect": "1:1",
        "width": 1024,
        "height": 1024,
        "seed": "48",
    },
    "I05": {
        "priority": "P0",
        "profile": "model",
        "evidence_kind": "real_execution",
        "description": "isolated missing-component rejection",
    },
    "G01": {
        "priority": "P0",
        "profile": "model",
        "evidence_kind": "real_execution",
        "description": "Chinese square Z-Image generation",
        "prompt": "一只橙色的小猫坐在窗边，窗外是秋天的树叶，温暖自然光，细腻的水彩插画。",
        "aspect": "1:1",
        "width": 1024,
        "height": 1024,
        "seed": "42",
    },
    "G02": {
        "priority": "P0",
        "profile": "model",
        "evidence_kind": "real_execution",
        "description": "English square Z-Image generation",
        "prompt": "An orange cat sits by a window with autumn leaves outside, warm natural light, delicate watercolor illustration.",
        "aspect": "1:1",
        "width": 1024,
        "height": 1024,
        "seed": "42",
    },
    "G03": {
        "priority": "P0",
        "profile": "model",
        "evidence_kind": "real_execution",
        "description": "Chinese landscape Z-Image generation",
        "prompt": "平静山湖的横向风景，远处雪山，近处木船，清晨薄雾，写实摄影。",
        "aspect": "4:3",
        "width": 1024,
        "height": 768,
        "seed": "43",
    },
    "G04": {
        "priority": "P0",
        "profile": "model",
        "evidence_kind": "real_execution",
        "description": "English landscape Z-Image generation",
        "prompt": "A wide landscape of a calm mountain lake, snowy peaks in the distance, a wooden boat in the foreground, morning mist, realistic photography.",
        "aspect": "4:3",
        "width": 1024,
        "height": 768,
        "seed": "43",
    },
    "G05": {
        "priority": "P0",
        "profile": "model",
        "evidence_kind": "real_execution",
        "description": "Chinese portrait Z-Image generation",
        "prompt": "一束向日葵插在白色陶瓷花瓶里，木桌上有一本合上的书，柔和侧光，竖幅静物摄影。",
        "aspect": "3:4",
        "width": 768,
        "height": 1024,
        "seed": "44",
    },
    "G06": {
        "priority": "P0",
        "profile": "model",
        "evidence_kind": "real_execution",
        "description": "English portrait Z-Image generation",
        "prompt": "A bouquet of sunflowers in a white ceramic vase, a closed book on a wooden table, soft side lighting, vertical still life photography.",
        "aspect": "3:4",
        "width": 768,
        "height": 1024,
        "seed": "44",
    },
    "L04": {
        "priority": "P0",
        "profile": "model",
        "evidence_kind": "real_execution",
        "description": "real loading-stage cancellation",
    },
    "L01": {
        "priority": "P0",
        "profile": "model",
        "evidence_kind": "native_ui",
        "description": "three independent generations in one native conversation",
    },
    "L02": {
        "priority": "P0",
        "profile": "model",
        "evidence_kind": "real_execution",
        "description": "random seed freeze, cancellation and explicit retry",
    },
    "L03": {
        "priority": "P0",
        "profile": "model",
        "evidence_kind": "native_ui",
        "description": "native rapid-submit deduplication and idempotency",
    },
    "L05": {
        "priority": "P0",
        "profile": "model",
        "evidence_kind": "real_execution",
        "description": "real sampling-stage cancellation and explicit retry",
    },
    "L06": {
        "priority": "P0",
        "profile": "model",
        "evidence_kind": "real_execution",
        "description": "host SIGKILL after real GPU sampling and recovery",
    },
    "S02": {
        "priority": "P0",
        "profile": "model",
        "evidence_kind": "native_ui",
        "description": "offline native startup without a model",
    },
    "S01": {
        "priority": "P0",
        "profile": "daily",
        "evidence_kind": "native_ui",
        "description": "ordinary default-root startup",
    },
}

for index in range(1, 11):
    case_id = f"D{index:02}"
    CASES[case_id] = {
        "priority": "P0",
        "profile": "daily",
        "evidence_kind": "native_ui",
        "description": "native default-root daily workflow",
    }

PROFILE_CASES = {
    "contracts": ["C01", "C02", "C03", "C04", "I04", "I06", "L07", "L08", "L09"],
    "model": [
        "I01",
        "I02",
        "I03",
        "I05",
        "G01",
        "G02",
        "G03",
        "G04",
        "G05",
        "G06",
        "L01",
        "L02",
        "L03",
        "L04",
        "L05",
        "L06",
        "S02",
    ],
    "daily": [f"D{index:02}" for index in range(1, 11)] + ["S01"],
}
QUALITY_CASES = {f"G{index:02}" for index in range(1, 7)}


@dataclass
class CommandResult:
    command: list[str]
    cwd: str
    exit_code: int
    elapsed_seconds: float
    timed_out: bool = False
    log_path: str | None = None


@dataclass
class CaseResult:
    case_id: str
    status: str
    evidence_kind: str
    started_at: str
    finished_at: str
    reason: str = ""
    command: list[str] = field(default_factory=list)
    exit_code: int | None = None
    evidence: list[dict[str, Any]] = field(default_factory=list)
    details: dict[str, Any] = field(default_factory=dict)

    def validate(self) -> None:
        if self.status not in VALID_STATES:
            raise ValueError(f"invalid case status: {self.status}")
        if self.evidence_kind not in VALID_EVIDENCE_KINDS:
            raise ValueError(f"invalid evidence kind: {self.evidence_kind}")

    def as_dict(self, definition: dict[str, Any]) -> dict[str, Any]:
        self.validate()
        return {
            "case_id": self.case_id,
            "priority": definition["priority"],
            "profile": definition["profile"],
            "description": definition["description"],
            "status": self.status,
            "evidence_kind": self.evidence_kind,
            "started_at": self.started_at,
            "finished_at": self.finished_at,
            "reason": self.reason,
            "command": self.command,
            "exit_code": self.exit_code,
            "evidence": self.evidence,
            "details": self.details,
        }


def now_utc() -> str:
    return datetime.now(timezone.utc).isoformat()


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def atomic_write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{uuid.uuid4().hex}.tmp")
    descriptor = os.open(
        temporary,
        os.O_CREAT | os.O_WRONLY | os.O_EXCL | os.O_NOFOLLOW,
        0o600,
    )
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
            stream.write(content)
            stream.flush()
            os.fsync(stream.fileno())
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def bounded_log_text(value: bytes) -> str:
    if len(value) <= LOG_LIMIT_BYTES:
        return value.decode("utf-8", errors="replace")
    head_bytes = 2 * 1024 * 1024
    tail_bytes = LOG_LIMIT_BYTES - head_bytes
    omitted = len(value) - LOG_LIMIT_BYTES
    return (
        value[:head_bytes].decode("utf-8", errors="replace")
        + f"\n\n[... {omitted} output bytes omitted ...]\n\n"
        + value[-tail_bytes:].decode("utf-8", errors="replace")
    )


def normalized_log_text(value: bytes) -> str:
    text = bounded_log_text(value).replace("\r\n", "\n").replace("\r", "\n")
    return text.rstrip() + "\n"


def run_command(
    name: str,
    command: list[str],
    report_dir: Path,
    *,
    cwd: Path = ROOT,
    timeout: int = 900,
    env: dict[str, str] | None = None,
) -> CommandResult:
    log_path = report_dir / "logs" / f"{name}.log"
    log_path.parent.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    merged_env = os.environ.copy()
    if env:
        merged_env.update(env)
    process: subprocess.Popen[bytes] | None = None
    timed_out = False
    try:
        process = subprocess.Popen(
            command,
            cwd=cwd,
            env=merged_env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        try:
            output, _ = process.communicate(timeout=timeout)
            exit_code = process.returncode
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(process.pid, signal.SIGKILL)
            output, _ = process.communicate(timeout=10)
            exit_code = 124
    except OSError as error:
        output = f"{type(error).__name__}: {error}\n".encode()
        exit_code = 127
    elapsed = round(time.monotonic() - started, 3)
    header = (
        f"command={json.dumps(command, ensure_ascii=False)}\n"
        f"cwd={cwd}\n"
        f"exit_code={exit_code}\n"
        f"timed_out={timed_out}\n"
        f"elapsed_seconds={elapsed}\n\n"
    ).encode()
    atomic_write(log_path, normalized_log_text(header + output))
    return CommandResult(
        command=command,
        cwd=str(cwd),
        exit_code=exit_code,
        elapsed_seconds=elapsed,
        timed_out=timed_out,
        log_path=str(log_path),
    )


def evidence(path: Path, *, note: str = "") -> dict[str, Any]:
    resolved = path.resolve()
    item: dict[str, Any] = {
        "path": str(resolved),
        "exists": resolved.is_file(),
        "sha256": None,
        "bytes": None,
    }
    if resolved.is_file():
        item["sha256"] = sha256_file(resolved)
        item["bytes"] = resolved.stat().st_size
    if note:
        item["note"] = note
    return item


def command_evidence(result: CommandResult) -> list[dict[str, Any]]:
    return [evidence(Path(result.log_path), note="bounded command output")] if result.log_path else []


def is_within(path: Path, parent: Path) -> bool:
    try:
        path.relative_to(parent)
        return True
    except ValueError:
        return False


def validate_isolated_root(raw: str | Path) -> Path:
    path = Path(raw).expanduser()
    if not path.is_absolute():
        raise ValueError("--data-root must be an absolute path")
    if path.is_symlink():
        raise ValueError("--data-root must not be a symbolic link")
    resolved = path.resolve(strict=False)
    default = DEFAULT_DATA_ROOT.resolve(strict=False)
    home = Path.home().resolve()
    if resolved == Path("/") or resolved == home:
        raise ValueError("--data-root cannot be the filesystem root or home directory")
    if resolved == default or is_within(resolved, default) or is_within(default, resolved):
        raise ValueError(
            "--data-root must be an isolated directory that does not contain or sit inside "
            "the default ImageStudio data root"
        )
    return resolved


def validate_report_dir(raw: str | Path) -> Path:
    path = Path(raw).expanduser()
    if not path.is_absolute():
        raise ValueError("--report-dir must be an absolute path")
    resolved = path.resolve(strict=False)
    default = DEFAULT_DATA_ROOT.resolve(strict=False)
    if resolved == default or is_within(resolved, default):
        raise ValueError("--report-dir cannot be inside the default user data root")
    return resolved


def source_identity() -> dict[str, Any]:
    commands = {
        "rustc": ["rustc", "--version"],
        "cargo": ["cargo", "--version"],
        "node": ["node", "--version"],
        "npm": ["npm", "--version"],
        "python": [sys.executable, "--version"],
    }
    versions: dict[str, str] = {}
    for name, command in commands.items():
        try:
            versions[name] = subprocess.run(
                command,
                cwd=ROOT,
                capture_output=True,
                text=True,
                timeout=10,
                check=False,
            ).stdout.strip()
        except OSError:
            versions[name] = "unavailable"
    git_head = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )
    git_status = subprocess.run(
        ["git", "status", "--porcelain=v1"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )
    files = [
        ROOT / "Cargo.lock",
        ROOT / "resources/local-generation/model-manifest.json",
        ROOT / "resources/local-generation/runtime-manifest.json",
        ROOT / "resources/model-catalog/catalog.json",
        ROOT / "apps/frontend/package-lock.json",
    ]
    return {
        "git_head": git_head.stdout.strip() if git_head.returncode == 0 else None,
        "working_tree_status_sha256": sha256_bytes(git_status.stdout.encode()),
        "working_tree_dirty": bool(git_status.stdout.strip()),
        "versions": versions,
        "file_sha256": {
            str(path.relative_to(ROOT)): sha256_file(path)
            for path in files
            if path.is_file()
        },
        "test_plan_sha256": sha256_file(TEST_PLAN) if TEST_PLAN.is_file() else None,
        "case_set_version": CASE_SET_VERSION,
    }


def environment_identity() -> dict[str, Any]:
    system = platform.system()
    details: dict[str, Any] = {
        "platform": system,
        "platform_release": platform.release(),
        "machine": platform.machine(),
        "python": sys.version,
        "timezone": time.tzname,
    }
    if system == "Darwin":
        version = subprocess.run(
            ["sw_vers"], capture_output=True, text=True, timeout=10, check=False
        )
        details["sw_vers"] = version.stdout.strip()
        memory = subprocess.run(
            ["sysctl", "-n", "hw.memsize"],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        cpu = subprocess.run(
            ["sysctl", "-n", "machdep.cpu.brand_string"],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        details["memory_bytes"] = int(memory.stdout.strip()) if memory.stdout.strip().isdigit() else None
        details["cpu"] = cpu.stdout.strip() or None
    return details


def root_snapshot(path: Path | None) -> dict[str, Any]:
    if path is None:
        return {"path": None, "category": "not_applicable"}
    default = DEFAULT_DATA_ROOT.resolve(strict=False)
    resolved = path.resolve(strict=False)
    category = "default" if resolved == default else "isolated"
    snapshot: dict[str, Any] = {
        "path": str(resolved),
        "category": category,
        "exists": resolved.exists(),
        "top_level": [],
    }
    if resolved.is_dir():
        snapshot["top_level"] = sorted(
            entry.name for entry in resolved.iterdir() if not entry.name.startswith(".")
        )
    ready = resolved / "data/installation-ready.json"
    if ready.is_file():
        try:
            record = json.loads(ready.read_text())
            snapshot["installation"] = {
                "runtime_lock_id": record.get("runtime_lock_id"),
                "model_snapshot_id": record.get("model_snapshot_id"),
                "runtime_manifest_sha256": record.get("runtime_manifest_sha256"),
                "model_manifest_sha256": record.get("model_manifest_sha256"),
                "device": (record.get("probe") or {}).get("device"),
                "device_name": (record.get("probe") or {}).get("device_name"),
                "runtime_file_count": len(record.get("runtime_files") or []),
            }
        except (OSError, ValueError, TypeError) as error:
            snapshot["installation_error"] = f"{type(error).__name__}: {error}"
    return snapshot


def test_names_from_log(path: str | None) -> set[str]:
    if not path:
        return set()
    try:
        text = Path(path).read_text(errors="replace")
    except OSError:
        return set()
    names = set()
    for line in text.splitlines():
        match = re.match(r"\s*test ([^ ]+) \.\.\. (?:ok|ignored|FAILED)", line)
        if match:
            names.add(match.group(1))
    return names


def build_case_result(
    case_id: str,
    status: str,
    reason: str,
    *,
    command: list[str] | None = None,
    exit_code: int | None = None,
    evidence_items: list[dict[str, Any]] | None = None,
    details: dict[str, Any] | None = None,
    started_at: str | None = None,
) -> CaseResult:
    definition = CASES[case_id]
    return CaseResult(
        case_id=case_id,
        status=status,
        evidence_kind=definition["evidence_kind"],
        started_at=started_at or now_utc(),
        finished_at=now_utc(),
        reason=reason,
        command=command or [],
        exit_code=exit_code,
        evidence=evidence_items or [],
        details=details or {},
    )


def run_contract_c01(report_dir: Path) -> CaseResult:
    started = now_utc()
    steps = [
        (
            "frontend-check",
            ["npm", "--prefix", "apps/frontend", "run", "check"],
            300,
        ),
        (
            "frontend-tests",
            ["npm", "--prefix", "apps/frontend", "test"],
            300,
        ),
        (
            "frontend-build",
            ["npm", "--prefix", "apps/frontend", "run", "build"],
            300,
        ),
        ("rust-fmt", ["cargo", "fmt", "--all", "--", "--check"], 300),
        (
            "rust-clippy",
            [
                "cargo",
                "clippy",
                "--workspace",
                "--all-targets",
                "--locked",
                "--offline",
                "--",
                "-D",
                "warnings",
            ],
            900,
        ),
        ("rust-tests", ["cargo", "test", "--workspace", "--locked", "--offline"], 900),
        (
            "rust-build",
            [
                "cargo",
                "build",
                "--workspace",
                "--examples",
                "--bins",
                "--locked",
                "--offline",
            ],
            900,
        ),
        (
            "worker-tests",
            [
                sys.executable,
                "-m",
                "unittest",
                "discover",
                "-s",
                "tools/mflux-worker/tests",
                "-p",
                "test*.py",
                "-v",
            ],
            300,
        ),
        (
            "acceptance-tool-tests",
            [
                sys.executable,
                "-m",
                "unittest",
                "discover",
                "-s",
                "scripts/tests",
                "-p",
                "test*.py",
                "-v",
            ],
            300,
        ),
        (
            "generation-lock",
            [sys.executable, "scripts/verify-generation-lock.py"],
            120,
        ),
    ]
    records = []
    evidence_items = []
    all_passed = True
    for name, command, timeout in steps:
        result = run_command(name, command, report_dir, timeout=timeout)
        records.append(
            {
                "name": name,
                "command": result.command,
                "cwd": result.cwd,
                "exit_code": result.exit_code,
                "timed_out": result.timed_out,
                "elapsed_seconds": result.elapsed_seconds,
                "log": result.log_path,
            }
        )
        evidence_items.extend(command_evidence(result))
        all_passed = all_passed and result.exit_code == 0 and not result.timed_out
    rust_log = next((row["log"] for row in records if row["name"] == "rust-tests"), None)
    ignored = sorted(
        line.strip()
        for line in Path(rust_log).read_text(errors="replace").splitlines()
        if line.strip().startswith("test ") and " ... ignored" in line
    )
    return build_case_result(
        "C01",
        "passed" if all_passed else "failed",
        "all build, test and fixed-lock gates passed"
        if all_passed
        else "at least one build, test or fixed-lock gate failed",
        command=[step[1] for step in steps],
        exit_code=0 if all_passed else 1,
        evidence_items=evidence_items,
        details={
            "steps": records,
            "default_ignored_tests": ignored,
            "ignored_tests_are_not_success": True,
        },
        started_at=started,
    )


def run_contract_c02(report_dir: Path) -> CaseResult:
    supervisor = ROOT / "target/debug/inference-supervisor"
    started = now_utc()
    if not supervisor.is_file():
        return build_case_result(
            "C02",
            "blocked",
            "native supervisor binary is missing",
            started_at=started,
        )
    command = [
        "cargo",
        "test",
        "-p",
        "image-studio-desktop",
        "adapters::generation::tests::actual_native_supervisor_sigkill_is_reclaimed_by_its_current_host",
        "--lib",
        "--locked",
        "--offline",
        "--",
        "--ignored",
        "--exact",
        "--nocapture",
    ]
    result = run_command(
        "C02-native-supervisor-sigkill",
        command,
        report_dir,
        timeout=300,
        env={"STUDIO_TEST_NATIVE_SUPERVISOR": str(supervisor.resolve())},
    )
    return build_case_result(
        "C02",
        "passed" if result.exit_code == 0 and not result.timed_out else "failed",
        "explicit native-supervisor SIGKILL fixture passed"
        if result.exit_code == 0 and not result.timed_out
        else "explicit native-supervisor SIGKILL fixture failed",
        command=command,
        exit_code=result.exit_code,
        evidence_items=command_evidence(result),
        details={
            "fixture_not_full_model": True,
            "elapsed_seconds": result.elapsed_seconds,
        },
        started_at=started,
    )


def run_contract_c03(report_dir: Path) -> CaseResult:
    started = now_utc()
    cache = ROOT / "target/first-generation-validation/cache/downloads"
    if not cache.is_dir():
        return build_case_result(
            "C03",
            "blocked",
            "fixed runtime cache is missing",
            started_at=started,
        )
    command = [
        "/usr/bin/sandbox-exec",
        "-f",
        str(ROOT / "verification/deny-network.sb"),
        "/usr/bin/env",
        "STUDIO_ACCEPTANCE_NETWORK_DENIED=1",
        "cargo",
        "test",
        "-p",
        "image-studio-desktop",
        "adapters::installation::tests::actual_private_runtime_installs_offline_without_activating_missing_model",
        "--lib",
        "--locked",
        "--offline",
        "--",
        "--ignored",
        "--exact",
        "--nocapture",
    ]
    result = run_command("C03-offline-runtime-install", command, report_dir, timeout=600)
    report = ROOT / "verification/first-generation-runtime-install.json"
    passed = result.exit_code == 0 and not result.timed_out and report.is_file()
    details: dict[str, Any] = {"elapsed_seconds": result.elapsed_seconds}
    if report.is_file():
        try:
            record = json.loads(report.read_text())
            details.update(
                {
                    "network_denied": record.get("network_denied"),
                    "installation_activated": record.get("installation_activated"),
                    "runtime_lock_id": record.get("runtime_lock_id"),
                    "model_snapshot_id": record.get("model_snapshot_id"),
                    "inventory_entries": record.get("inventory_entries"),
                    "probe_device": (record.get("probe") or {}).get("device"),
                    "is_full_model_inference": record.get("is_full_model_inference"),
                }
            )
            passed = (
                passed
                and record.get("network_denied") is True
                and record.get("installation_activated") is False
                and record.get("is_full_model_inference") is False
                and (record.get("probe") or {}).get("device") == "gpu"
            )
        except (OSError, ValueError, TypeError):
            passed = False
    evidence_items = command_evidence(result) + [evidence(report)]
    return build_case_result(
        "C03",
        "passed" if passed else "failed",
        "offline runtime installation passed and did not activate a model"
        if passed
        else "offline runtime installation result was missing or inconsistent",
        command=command,
        exit_code=result.exit_code,
        evidence_items=evidence_items,
        details=details,
        started_at=started,
    )


def run_contract_c04(_: Path) -> CaseResult:
    started = now_utc()
    catalog_path = ROOT / "resources/model-catalog/catalog.json"
    try:
        catalog = json.loads(catalog_path.read_text())
        entries = catalog["entries"]
    except (OSError, ValueError, KeyError, TypeError) as error:
        return build_case_result(
            "C04",
            "failed",
            f"catalog could not be read: {type(error).__name__}: {error}",
            evidence_items=[evidence(catalog_path)],
            started_at=started,
        )
    implemented = [entry for entry in entries if entry.get("productStatus") == "implemented"]
    planned = [entry for entry in entries if entry.get("productStatus") == "planned"]
    implemented_ids = sorted(entry.get("id") for entry in implemented)
    planned_ids = sorted(entry.get("id") for entry in planned)
    executable = [
        entry
        for entry in entries
        if entry.get("installerSupported") is True or entry.get("installId")
    ]
    passed = (
        len(entries) == 4
        and implemented_ids
        == [
            "birefnet-general-epoch-244",
            "flux2-klein-4b-distilled",
            "realesrgan-x4plus",
            "z-image-turbo-mflux-v1",
        ]
        and planned_ids == []
        and len(executable) == 4
        and {entry.get("id") for entry in executable}
        == {
            "birefnet-general-epoch-244",
            "flux2-klein-4b-distilled",
            "realesrgan-x4plus",
            "z-image-turbo-mflux-v1",
        }
        and all(not entry.get("installerSupported") for entry in planned)
        and all(entry.get("installId") is None for entry in planned)
    )
    return build_case_result(
        "C04",
        "passed" if passed else "failed",
        "four implemented executable entries and no planned entries"
        if passed
        else "catalog status or executable actions do not match the certification scope",
        evidence_items=[evidence(catalog_path)],
        details={
            "total": len(entries),
            "implemented": implemented_ids,
            "planned": planned_ids,
            "executable": sorted(entry.get("id") for entry in executable),
        },
        started_at=started,
    )


def run_installation_contracts(case_id: str, report_dir: Path) -> CaseResult:
    started = now_utc()
    command = [
        "cargo",
        "test",
        "-p",
        "image-studio-desktop",
        "adapters::installation::tests",
        "--lib",
        "--locked",
        "--offline",
        "--",
        "--nocapture",
    ]
    result = run_command(f"{case_id}-installation-fixtures", command, report_dir, timeout=600)
    names = test_names_from_log(result.log_path)
    required = {
        "I04": {
            "adapters::installation::tests::full_response_replaces_a_partial_without_appending",
            "adapters::installation::tests::interrupted_transfer_keeps_a_resumable_part_but_never_a_final_file",
            "adapters::installation::tests::range_requires_exact_start_end_and_total",
            "adapters::installation::tests::range_resume_requires_identity_and_preserves_exact_bytes",
            "adapters::installation::tests::stalled_transfer_cancellation_does_not_wait_for_a_file_or_read_deadline",
            "adapters::installation::tests::wrong_digest_never_becomes_a_completed_cache_file",
        },
        "I06": {
            "adapters::installation::tests::cached_part_and_managed_parent_links_cannot_write_outside",
            "adapters::installation::tests::paths_urls_and_hashes_fail_closed",
            "adapters::installation::tests::runtime_inventory_detects_changed_link_targets",
            "adapters::installation::tests::tar_links_cannot_escape_the_private_runtime",
        },
    }[case_id]
    missing = sorted(required - names)
    passed = result.exit_code == 0 and not result.timed_out and not missing
    return build_case_result(
        case_id,
        "passed" if passed else "failed",
        f"all {len(required)} required installation fixtures passed"
        if passed
        else "one or more required installation fixtures failed or were not observed",
        command=command,
        exit_code=result.exit_code,
        evidence_items=command_evidence(result),
        details={
            "required_tests": sorted(required),
            "observed_tests": sorted(names),
            "missing_tests": missing,
        },
        started_at=started,
    )


def run_lifecycle_contracts(case_id: str, report_dir: Path) -> CaseResult:
    started = now_utc()
    generation_command = [
        "cargo",
        "test",
        "-p",
        "image-studio-desktop",
        "adapters::generation::tests",
        "--lib",
        "--locked",
        "--offline",
        "--",
        "--nocapture",
    ]
    worker_command = [
        sys.executable,
        "-m",
        "unittest",
        "discover",
        "-s",
        "tools/mflux-worker/tests",
        "-p",
        "test*.py",
        "-v",
    ]
    generation = run_command(f"{case_id}-generation-fixtures", generation_command, report_dir, timeout=600)
    worker = run_command(f"{case_id}-worker-fixtures", worker_command, report_dir, timeout=300)
    generation_names = test_names_from_log(generation.log_path)
    required = {
        "L07": {
            "adapters::generation::tests::accepted_cancel_before_preparation_remains_cancelled",
            "adapters::generation::tests::candidate_waits_for_process_exit_before_durable_success",
            "adapters::generation::tests::cleanup_incomplete_is_nonterminal_and_later_cancel_can_finish",
            "adapters::generation::tests::framing_failure_closes_control_before_reaping_and_records_failure",
            "adapters::generation::tests::missing_supervisor_finishes_persisted_attempt_without_leaving_busy",
        },
        "L09": {
            "adapters::generation::tests::cleanup_incomplete_is_nonterminal_and_later_cancel_can_finish",
            "adapters::generation::tests::existing_lease_blocks_launch_and_does_not_claim_resource_release",
            "adapters::generation::tests::framing_failure_closes_control_before_reaping_and_records_failure",
            "adapters::generation::tests::missing_supervisor_finishes_persisted_attempt_without_leaving_busy",
        },
    }[case_id]
    missing = sorted(required - generation_names)
    passed = (
        generation.exit_code == 0
        and worker.exit_code == 0
        and not generation.timed_out
        and not worker.timed_out
        and not missing
    )
    return build_case_result(
        case_id,
        "passed" if passed else "failed",
        "all required controlled lifecycle fixtures passed"
        if passed
        else "controlled lifecycle fixture failed or an expected test was not observed",
        command=[generation_command, worker_command],
        exit_code=0 if passed else 1,
        evidence_items=command_evidence(generation) + command_evidence(worker),
        details={
            "required_generation_tests": sorted(required),
            "observed_generation_tests": sorted(generation_names),
            "missing_generation_tests": missing,
            "worker_exit_code": worker.exit_code,
        },
        started_at=started,
    )


def run_contract_l08(data_root: Path, report_dir: Path) -> CaseResult:
    started = now_utc()
    generation_command = [
        "cargo",
        "test",
        "-p",
        "studio-core",
        "generation::tests",
        "--lib",
        "--locked",
        "--offline",
        "--",
        "--nocapture",
    ]
    worker_command = [
        sys.executable,
        "-m",
        "unittest",
        "discover",
        "-s",
        "tools/mflux-worker/tests",
        "-p",
        "test*.py",
        "-v",
    ]
    generation = run_command("L08-generation-boundaries", generation_command, report_dir, timeout=300)
    worker = run_command("L08-worker-boundaries", worker_command, report_dir, timeout=300)
    generation_names = test_names_from_log(generation.log_path)
    required = {
        "studio_core::generation::tests::explicit_seed_is_preserved",
        "studio_core::generation::tests::unsupported_inputs_are_not_downgraded",
        "studio_core::generation::tests::random_seed_is_frozen_without_rewriting_request",
    }
    # Cargo's qualified test names omit the crate prefix in output.
    observed_short = {
        name.split("::tests::", 1)[-1] if "::tests::" in name else name
        for name in generation_names
    }
    required_short = {
        name.split("::tests::", 1)[-1] if "::tests::" in name else name
        for name in required
    }
    missing = sorted(required_short - observed_short)
    tokenizer_report = report_dir / "L08-real-tokenizer.json"
    tokenizer_command = [
        "/usr/bin/sandbox-exec",
        "-f",
        str(ROOT / "verification/deny-network.sb"),
        sys.executable,
        "scripts/probe-model-tokenizer.py",
        "--data-root",
        str(data_root),
        "--report",
        str(tokenizer_report),
        "--network-denied",
    ]
    tokenizer = run_command(
        "L08-real-tokenizer",
        tokenizer_command,
        report_dir,
        timeout=180,
    )
    tokenizer_record: dict[str, Any] = {}
    if tokenizer_report.is_file():
        try:
            tokenizer_record = json.loads(tokenizer_report.read_text())
        except (OSError, ValueError, TypeError):
            tokenizer_record = {}
    passed = (
        generation.exit_code == 0
        and worker.exit_code == 0
        and tokenizer.exit_code == 0
        and not generation.timed_out
        and not worker.timed_out
        and not tokenizer.timed_out
        and not missing
        and tokenizer_record.get("network_denied") is True
        and tokenizer_record.get("short_prompt_within_limit") is True
        and tokenizer_record.get("long_prompt_rejected") is True
        and tokenizer_record.get("truncation_disabled") is True
        and tokenizer_record.get("model_weights_loaded") is False
    )
    return build_case_result(
        "L08",
        "passed" if passed else "failed",
        "controlled boundary fixtures and real offline tokenizer probe passed"
        if passed
        else "an input-boundary fixture or the real offline tokenizer probe failed",
        command=[generation_command, worker_command, tokenizer_command],
        exit_code=0 if passed else 1,
        evidence_items=command_evidence(generation)
        + command_evidence(worker)
        + command_evidence(tokenizer)
        + [evidence(tokenizer_report)],
        details={
            "required_generation_tests": sorted(required),
            "observed_generation_tests": sorted(generation_names),
            "missing_generation_tests": missing,
            "tokenizer": {
                key: tokenizer_record.get(key)
                for key in (
                    "network_denied",
                    "runtime_lock_id",
                    "model_snapshot_id",
                    "short_prompt_template_tokens",
                    "long_prompt_rejection",
                    "truncation_disabled",
                    "model_weights_loaded",
                    "is_full_model_inference",
                )
            },
        },
        started_at=started,
    )


def validate_png(path: Path, expected_width: int, expected_height: int) -> dict[str, Any]:
    data = path.read_bytes()
    if not data.startswith(b"\x89PNG\r\n\x1a\n"):
        raise ValueError("PNG signature is missing")
    cursor = 8
    width = height = None
    idat = bytearray()
    saw_iend = False
    while cursor < len(data):
        if cursor + 12 > len(data):
            raise ValueError("truncated PNG chunk")
        length = int.from_bytes(data[cursor : cursor + 4], "big")
        chunk_type = data[cursor + 4 : cursor + 8]
        end = cursor + 12 + length
        if end > len(data):
            raise ValueError("PNG chunk exceeds file")
        payload = data[cursor + 8 : cursor + 8 + length]
        checksum = int.from_bytes(data[cursor + 8 + length : end], "big")
        if zlib.crc32(chunk_type + payload) & 0xFFFFFFFF != checksum:
            raise ValueError(f"PNG chunk CRC mismatch: {chunk_type!r}")
        if chunk_type == b"IHDR":
            if length != 13:
                raise ValueError("invalid IHDR length")
            width = int.from_bytes(payload[0:4], "big")
            height = int.from_bytes(payload[4:8], "big")
        elif chunk_type == b"IDAT":
            idat.extend(payload)
        elif chunk_type == b"IEND":
            saw_iend = True
            if end != len(data):
                raise ValueError("bytes follow PNG IEND")
            break
        cursor = end
    if width is None or height is None or not idat or not saw_iend:
        raise ValueError("PNG is missing IHDR, IDAT or IEND")
    if (width, height) != (expected_width, expected_height):
        raise ValueError(f"PNG dimensions are {width}x{height}, expected {expected_width}x{expected_height}")
    decompressed = zlib.decompress(idat)
    if not decompressed:
        raise ValueError("PNG image data decompressed to zero bytes")
    return {
        "width": width,
        "height": height,
        "bytes": len(data),
        "decompressed_bytes": len(decompressed),
        "chunk_structure_valid": True,
    }


def parse_generation_output(text: str) -> tuple[list[dict[str, Any]], dict[str, Any] | None]:
    submitted = []
    final = None
    for line in text.splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            value = json.loads(line)
        except ValueError:
            continue
        if isinstance(value.get("submitted"), dict):
            submitted.append(value["submitted"])
        if isinstance(value.get("jobs"), list) and isinstance(value.get("assets"), list):
            final = value
    return submitted, final


def generation_control_events(text: str) -> list[dict[str, Any]]:
    events = []
    for line in text.splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            value = json.loads(line)
        except ValueError:
            continue
        if isinstance(value.get("acceptance_control"), dict):
            events.append(value["acceptance_control"])
    return events


def sqlite_rows(
    data_root: Path, query: str, parameters: tuple[Any, ...] = ()
) -> list[dict[str, Any]]:
    database = data_root / "data/library.sqlite3"
    connection = sqlite3.connect(f"file:{database}?mode=ro", uri=True)
    connection.row_factory = sqlite3.Row
    try:
        rows = connection.execute(query, parameters).fetchall()
        return [dict(row) for row in rows]
    finally:
        connection.close()


def asset_count(data_root: Path) -> int:
    rows = sqlite_rows(data_root, "SELECT count(*) AS count FROM assets")
    return int(rows[0]["count"])


def run_cancel_case(case_id: str, data_root: Path, report_dir: Path) -> CaseResult:
    started = now_utc()
    helper = ROOT / "target/debug/examples/generation-check"
    if not helper.is_file() or not (data_root / "data/installation-ready.json").is_file():
        return build_case_result(
            case_id,
            "blocked",
            "generation helper or isolated installation is missing",
            started_at=started,
        )
    stage = "loading" if case_id in {"L02", "L04"} else "sampling"
    prompt = (
        "一只橙色小猫在安静窗台上，秋日暖光，简洁水彩插画。"
        if case_id in {"L02", "L04"}
        else "平静山湖、远处雪山和近处木船，清晨自然光，写实摄影。"
    )
    seed = "46" if case_id == "L04" else "" if case_id == "L02" else "47"
    before_assets = asset_count(data_root)
    command = [
        str(helper),
        "--data-dir",
        str(data_root),
        "--prompt",
        prompt,
        "--aspect",
        "1:1",
        "--seed",
        seed,
        "--cancel-at-stage",
        stage,
    ]
    first = run_command(f"{case_id}-cancel-{stage}", command, report_dir, timeout=900)
    first_log = Path(first.log_path) if first.log_path else None
    first_text = first_log.read_text(errors="replace") if first_log else ""
    submitted, first_snapshot = parse_generation_output(first_text)
    controls = generation_control_events(first_text)
    if first.exit_code != 0 or first.timed_out or not submitted or first_snapshot is None:
        return build_case_result(
            case_id,
            "failed",
            "real cancellation helper did not exit successfully with a submitted job and final snapshot",
            command=command,
            exit_code=first.exit_code,
            evidence_items=command_evidence(first),
            details={
                "timed_out": first.timed_out,
                "submitted": submitted,
                "control_events": controls,
            },
            started_at=started,
        )
    job_id = submitted[-1]["id"]
    first_job = next(
        (item for item in first_snapshot.get("jobs", []) if item.get("id") == job_id), None
    )
    attempts_before_retry = sqlite_rows(
        data_root,
        "SELECT id, token, state, plan_json, manifest_json, error FROM attempts "
        "WHERE job_id=? ORDER BY created_at, rowid",
        (job_id,),
    )
    cancelled_attempt = attempts_before_retry[-1] if attempts_before_retry else None
    after_assets = asset_count(data_root)
    problems: list[str] = []
    if not controls or controls[-1].get("stage") != stage:
        problems.append(f"real {stage} stage was not observed")
    if first_job is None or first_job.get("state") != "Cancelled":
        problems.append("job did not reach Cancelled")
    if first_job is not None and first_job.get("assetId") is not None:
        problems.append("cancelled job received a formal asset")
    if not cancelled_attempt or cancelled_attempt["state"] != "cancelled":
        problems.append("persisted cancelled Attempt is absent or non-terminal")
    if after_assets != before_assets:
        problems.append("cancellation changed the formal asset count")

    job_request = sqlite_rows(
        data_root, "SELECT request_json FROM jobs WHERE id=?", (job_id,)
    )
    frozen_seed = None
    if attempts_before_retry:
        try:
            first_plan = json.loads(attempts_before_retry[0]["plan_json"])
            frozen_seed = (first_plan.get("effective_parameters") or {}).get("seed")
        except (ValueError, TypeError):
            problems.append("cancelled Attempt plan JSON could not be read")
    if case_id == "L02":
        if not job_request:
            problems.append("job request is missing")
        else:
            try:
                requested = json.loads(job_request[0]["request_json"])
                if (requested.get("parameters") or {}).get("seed") is not None:
                    problems.append("random-seed request unexpectedly stored an explicit seed")
            except (ValueError, TypeError):
                problems.append("job request JSON could not be read")
        if type(frozen_seed) is not int or not 0 <= frozen_seed <= 0xFFFFFFFF:
            problems.append("random seed was not frozen as a u32 before cancellation")

    retry_result = None
    retry_command = None
    attempts_after = attempts_before_retry
    retry_snapshot = None
    output_evidence: dict[str, Any] | None = None
    if case_id in {"L02", "L05"}:
        retry_command = [str(helper), "--data-dir", str(data_root), "--retry-job", job_id]
        retry_result = run_command(
            f"{case_id}-explicit-retry", retry_command, report_dir, timeout=900
        )
        retry_log = Path(retry_result.log_path) if retry_result.log_path else None
        retry_text = retry_log.read_text(errors="replace") if retry_log else ""
        _, retry_snapshot = parse_generation_output(retry_text)
        attempts_after = sqlite_rows(
            data_root,
            "SELECT id, token, state, plan_json, manifest_json, error FROM attempts "
            "WHERE job_id=? ORDER BY created_at, rowid",
            (job_id,),
        )
        if retry_result.exit_code != 0 or retry_result.timed_out or retry_snapshot is None:
            problems.append("explicit retry helper failed")
        if len(attempts_after) != 2:
            problems.append("explicit retry did not create exactly one new Attempt")
        if len({row["token"] for row in attempts_after}) != len(attempts_after):
            problems.append("Attempt execution tokens are not unique")
        if attempts_after and attempts_after[0]["state"] != "cancelled":
            problems.append("original Attempt did not retain cancelled state")
        if len(attempts_after) >= 2 and attempts_after[-1]["state"] != "succeeded":
            problems.append("retry Attempt did not succeed")
        try:
            first_plan = json.loads(attempts_after[0]["plan_json"])
            retry_plan = json.loads(attempts_after[-1]["plan_json"])
            if (
                first_plan.get("effective_parameters")
                != retry_plan.get("effective_parameters")
            ):
                problems.append("retry changed frozen effective parameters")
            if case_id == "L02":
                retry_seed = (retry_plan.get("effective_parameters") or {}).get("seed")
                if retry_seed != frozen_seed:
                    problems.append("retry did not preserve the originally frozen random seed")
                if type(retry_seed) is not int or not 0 <= retry_seed <= 0xFFFFFFFF:
                    problems.append("retry random seed is not a persisted u32")
        except (ValueError, TypeError, IndexError):
            problems.append("Attempt plan JSON could not be compared")
        retry_job = None
        if retry_snapshot:
            retry_job = next(
                (item for item in retry_snapshot.get("jobs", []) if item.get("id") == job_id),
                None,
            )
        if retry_job is None or retry_job.get("state") != "Succeeded":
            problems.append("retried job did not reach Succeeded")
        elif retry_job.get("assetId") is None:
            problems.append("retried job has no formal asset")
        if retry_job and retry_job.get("manifest"):
            outputs = (retry_job.get("manifest") or {}).get("outputs") or []
            if outputs:
                relative = ((outputs[0].get("location") or {}).get("relative_path"))
                asset_path = (data_root / "data" / relative).resolve() if relative else None
                if asset_path and asset_path.is_file():
                    output_evidence = evidence(asset_path, note="retry formal PNG")
                    try:
                        validate_png(asset_path, 1024, 1024)
                    except (OSError, ValueError) as error:
                        problems.append(
                            f"retry PNG validation failed: {type(error).__name__}: {error}"
                        )
                    if sha256_file(asset_path) != outputs[0].get("content_hash"):
                        problems.append("retry PNG SHA-256 differs from manifest")
                else:
                    problems.append("retry formal PNG is missing")

    evidence_items = command_evidence(first)
    if retry_result:
        evidence_items.extend(command_evidence(retry_result))
    if output_evidence:
        evidence_items.append(output_evidence)
    details = {
        "job_id": job_id,
        "requested_stage": stage,
        "requested_seed": "random" if case_id == "L02" else seed,
        "frozen_seed": frozen_seed,
        "control_events": controls,
        "attempts": [
            {key: row[key] for key in ("id", "token", "state", "error")}
            for row in attempts_after
        ],
        "asset_count_before": before_assets,
        "asset_count_after_cancel": after_assets,
        "asset_count_after_retry": asset_count(data_root),
        "first_job": {
            "state": (first_job or {}).get("state"),
            "asset_id": (first_job or {}).get("assetId"),
        },
    }
    if retry_snapshot:
        retry_job = next(
            (item for item in retry_snapshot.get("jobs", []) if item.get("id") == job_id),
            None,
        )
        details["retry_job"] = {
            "state": (retry_job or {}).get("state"),
            "attempt_id": (retry_job or {}).get("attemptId"),
            "asset_id": (retry_job or {}).get("assetId"),
        }
    return build_case_result(
        case_id,
        "passed" if not problems else "failed",
        (
            "real sampling cancellation was cleaned up and an explicit retry succeeded"
            if case_id == "L05" and not problems
            else "random seed was frozen, cancellation was cleaned up and explicit retry succeeded"
            if case_id == "L02" and not problems
            else "real loading cancellation was cleaned up without a formal asset"
            if case_id == "L04" and not problems
            else "; ".join(problems)
        ),
        command=command,
        exit_code=0 if not problems else 1,
        evidence_items=evidence_items,
        details=details,
        started_at=started,
    )


def run_host_crash_case(data_root: Path, report_dir: Path) -> CaseResult:
    started = now_utc()
    raw_report = report_dir / "L06-real-host-crash.json"
    raw_log = report_dir / "logs/L06-real-host-crash.log"
    command = [
        sys.executable,
        "scripts/check-generation-crash.py",
        "--data-root",
        str(data_root),
        "--report",
        str(raw_report),
        "--log",
        str(raw_log),
    ]
    result = run_command(
        "L06-real-host-crash-wrapper", command, report_dir, timeout=600
    )
    record: dict[str, Any] = {}
    if raw_report.is_file():
        try:
            record = json.loads(raw_report.read_text())
        except (OSError, ValueError, TypeError):
            record = {}
    problems: list[str] = []
    if result.exit_code != 0 or result.timed_out:
        problems.append("host-crash acceptance script failed")
    if record.get("is_full_model_execution") is not True:
        problems.append("execution is not marked as full-model")
    execution = record.get("observed_execution") or {}
    if execution.get("stage") != "sampling" or (execution.get("step") or 0) < 2:
        problems.append("real sampling progress was not observed before the kill")
    if (record.get("recovered_job") or {}).get("state") != "Interrupted":
        problems.append("pre-retry job was not Interrupted")
    if record.get("execution_lease_released") is not True:
        problems.append("execution lease release was not proven")
    if record.get("execution_lease_was_busy_during_recovery") is not True:
        problems.append("old execution lease was not observed busy during recovery")
    if (record.get("retry_job") or {}).get("state") != "Succeeded":
        problems.append("explicit retry did not succeed")
    rows = record.get("attempts") or []
    if len(rows) != 2 or rows[0].get("state") != "interrupted" or rows[1].get("state") != "succeeded":
        problems.append("Attempt history did not retain Interrupted then Succeeded")
    if len({row.get("token") for row in rows}) != 2:
        problems.append("host recovery and retry did not use unique tokens")
    asset_id = (record.get("retry_job") or {}).get("asset_id")
    asset_evidence = None
    if asset_id:
        asset_rows = sqlite_rows(
            data_root,
            "SELECT relative_path,width,height,byte_count FROM assets WHERE id=?",
            (asset_id,),
        )
        if len(asset_rows) == 1:
            asset_path = (data_root / "data" / asset_rows[0]["relative_path"]).resolve()
            if asset_path.is_file():
                try:
                    validate_png(
                        asset_path, asset_rows[0]["width"], asset_rows[0]["height"]
                    )
                except (OSError, ValueError) as error:
                    problems.append(
                        f"retry PNG validation failed: {type(error).__name__}: {error}"
                    )
                asset_evidence = evidence(asset_path, note="post-crash retry PNG")
            else:
                problems.append("post-crash retry formal asset is missing")
        else:
            problems.append("post-crash retry formal asset row is missing or ambiguous")
    evidence_items = command_evidence(result) + [evidence(raw_report), evidence(raw_log)]
    if asset_evidence:
        evidence_items.append(asset_evidence)
    return build_case_result(
        "L06",
        "passed" if not problems else "failed",
        "real GPU execution was killed, the old lease was busy, and explicit retry succeeded"
        if not problems
        else "; ".join(problems),
        command=command,
        exit_code=result.exit_code,
        evidence_items=evidence_items,
        details={
            "raw_report": str(raw_report),
            "observed_execution": execution,
            "recovered_job_state": (record.get("recovered_job") or {}).get("state"),
            "lease_was_busy": record.get("execution_lease_was_busy_during_recovery"),
            "lease_released": record.get("execution_lease_released"),
            "retry_job": record.get("retry_job"),
            "attempts": rows,
            "asset": asset_evidence,
        },
        started_at=started,
    )


def run_generation_case(
    case_id: str,
    data_root: Path,
    report_dir: Path,
    *,
    network_denied: bool = False,
) -> CaseResult:
    definition = CASES[case_id]
    started = now_utc()
    helper = ROOT / "target/debug/examples/generation-check"
    supervisor = ROOT / "target/debug/inference-supervisor"
    if not helper.is_file() or not supervisor.is_file():
        return build_case_result(
            case_id,
            "blocked",
            "generation helper or native supervisor is missing",
            started_at=started,
        )
    ready = data_root / "data/installation-ready.json"
    if not ready.is_file():
        return build_case_result(
            case_id,
            "blocked",
            "isolated data root has no installation-ready record",
            started_at=started,
        )
    helper_command = [
        str(helper),
        "--data-dir",
        str(data_root),
        "--prompt",
        definition["prompt"],
        "--aspect",
        definition["aspect"],
        "--seed",
        definition["seed"],
    ]
    command = helper_command
    environment = None
    if network_denied:
        command = [
            "/usr/bin/sandbox-exec",
            "-f",
            str(ROOT / "verification/deny-network.sb"),
            "/usr/bin/env",
            "HF_HUB_OFFLINE=1",
            "TRANSFORMERS_OFFLINE=1",
            "HF_HUB_DISABLE_TELEMETRY=1",
            *helper_command,
        ]
        environment = {
            "HF_HUB_OFFLINE": "1",
            "TRANSFORMERS_OFFLINE": "1",
            "HF_HUB_DISABLE_TELEMETRY": "1",
            "DO_NOT_TRACK": "1",
        }
    result = run_command(
        f"{case_id}-real-generation",
        command,
        report_dir,
        timeout=900,
        env=environment,
    )
    log_path = Path(result.log_path) if result.log_path else None
    text = log_path.read_text(errors="replace") if log_path else ""
    submitted, snapshot = parse_generation_output(text)
    if result.exit_code != 0 or result.timed_out or not submitted or snapshot is None:
        return build_case_result(
            case_id,
            "failed",
            "generation helper did not exit successfully with a submitted job and final snapshot",
            command=command,
            exit_code=result.exit_code,
            evidence_items=command_evidence(result),
            details={
                "timed_out": result.timed_out,
                "network_denied": network_denied,
                "submitted_jobs": submitted,
                "final_snapshot_observed": snapshot is not None,
            },
            started_at=started,
        )
    job_id = submitted[-1].get("id")
    job = next((item for item in snapshot.get("jobs", []) if item.get("id") == job_id), None)
    asset = next(
        (item for item in snapshot.get("assets", []) if item.get("id") == (job or {}).get("assetId")),
        None,
    )
    problems: list[str] = []
    details: dict[str, Any] = {"job_id": job_id, "network_denied": network_denied}
    if job is None:
        problems.append("submitted job is absent from final snapshot")
    else:
        details["job"] = {
            "state": job.get("state"),
            "attempt_id": job.get("attemptId"),
            "asset_id": job.get("assetId"),
            "width": job.get("width"),
            "height": job.get("height"),
            "seed": job.get("seed"),
        }
        manifest = job.get("manifest") or {}
        plan = manifest.get("plan") or {}
        parameters = plan.get("effective_parameters") or {}
        output = plan.get("effective_output") or {}
        request = manifest.get("request") or {}
        if job.get("state") != "Succeeded":
            problems.append("job state is not Succeeded")
        if manifest.get("evidence_kind") != "execution_record":
            problems.append("manifest evidence_kind is not execution_record")
        if (manifest.get("final_result") or {}).get("status") != "succeeded":
            problems.append("manifest final_result is not succeeded")
        if request.get("instruction") != definition["prompt"]:
            problems.append("request prompt differs from fixed case input")
        if parameters.get("seed") != int(definition["seed"]):
            problems.append("effective seed differs from fixed case input")
        if parameters.get("steps") != 9 or parameters.get("quantization") != 4:
            problems.append("effective recipe differs from q4/9-step")
        if parameters.get("scheduler") != "linear" or parameters.get("guidance") != 0.0:
            problems.append("scheduler or guidance differs from fixed recipe")
        if (
            output.get("width") != definition["width"]
            or output.get("height") != definition["height"]
            or output.get("count") != 1
        ):
            problems.append("effective output dimensions/count differ from fixed input")
        details["manifest"] = {
            "evidence_kind": manifest.get("evidence_kind"),
            "effective_parameters": parameters,
            "effective_output": output,
            "runtime_package_id": (plan.get("runtime") or {}).get("package_id"),
            "device": plan.get("device"),
        }
    if asset is None:
        problems.append("final asset projection is missing")
    else:
        details["asset"] = {
            "id": asset.get("id"),
            "available": asset.get("available"),
            "bytes": asset.get("bytes"),
            "width": asset.get("width"),
            "height": asset.get("height"),
        }
        if asset.get("available") is not True:
            problems.append("asset is not available")
    output_location = None
    if job is not None:
        outputs = (job.get("manifest") or {}).get("outputs") or []
        if outputs:
            output_location = outputs[0]
    if output_location is None:
        problems.append("manifest has no formal output")
    else:
        relative = ((output_location.get("location") or {}).get("relative_path"))
        expected_hash = output_location.get("content_hash")
        asset_path = (data_root / "data" / relative).resolve() if relative else None
        if asset_path is None or not asset_path.is_file():
            problems.append("formal output asset file is missing")
        else:
            actual_hash = sha256_file(asset_path)
            details["output"] = {
                "asset_id": output_location.get("asset_id"),
                "path": str(asset_path),
                "sha256": actual_hash,
                "manifest_content_hash": expected_hash,
            }
            if actual_hash != expected_hash:
                problems.append("formal output SHA-256 differs from manifest")
            try:
                details["png"] = validate_png(
                    asset_path, definition["width"], definition["height"]
                )
            except (OSError, ValueError) as error:
                problems.append(f"PNG validation failed: {type(error).__name__}: {error}")
    evidence_items = command_evidence(result)
    if details.get("output"):
        evidence_items.append(
            evidence(Path(details["output"]["path"]), note="formal generated PNG")
        )
    return build_case_result(
        case_id,
        "passed" if not problems else "failed",
        "fixed input produced a decodable formal PNG with a successful execution record"
        if not problems
        else "; ".join(problems),
        command=command,
        exit_code=result.exit_code,
        evidence_items=evidence_items,
        details=details,
        started_at=started,
    )


def clone_download_cache(source: Path, destination: Path) -> dict[str, Any]:
    destination.mkdir(parents=True, exist_ok=False)
    copied = 0
    cloned = 0
    total_bytes = 0
    for item in sorted(source.iterdir()):
        if (
            not item.is_file()
            or item.is_symlink()
            or not re.fullmatch(r"[0-9a-f]{64}", item.name)
        ):
            continue
        target = destination / item.name
        if hasattr(os, "clonefile"):
            os.clonefile(item, target)
            cloned += 1
        else:
            shutil.copyfile(item, target)
        copied += 1
        total_bytes += item.stat().st_size
    return {
        "source": str(source),
        "destination": str(destination),
        "files": copied,
        "cloned_files": cloned,
        "bytes": total_bytes,
    }


def run_cache_install_case(data_root: Path, report_dir: Path) -> CaseResult:
    started = now_utc()
    helper = ROOT / "target/debug/examples/generation-check"
    source_cache = ROOT / "target/first-generation-validation/cache/downloads"
    destination_cache = data_root / "cache/downloads"
    if not helper.is_file() or not source_cache.is_dir():
        return build_case_result(
            "I02",
            "blocked",
            "generation helper or verified source cache is missing",
            started_at=started,
        )
    if (data_root / "data/installation-ready.json").exists():
        return build_case_result(
            "I02",
            "not_run",
            "cache-install case requires a fresh isolated root without an active installation",
            started_at=started,
        )
    (data_root / "cache").mkdir(parents=True, exist_ok=True)
    clone = clone_download_cache(source_cache, destination_cache)
    model_manifest = json.loads(
        (ROOT / "resources/local-generation/model-manifest.json").read_text()
    )
    runtime_manifest = json.loads(
        (ROOT / "resources/local-generation/runtime-manifest.json").read_text()
    )
    required = [
        *model_manifest["files"],
        runtime_manifest["interpreter"],
        *runtime_manifest["wheels"],
    ]
    missing = [
        item["path"]
        for item in required
        if not (destination_cache / item["sha256"]).is_file()
        or (destination_cache / item["sha256"]).stat().st_size != item["bytes"]
    ]
    if missing:
        return build_case_result(
            "I02",
            "failed",
            f"cloned download cache is incomplete: {len(missing)} missing or size-mismatched artifacts",
            details={"cache_clone": clone, "missing": missing[:20]},
            started_at=started,
        )
    helper_command = [str(helper), "--data-dir", str(data_root), "--install"]
    command = [
        "/usr/bin/sandbox-exec",
        "-f",
        str(ROOT / "verification/deny-network.sb"),
        "/usr/bin/env",
        "HF_HUB_OFFLINE=1",
        "TRANSFORMERS_OFFLINE=1",
        "HF_HUB_DISABLE_TELEMETRY=1",
        *helper_command,
    ]
    result = run_command(
        "I02-offline-cache-install",
        command,
        report_dir,
        timeout=900,
        env={
            "HF_HUB_OFFLINE": "1",
            "TRANSFORMERS_OFFLINE": "1",
            "HF_HUB_DISABLE_TELEMETRY": "1",
            "DO_NOT_TRACK": "1",
        },
    )
    log_path = Path(result.log_path) if result.log_path else None
    text = log_path.read_text(errors="replace") if log_path else ""
    _, snapshot = parse_generation_output(text)
    model = (snapshot or {}).get("model") or {}
    ready_record = data_root / "data/installation-ready.json"
    ready = None
    if ready_record.is_file():
        try:
            ready = json.loads(ready_record.read_text())
        except (OSError, ValueError):
            ready = None
    problems = []
    if result.exit_code != 0 or result.timed_out:
        problems.append("offline cache installation failed")
    if not model.get("ready"):
        problems.append("model did not become ready")
    if not model.get("runtimeReady") or not model.get("modelReady") or not model.get("deviceVerified"):
        problems.append("runtime, model and device readiness were not all true")
    if not ready:
        problems.append("installation-ready record is missing or invalid")
    elif (
        ready.get("runtime_manifest_sha256") != sha256_file(
            ROOT / "resources/local-generation/runtime-manifest.json"
        )
        or ready.get("model_manifest_sha256")
        != sha256_file(ROOT / "resources/local-generation/model-manifest.json")
        or (ready.get("probe") or {}).get("device") != "gpu"
    ):
        problems.append("installation-ready identity or GPU probe differs from fixed manifests")
    return build_case_result(
        "I02",
        "passed" if not problems else "failed",
        "verified cache installed the private runtime and model with networking denied"
        if not problems
        else "; ".join(problems),
        command=command,
        exit_code=result.exit_code,
        evidence_items=command_evidence(result) + [evidence(ready_record)],
        details={
            "cache_clone": clone,
            "required_artifacts": len(required),
            "missing_artifacts": missing,
            "network_denied": True,
            "runtime_rebuilt_from_cache": True,
            "model": model,
            "installation": {
                "runtime_lock_id": ready.get("runtime_lock_id"),
                "model_snapshot_id": ready.get("model_snapshot_id"),
                "runtime_manifest_sha256": ready.get("runtime_manifest_sha256"),
                "model_manifest_sha256": ready.get("model_manifest_sha256"),
                "runtime_file_count": len(ready.get("runtime_files") or []),
                "probe": ready.get("probe"),
            }
            if ready
            else None,
        },
        started_at=started,
    )


def run_missing_component_case(source_root: Path, report_dir: Path) -> CaseResult:
    started = now_utc()
    helper = ROOT / "target/debug/examples/generation-check"
    ready = source_root / "data/installation-ready.json"
    if not helper.is_file() or not ready.is_file():
        return build_case_result(
            "I05",
            "blocked",
            "generation helper or installed source root is missing",
            started_at=started,
        )
    fault_root = report_dir / "I05-missing-component-root"
    if fault_root.exists():
        return build_case_result(
            "I05",
            "not_run",
            "I05 fault root already exists; use a fresh report directory",
            started_at=started,
        )
    fault_root.mkdir(parents=True)
    clone = run_command(
        "I05-clone-installed-root",
        ["/bin/cp", "-cPR", f"{source_root}/.", str(fault_root)],
        report_dir,
        timeout=300,
    )
    if clone.exit_code != 0:
        return build_case_result(
            "I05",
            "failed",
            "could not create an APFS-cloned isolated fault root",
            command=clone.command,
            exit_code=clone.exit_code,
            evidence_items=command_evidence(clone),
            started_at=started,
        )
    manifest = json.loads(
        (ROOT / "resources/local-generation/model-manifest.json").read_text()
    )
    model_id = json.loads(ready.read_text())["model_snapshot_id"]
    model_root = fault_root / "data/models" / model_id
    moved_artifact = next(
        item
        for item in reversed(manifest["files"])
        if item["path"].endswith(".safetensors")
    )
    source_artifact = model_root / moved_artifact["path"]
    quarantine = fault_root / "work/missing-component"
    quarantine.mkdir(parents=True, exist_ok=True)
    moved_target = quarantine / Path(moved_artifact["path"]).name
    if not source_artifact.is_file() or moved_target.exists():
        return build_case_result(
            "I05",
            "failed",
            "expected model component is missing before fault injection or quarantine target exists",
            evidence_items=command_evidence(clone),
            details={"component": moved_artifact["path"]},
            started_at=started,
        )
    source_artifact.replace(moved_target)

    before_assets = asset_count(fault_root)
    before_jobs = int(sqlite_rows(fault_root, "SELECT count(*) AS count FROM jobs")[0]["count"])
    before_attempts = int(
        sqlite_rows(fault_root, "SELECT count(*) AS count FROM attempts")[0]["count"]
    )
    before_runs = len(list((fault_root / "data/runs").iterdir()))
    existing = sqlite_rows(
        fault_root,
        "SELECT id, relative_path FROM assets ORDER BY created_at, rowid LIMIT 1",
    )
    existing_path = (
        (fault_root / "data" / existing[0]["relative_path"]).resolve() if existing else None
    )
    existing_hash = sha256_file(existing_path) if existing_path and existing_path.is_file() else None

    command = [
        str(helper),
        "--data-dir",
        str(fault_root),
        "--prompt",
        "一朵白色小花放在木桌上，柔和晨光，简洁静物摄影。",
        "--aspect",
        "1:1",
        "--seed",
        "49",
    ]
    result = run_command(
        "I05-missing-component-generation", command, report_dir, timeout=180
    )
    log_path = Path(result.log_path) if result.log_path else None
    log_text = log_path.read_text(errors="replace") if log_path else ""
    after_assets = asset_count(fault_root)
    after_jobs = int(sqlite_rows(fault_root, "SELECT count(*) AS count FROM jobs")[0]["count"])
    after_attempts = int(
        sqlite_rows(fault_root, "SELECT count(*) AS count FROM attempts")[0]["count"]
    )
    after_runs = len(list((fault_root / "data/runs").iterdir()))
    after_hash = sha256_file(existing_path) if existing_path and existing_path.is_file() else None
    problems = []
    if result.exit_code == 0:
        problems.append("generation unexpectedly succeeded")
    if "文件缺失" not in log_text and "安装文件缺失" not in log_text and "大小不符" not in log_text:
        problems.append("missing-component rejection message was not observed")
    if (after_assets, after_jobs, after_attempts, after_runs) != (
        before_assets,
        before_jobs,
        before_attempts,
        before_runs,
    ):
        problems.append("missing-component submission changed jobs, attempts, runs or formal assets")
    if existing_hash is None or after_hash != existing_hash:
        problems.append("pre-existing formal asset was not readable and unchanged")
    return build_case_result(
        "I05",
        "passed" if not problems else "failed",
        "missing component was rejected before execution while existing assets stayed readable"
        if not problems
        else "; ".join(problems),
        command=command,
        exit_code=result.exit_code,
        evidence_items=command_evidence(clone)
        + command_evidence(result)
        + ([evidence(existing_path, note="pre-existing formal asset")]
           if existing_path
           else []),
        details={
            "source_root": str(source_root),
            "fault_root": str(fault_root),
            "fault_injection": {
                "component": moved_artifact["path"],
                "moved_to": str(moved_target),
                "recoverable": True,
            },
            "counts_before": {
                "assets": before_assets,
                "jobs": before_jobs,
                "attempts": before_attempts,
                "runs": before_runs,
            },
            "counts_after": {
                "assets": after_assets,
                "jobs": after_jobs,
                "attempts": after_attempts,
                "runs": after_runs,
            },
            "existing_asset_sha256": after_hash,
            "rejection_observed": not problems,
        },
        started_at=started,
    )


def run_model_case(
    case_id: str,
    data_root: Path,
    report_dir: Path,
    *,
    install_mode: str,
) -> CaseResult:
    recorded = {
        "L01": ROOT / "verification/model-usability-20261031/L01-native-continuous.json",
        "L03": ROOT / "verification/model-usability-20261031/L03-native-rapid-click-failed.json",
        "S02": ROOT / "verification/model-usability-20261031/S02-offline-no-model.json",
    }
    if case_id == "I01":
        passed = ROOT / "verification/model-usability-20261031/I01-network-install-passed.json"
        blocked = ROOT / "verification/model-usability-20261031/I01-network-install-blocked.json"
        if passed.is_file() or not blocked.is_file():
            recorded["I01"] = passed
        else:
            recorded["I01"] = blocked
    if case_id in recorded and recorded[case_id].is_file():
        source = recorded[case_id]
        try:
            detail = json.loads(source.read_text())
        except (OSError, ValueError):
            detail = {}
        status = detail.get("status")
        if status not in VALID_STATES:
            status = "failed"
        evidence_paths = detail.get("evidence_paths") or []
        evidence_items = [evidence(source, note="recorded native/model evidence")]
        for value in evidence_paths:
            path = Path(value)
            if not path.is_absolute():
                path = ROOT / path
            evidence_items.append(evidence(path))
        return build_case_result(
            case_id,
            status,
            detail.get("reason")
            or detail.get("ui_result")
            or detail.get("observations")
            or "recorded evidence reviewed",
            evidence_items=evidence_items,
            details={"recorded_evidence": str(source), **detail},
        )
    if case_id in {f"G{index:02}" for index in range(1, 7)}:
        return run_generation_case(case_id, data_root, report_dir)
    if case_id == "I02":
        if install_mode != "cache":
            return build_case_result(
                "I02",
                "not_run",
                "cache install requires --install-mode cache and a fresh isolated root",
            )
        return run_cache_install_case(data_root, report_dir)
    if case_id == "I03":
        return run_generation_case(
            "I03", data_root, report_dir, network_denied=True
        )
    if case_id == "I05":
        return run_missing_component_case(data_root, report_dir)
    if case_id in {"L02", "L04", "L05"}:
        return run_cancel_case(case_id, data_root, report_dir)
    if case_id == "L06":
        return run_host_crash_case(data_root, report_dir)
    return build_case_result(
        case_id,
        "not_run",
        "this model case requires a dedicated lifecycle or installation executor",
    )


def read_daily_manifest(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text())
    except OSError as error:
        raise ValueError(f"daily evidence manifest cannot be read: {error}") from error
    except ValueError as error:
        raise ValueError(f"daily evidence manifest is not valid JSON: {error}") from error
    if value.get("schema_version") != 1:
        raise ValueError("daily evidence manifest schema_version must be 1")
    if not isinstance(value.get("cases"), dict):
        raise ValueError("daily evidence manifest requires a cases object")
    return value


def read_quality_review(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text())
    except OSError as error:
        raise ValueError(f"quality review cannot be read: {error}") from error
    except ValueError as error:
        raise ValueError(f"quality review is not valid JSON: {error}") from error
    if value.get("schema_version") != 1:
        raise ValueError("quality review schema_version must be 1")
    if value.get("status") not in {"passed", "failed", "incomplete"}:
        raise ValueError("quality review status must be passed, failed, or incomplete")
    if not isinstance(value.get("meets_quality_bar"), bool):
        raise ValueError("quality review requires a boolean meets_quality_bar")
    cases = value.get("cases")
    if not isinstance(cases, dict):
        raise ValueError("quality review requires a cases object")
    missing = sorted(QUALITY_CASES - set(cases))
    if missing:
        raise ValueError("quality review is missing cases: " + ", ".join(missing))
    for case_id in sorted(QUALITY_CASES):
        review = cases.get(case_id)
        if not isinstance(review, dict) or review.get("status") not in VALID_STATES:
            raise ValueError(f"quality review case {case_id} has an invalid status")
    if value["status"] == "passed" and not value["meets_quality_bar"]:
        raise ValueError("a passed quality review must meet the quality bar")
    return value


def apply_quality_gate(
    summary: dict[str, Any],
    selected: Iterable[str],
    review: dict[str, Any] | None,
) -> dict[str, Any]:
    if not QUALITY_CASES.intersection(selected):
        return summary
    gated = {**summary}
    if review is None:
        gated["overall"] = "incomplete"
    elif review["status"] == "failed" or not review["meets_quality_bar"]:
        gated["overall"] = "failed"
    elif review["status"] != "passed":
        gated["overall"] = "incomplete"
    return gated


def daily_case(
    case_id: str,
    manifest: dict[str, Any] | None,
    manifest_path: Path | None,
) -> CaseResult:
    started = now_utc()
    definition = CASES[case_id]
    if manifest is None:
        return build_case_result(
            case_id,
            "not_run",
            "native daily evidence manifest was not supplied",
            started_at=started,
        )
    raw = manifest["cases"].get(case_id)
    if not isinstance(raw, dict):
        return build_case_result(
            case_id,
            "not_run",
            "case is missing from native daily evidence manifest",
            evidence_items=[evidence(manifest_path)] if manifest_path else [],
            started_at=started,
        )
    status = raw.get("status", "not_run")
    if status not in VALID_STATES:
        return build_case_result(
            case_id,
            "failed",
            f"manifest contains unsupported status {status!r}",
            evidence_items=[evidence(manifest_path)] if manifest_path else [],
            started_at=started,
        )
    evidence_kind = raw.get("evidence_kind")
    if evidence_kind != definition["evidence_kind"]:
        return build_case_result(
            case_id,
            "failed",
            f"manifest evidence_kind {evidence_kind!r} does not match {definition['evidence_kind']!r}",
            evidence_items=[evidence(manifest_path)] if manifest_path else [],
            started_at=started,
        )
    paths = raw.get("evidence_paths") or []
    evidence_items = [evidence(manifest_path)] if manifest_path else []
    missing_paths = []
    for item in paths:
        candidate = Path(item).expanduser()
        if not candidate.is_absolute():
            candidate = ROOT / candidate
        record = evidence(candidate)
        evidence_items.append(record)
        if not record["exists"]:
            missing_paths.append(str(candidate))
    observations = raw.get("observations")
    if status == "passed" and (
        not paths or missing_paths or not isinstance(observations, str) or len(observations.strip()) < 20
    ):
        return build_case_result(
            case_id,
            "failed",
            "passed native case lacks concrete observations or existing evidence files",
            evidence_items=evidence_items,
            details={
                "missing_evidence_paths": missing_paths,
                "observations": observations,
            },
            started_at=started,
        )
    return build_case_result(
        case_id,
        status,
        raw.get("reason") or ("native evidence reviewed" if status == "passed" else ""),
        evidence_items=evidence_items,
        details={
            "observations": observations,
            "operation_method": raw.get("operation_method"),
            "logical_window": raw.get("logical_window"),
            "theme": raw.get("theme"),
        },
        started_at=started,
    )


def summarize(results: list[CaseResult]) -> dict[str, Any]:
    counts = {state: 0 for state in VALID_STATES}
    required_counts = {state: 0 for state in VALID_STATES}
    for result in results:
        counts[result.status] += 1
        if CASES[result.case_id]["priority"] == "P0":
            required_counts[result.status] += 1
    if required_counts["failed"]:
        overall = "failed"
    elif required_counts["not_run"] or required_counts["blocked"]:
        overall = "incomplete"
    else:
        overall = "passed"
    return {
        "case_count": len(results),
        "counts": counts,
        "required_counts": required_counts,
        "overall": overall,
    }


def render_markdown(report: dict[str, Any]) -> str:
    summary = report["summary"]
    lines = [
        f"# 模型可用性验收报告 `{report['run_id']}`",
        "",
        f"- Profile：`{report['profile']}`",
        f"- 结论：**{summary['overall']}**",
        f"- 通过 / 失败 / 未运行 / 阻塞：{summary['counts']['passed']} / "
        f"{summary['counts']['failed']} / {summary['counts']['not_run']} / "
        f"{summary['counts']['blocked']}",
        f"- 数据根：`{report['data_root']['path']}`（{report['data_root']['category']}）",
        "",
        "| 用例 | 优先级 | 状态 | 证据类别 | 结论或原因 |",
        "|---|---|---|---|---|",
    ]
    for item in report["cases"]:
        reason = item["reason"].replace("|", "\\|").replace("\n", " ")
        lines.append(
            f"| {item['case_id']} | {item['priority']} | {item['status']} | "
            f"{item['evidence_kind']} | {reason} |"
        )
    lines.extend(
        [
            "",
            "## 证据与口径",
            "",
            "- 命令输出保存在同目录 `logs/`，每个证据文件在 JSON 中记录 SHA-256。",
            "- 默认忽略测试、历史记录和计划模型不计入当前通过数。",
            "- 存在任何 P0 失败时整体为 failed；否则存在 P0 未运行或阻塞时为 incomplete。",
            f"- 人工质量审阅：{report['human_review']['status']}。",
            "",
        ]
    )
    return "\n".join(lines)


def parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=sorted(PROFILE_CASES), required=True)
    parser.add_argument("--data-root")
    parser.add_argument("--report-dir")
    parser.add_argument("--case-id", action="append", default=[])
    parser.add_argument(
        "--install-mode",
        choices=["none", "cache", "network"],
        default="none",
        help="installation intent; no download occurs without cache/network",
    )
    parser.add_argument(
        "--daily-evidence",
        help="schema-v1 JSON manifest containing native daily case evidence",
    )
    parser.add_argument(
        "--quality-review",
        help="schema-v1 JSON visual review for G01-G06",
    )
    return parser.parse_args(argv)


def run(argv: Iterable[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        report_dir = validate_report_dir(args.report_dir) if args.report_dir else None
        data_root = validate_isolated_root(args.data_root) if args.data_root else None
    except ValueError as error:
        print(f"error: {error}", file=sys.stderr)
        return 3
    if report_dir is None:
        print("error: --report-dir is required", file=sys.stderr)
        return 3
    if args.profile != "daily" and data_root is None:
        print(f"error: --data-root is required for {args.profile}", file=sys.stderr)
        return 3
    if args.profile == "daily" and data_root is not None:
        print("error: daily profile records the actual default root and does not accept --data-root", file=sys.stderr)
        return 3
    if args.install_mode != "none" and args.profile != "model":
        print("error: --install-mode is only valid with the model profile", file=sys.stderr)
        return 3
    if args.quality_review and args.profile != "model":
        print("error: --quality-review is only valid with the model profile", file=sys.stderr)
        return 3
    if args.profile == "model" and args.install_mode == "none" and data_root and not data_root.exists():
        print("error: model profile with no install mode requires an existing isolated root", file=sys.stderr)
        return 3
    if args.case_id:
        selected = args.case_id
        unknown = sorted(set(selected) - CASES.keys())
        if unknown:
            print(f"error: unknown case IDs: {', '.join(unknown)}", file=sys.stderr)
            return 3
        invalid_profile = [
            case_id for case_id in selected if CASES[case_id]["profile"] != args.profile
        ]
        if invalid_profile:
            print(
                "error: case/profile mismatch: " + ", ".join(sorted(invalid_profile)),
                file=sys.stderr,
            )
            return 3
    else:
        selected = PROFILE_CASES[args.profile]
    selected = list(dict.fromkeys(selected))

    report_dir.mkdir(parents=True, exist_ok=True)
    run_id = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ") + "-" + uuid.uuid4().hex[:8]
    started_at = now_utc()
    before_root = root_snapshot(data_root if data_root else DEFAULT_DATA_ROOT)
    results: list[CaseResult] = []

    if args.profile == "contracts":
        assert data_root is not None
        executors = {
            "C01": lambda: run_contract_c01(report_dir),
            "C02": lambda: run_contract_c02(report_dir),
            "C03": lambda: run_contract_c03(report_dir),
            "C04": lambda: run_contract_c04(report_dir),
            "I04": lambda: run_installation_contracts("I04", report_dir),
            "I06": lambda: run_installation_contracts("I06", report_dir),
            "L07": lambda: run_lifecycle_contracts("L07", report_dir),
            "L08": lambda: run_contract_l08(data_root, report_dir),
            "L09": lambda: run_lifecycle_contracts("L09", report_dir),
        }
        for case_id in selected:
            print(f"[{case_id}] running", flush=True)
            result = executors[case_id]()
            results.append(result)
            print(f"[{case_id}] {result.status}: {result.reason}", flush=True)
    elif args.profile == "model":
        assert data_root is not None
        for case_id in selected:
            print(f"[{case_id}] running", flush=True)
            result = run_model_case(
                case_id,
                data_root,
                report_dir,
                install_mode=args.install_mode,
            )
            results.append(result)
            print(f"[{case_id}] {result.status}: {result.reason}", flush=True)
    else:
        manifest_path = Path(args.daily_evidence).expanduser().resolve() if args.daily_evidence else None
        try:
            manifest = read_daily_manifest(manifest_path) if manifest_path else None
        except ValueError as error:
            print(f"error: {error}", file=sys.stderr)
            return 3
        for case_id in selected:
            result = daily_case(case_id, manifest, manifest_path)
            results.append(result)
            print(f"[{case_id}] {result.status}: {result.reason}", flush=True)

    ending_root = root_snapshot(data_root if data_root else DEFAULT_DATA_ROOT)
    summary = summarize(results)
    quality_review = None
    if args.quality_review:
        quality_path = Path(args.quality_review).expanduser().resolve()
        try:
            quality_review = read_quality_review(quality_path)
        except ValueError as error:
            print(f"error: {error}", file=sys.stderr)
            return 3
    summary = apply_quality_gate(summary, selected, quality_review)
    human_review = {
        "status": "not_run",
        "reviewer": None,
        "meets_quality_bar": None,
        "notes": "Behavioral reports do not replace visual quality review.",
    }
    if quality_review is not None:
        human_review = {
            "status": quality_review["status"],
            "reviewer": quality_review.get("reviewer"),
            "reviewed_at": quality_review.get("reviewed_at"),
            "meets_quality_bar": quality_review["meets_quality_bar"],
            "notes": quality_review.get("summary"),
            "evidence": quality_review.get("contact_sheet"),
            "cases": quality_review["cases"],
        }
    report = {
        "schema_version": REPORT_SCHEMA_VERSION,
        "run_id": run_id,
        "started_at": started_at,
        "completed_at": now_utc(),
        "timezone": time.tzname,
        "profile": args.profile,
        "case_set": {
            "version": CASE_SET_VERSION,
            "path": str(TEST_PLAN.relative_to(ROOT)),
            "sha256": sha256_file(TEST_PLAN) if TEST_PLAN.is_file() else None,
            "selected_cases": selected,
        },
        "source_identity": source_identity(),
        "environment": environment_identity(),
        "data_root": {
            **ending_root,
            "before": before_root,
            "changed_normal_operations_only": before_root.get("installation") != ending_root.get("installation"),
        },
        "installation_mode": args.install_mode,
        "network_conditions": {
            "declared_install_mode": args.install_mode,
            "per_case_network_limits_are_recorded_in_details": True,
        },
        "cases": [result.as_dict(CASES[result.case_id]) for result in results],
        "summary": summary,
        "human_review": human_review,
    }
    json_path = report_dir / f"{args.profile}-model-usability-{run_id}.json"
    markdown_path = report_dir / f"{args.profile}-model-usability-{run_id}.md"
    atomic_write(json_path, json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    atomic_write(markdown_path, render_markdown(report))
    print(json_path)
    print(markdown_path)
    if summary["overall"] == "passed":
        return 0
    if summary["overall"] == "failed":
        return 1
    return 2


def main() -> None:
    raise SystemExit(run())


if __name__ == "__main__":
    main()
