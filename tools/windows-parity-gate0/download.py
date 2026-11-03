"""Resumable, hash-verified transfer of the fixed Windows model-parity snapshots.

The model manifests under ``resources/planned-models/`` are the single source of
truth for repository, revision, per-file URL, byte count and SHA-256. This tool:

* resumes interrupted transfers through ``.part`` files and HTTP Range requests;
* verifies every file's byte count and SHA-256 against the manifest;
* can transfer a Hugging Face-pinned snapshot from the ModelScope mirror of the
  same repository, but only after proving the mirror file's SHA-256 equals the
  manifest SHA-256 (the fixed manifest stays authoritative);
* writes a machine-readable transfer report with real source, timing and
  integrity data.

Experiment-only tooling for OpenSpec change ``achieve-windows-model-parity``
(tasks.md 1.1, bd issue image-studio-xwh.1). It is not part of the product.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import socket
import subprocess
import sys
import threading
import time
import urllib.parse
import urllib.request
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path, PurePosixPath

CHUNK_BYTES = 8 * 1024 * 1024
USER_AGENT = "ImageStudio-ParityGate0/1"
MODELSCOPE_API = (
    "https://modelscope.cn/api/v1/models/{repo}/repo/files"
    "?Revision={revision}&Recursive=true"
)
MODELSCOPE_FILE = "https://www.modelscope.cn/models/{repo}/resolve/{revision}/{path}"

_LOCK = threading.Lock()


def log(message: str) -> None:
    stamp = time.strftime("%H:%M:%S")
    with _LOCK:
        print(f"[{stamp}] {message}", flush=True)


def utc_now() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(CHUNK_BYTES), b""):
            digest.update(chunk)
    return digest.hexdigest()


def safe_target(root: Path, relative: str) -> Path:
    parts = PurePosixPath(relative)
    if parts.is_absolute() or ".." in parts.parts or not parts.parts:
        raise RuntimeError(f"unsafe manifest path: {relative!r}")
    return root.joinpath(*parts.parts)


def disk_free_bytes(path: Path) -> int:
    return shutil.disk_usage(path).free


def fetch_json(url: str, timeout: float = 120.0) -> dict:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return json.loads(response.read().decode("utf-8"))


def modelscope_revision_map(repo: str, revision: str) -> dict[str, dict]:
    """Return path -> {revision, sha256, size} for a ModelScope mirror repo."""
    url = MODELSCOPE_API.format(
        repo=urllib.parse.quote(repo, safe="/"), revision=urllib.parse.quote(revision)
    )
    payload = fetch_json(url)
    files = payload.get("Data", {}).get("Files")
    if not isinstance(files, list):
        raise RuntimeError(f"unexpected ModelScope listing payload for {repo}@{revision}")
    mapping: dict[str, dict] = {}
    for item in files:
        if item.get("Type") != "blob":
            continue
        mapping[str(item["Path"])] = {
            "revision": str(item.get("Revision") or revision),
            "sha256": str(item.get("Sha256") or "").lower(),
            "size": int(item.get("Size") or 0),
        }
    return mapping


def resolve_sources(
    manifest: dict,
    source: str,
    modelscope_repo: str | None,
    modelscope_revision: str,
) -> tuple[dict[str, dict], dict | None]:
    """Return path -> source info for every manifest file, plus mirror checks."""
    files = manifest["files"]
    if source == "manifest":
        resolved = {}
        for artifact in files:
            path = str(artifact["path"])
            url = str(artifact["url"])
            if "modelscope.cn" in url:
                kind = "modelscope"
            elif "huggingface.co" in url:
                kind = "huggingface"
            elif "github.com" in url or "githubusercontent.com" in url:
                kind = "github_release"
            else:
                kind = "manifest_url"
            revision = artifact.get("source_revision")
            resolved[path] = {
                "url": url,
                "source_kind": kind,
                "source_revision": None if revision is None else str(revision),
                "declared_url": url,
            }
        return resolved, None

    if source != "modelscope-mirror":
        raise RuntimeError(f"unknown source: {source}")
    if not modelscope_repo:
        raise RuntimeError("--source modelscope-mirror requires --modelscope-repo")
    mirror = modelscope_revision_map(modelscope_repo, modelscope_revision)
    resolved = {}
    missing = []
    mismatched = []
    checks = []
    for artifact in files:
        path = str(artifact["path"])
        expected_bytes = int(artifact["bytes"])
        expected_sha = str(artifact["sha256"]).lower()
        entry = mirror.get(path)
        if entry is None:
            missing.append(path)
            checks.append(
                {
                    "path": path,
                    "manifest_bytes": expected_bytes,
                    "manifest_sha256": expected_sha,
                    "mirror_bytes": None,
                    "mirror_sha256": None,
                    "present_in_mirror": False,
                    "bytes_match": False,
                    "sha256_match": False,
                }
            )
            continue
        bytes_match = entry["size"] == expected_bytes
        sha_match = bool(entry["sha256"]) and entry["sha256"] == expected_sha
        checks.append(
            {
                "path": path,
                "manifest_bytes": expected_bytes,
                "manifest_sha256": expected_sha,
                "mirror_bytes": entry["size"],
                "mirror_sha256": entry["sha256"],
                "mirror_revision": entry["revision"],
                "present_in_mirror": True,
                "bytes_match": bytes_match,
                "sha256_match": sha_match,
            }
        )
        if not (bytes_match and sha_match):
            mismatched.append(checks[-1])
            continue
        resolved[path] = {
            "url": MODELSCOPE_FILE.format(
                repo=urllib.parse.quote(modelscope_repo, safe="/"),
                revision=urllib.parse.quote(entry["revision"]),
                path=urllib.parse.quote(path),
            ),
            "source_kind": "modelscope_mirror",
            "source_revision": entry["revision"],
            "declared_url": str(artifact["url"]),
        }
    if missing:
        raise RuntimeError(
            "ModelScope mirror is missing manifest files: " + ", ".join(missing)
        )
    if mismatched:
        raise RuntimeError(
            "ModelScope mirror files do not match the fixed manifest: "
            + json.dumps(mismatched, ensure_ascii=False)
        )
    mirror_info = {
        "repository": modelscope_repo,
        "listing_revision": modelscope_revision,
        "mirror_file_count": len(checks),
        "all_manifest_files_present": True,
        "all_mirror_bytes_match_manifest": True,
        "all_mirror_sha256_match_manifest": True,
        "checks": checks,
    }
    return resolved, mirror_info


def probe_endpoint(url: str, timeout_seconds: float = 30.0) -> dict:
    """Probe an HTTP endpoint with curl and return reachability data."""
    curl = "curl.exe" if os.name == "nt" else "curl"
    command = [
        curl,
        "--location",
        "--silent",
        "--show-error",
        "--max-time",
        str(int(timeout_seconds)),
        "--connect-timeout",
        "10",
        "--output",
        os.devnull,
        "--user-agent",
        USER_AGENT,
        "--write-out",
        "%{http_code}\\t%{time_total}",
        url,
    ]
    started = time.monotonic()
    result = subprocess.run(
        command, capture_output=True, text=True, errors="replace", check=False
    )
    elapsed = time.monotonic() - started
    http_code = None
    time_total = None
    if result.stdout.strip():
        fields = result.stdout.strip().splitlines()[-1].split("\t")
        if len(fields) >= 2:
            try:
                http_code = int(fields[0])
                time_total = float(fields[1])
            except ValueError:
                pass
    return {
        "url": url,
        "http_code": http_code,
        "curl_exit_code": result.returncode,
        "elapsed_seconds": round(elapsed, 3),
        "curl_time_total_seconds": time_total,
        "reachable": result.returncode == 0 and http_code in (200, 206),
        "error": (result.stderr or "").strip()[:500] or None,
    }



def curl_download(
    url: str,
    partial: Path,
    expected_bytes: int,
    attempts: int,
    speed_limit: int,
    speed_time: int,
) -> dict:
    """Download ``url`` into ``partial`` with HTTP Range resume. Returns curl stats."""
    last_error = ""
    for attempt in range(1, attempts + 1):
        partial.parent.mkdir(parents=True, exist_ok=True)
        if partial.exists() and partial.stat().st_size > expected_bytes:
            log(f"discard oversized partial {partial.name} (attempt {attempt})")
            partial.unlink()
        offset = partial.stat().st_size if partial.exists() else 0
        if offset == expected_bytes:
            return {"http_code": 206, "downloaded": 0, "speed": 0.0, "time": 0.0}
        command = [
            "curl.exe" if os.name == "nt" else "curl",
            "--fail",
            "--location",
            "--http1.1",
            "--silent",
            "--show-error",
            "--retry",
            "10",
            "--retry-all-errors",
            "--retry-delay",
            "2",
            "--connect-timeout",
            "30",
            "--speed-limit",
            str(speed_limit),
            "--speed-time",
            str(speed_time),
            "--continue-at",
            "-",
            "--output",
            str(partial),
            "--user-agent",
            USER_AGENT,
            "--write-out",
            "%{http_code}\\t%{size_download}\\t%{speed_download}\\t%{time_total}",
            url,
        ]
        result = subprocess.run(
            command, capture_output=True, text=True, errors="replace", check=False
        )
        stats = {"http_code": None, "downloaded": 0, "speed": 0.0, "time": 0.0}
        if result.stdout:
            line = [item for item in result.stdout.strip().splitlines() if item.strip()]
            if line:
                fields = line[-1].split("\t")
                if len(fields) >= 4:
                    try:
                        stats = {
                            "http_code": int(fields[0]),
                            "downloaded": int(float(fields[1])),
                            "speed": float(fields[2]),
                            "time": float(fields[3]),
                        }
                    except ValueError:
                        pass
        size = partial.stat().st_size if partial.exists() else 0
        if result.returncode == 0 and stats["http_code"] in (200, 206):
            if size == expected_bytes:
                return stats
            last_error = f"curl exited 0 but partial is {size}/{expected_bytes} bytes"
        else:
            last_error = (
                f"curl exit {result.returncode}, http {stats['http_code']}: "
                f"{(result.stderr or '').strip()[:300]}"
            )
            if result.returncode == 33 or stats["http_code"] == 416:
                # Server cannot resume this range; restart the file from zero.
                if partial.exists():
                    partial.unlink()
                last_error += " (range not resumable; restarted from zero)"
        log(f"retry {partial.name} attempt {attempt}/{attempts}: {last_error}")
        time.sleep(min(30, 2 * attempt))
    raise RuntimeError(f"failed to download {url}: {last_error}")


def transfer_file(
    artifact: dict,
    resolved: dict,
    destination: Path,
    attempts: int,
    speed_limit: int,
    speed_time: int,
) -> dict:
    expected_bytes = int(artifact["bytes"])
    expected_sha = str(artifact["sha256"]).lower()
    destination.parent.mkdir(parents=True, exist_ok=True)
    partial = destination.with_name(destination.name + ".part")
    started = time.monotonic()
    status = "downloaded"

    def verify(path: Path) -> bool:
        return path.stat().st_size == expected_bytes and sha256_file(path) == expected_sha

    if destination.exists():
        if verify(destination):
            log(f"reuse verified {artifact['path']}")
            return {
                "path": artifact["path"],
                "bytes": expected_bytes,
                "sha256": expected_sha,
                "status": "reused",
                "elapsed_seconds": round(time.monotonic() - started, 3),
                "bytes_per_second": None,
            }
        preserved = destination.with_name(
            f"{destination.name}.sha256-mismatch-{int(time.time())}"
        )
        log(f"preserve mismatched file {destination.name} -> {preserved.name}")
        os.replace(destination, preserved)

    stats = curl_download(
        resolved["url"], partial, expected_bytes, attempts, speed_limit, speed_time
    )
    if partial.stat().st_size != expected_bytes:
        raise RuntimeError(
            f"size mismatch for {artifact['path']}: "
            f"{partial.stat().st_size} != {expected_bytes}"
        )
    actual = sha256_file(partial)
    if actual != expected_sha:
        preserved = partial.with_name(
            f"{partial.name}.sha256-mismatch-{actual[:16]}"
        )
        os.replace(partial, preserved)
        raise RuntimeError(
            f"SHA-256 mismatch for {artifact['path']}: expected {expected_sha}, "
            f"actual {actual}; preserved {preserved.name}"
        )
    os.replace(partial, destination)
    elapsed = time.monotonic() - started
    transferred = expected_bytes
    return {
        "path": artifact["path"],
        "bytes": expected_bytes,
        "sha256": actual,
        "status": status,
        "elapsed_seconds": round(elapsed, 3),
        "bytes_per_second": round(transferred / elapsed, 1) if elapsed > 0 else None,
    }


class ProgressMonitor:
    def __init__(
        self,
        output_root: Path,
        targets: list[tuple[dict, Path]],
        expected_total: int,
        progress_path: Path | None,
        interval: float = 30.0,
    ) -> None:
        self.output_root = output_root
        self.targets = targets
        self.expected_total = expected_total
        self.progress_path = progress_path
        self.interval = interval
        self.stop_event = threading.Event()
        self.thread: threading.Thread | None = None
        self.last_bytes = self.current_bytes()
        self.last_time = time.monotonic()
        self.started = time.monotonic()

    def current_bytes(self) -> int:
        total = 0
        for _artifact, destination in self.targets:
            part = destination.with_name(destination.name + ".part")
            if part.exists():
                total += part.stat().st_size
            elif destination.exists():
                total += destination.stat().st_size
        return total

    def _loop(self) -> None:
        while not self.stop_event.wait(self.interval):
            now_bytes = self.current_bytes()
            now = time.monotonic()
            speed = (now_bytes - self.last_bytes) / max(now - self.last_time, 1e-6)
            ratio = 100.0 * now_bytes / max(self.expected_total, 1)
            log(
                f"progress {now_bytes / 1024**3:.2f}/"
                f"{self.expected_total / 1024**3:.2f} GiB ({ratio:.1f}%) "
                f"{speed / 1024**2:.2f} MiB/s"
            )
            if self.progress_path is not None:
                payload = {
                    "expected_bytes": self.expected_total,
                    "current_bytes": now_bytes,
                    "ratio": round(ratio / 100.0, 4),
                    "bytes_per_second": round(speed, 1),
                    "elapsed_seconds": round(time.monotonic() - self.started, 1),
                    "updated_at_utc": utc_now(),
                }
                temporary = self.progress_path.with_suffix(".json.pending")
                temporary.write_text(
                    json.dumps(payload, indent=2) + "\n", encoding="utf-8"
                )
                os.replace(temporary, self.progress_path)
            self.last_bytes = now_bytes
            self.last_time = now

    def start(self) -> None:
        self.thread = threading.Thread(target=self._loop, daemon=True)
        self.thread.start()

    def stop(self) -> None:
        self.stop_event.set()
        if self.thread is not None:
            self.thread.join(timeout=10)


def write_json(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".pending")
    temporary.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    os.replace(temporary, path)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--report", type=Path)
    parser.add_argument(
        "--source",
        choices=["manifest", "modelscope-mirror"],
        default="manifest",
        help="manifest = fixed per-file URL; modelscope-mirror = ModelScope mirror",
    )
    parser.add_argument("--modelscope-repo")
    parser.add_argument("--modelscope-revision", default="master")
    parser.add_argument("--workers", type=int, default=2)
    parser.add_argument("--attempts", type=int, default=5)
    parser.add_argument("--speed-limit", type=int, default=10240)
    parser.add_argument("--speed-time", type=int, default=120)
    parser.add_argument("--progress", type=Path)
    parser.add_argument(
        "--probe-url",
        action="append",
        default=[],
        help="optional endpoint to probe (reachability evidence), repeatable",
    )
    parser.add_argument(
        "--probe-timeout-seconds", type=float, default=30.0
    )
    args = parser.parse_args()
    if not 1 <= args.workers <= 8:
        raise RuntimeError("workers must be between 1 and 8")

    manifest_path = args.manifest.resolve()
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    files = list(manifest["files"])
    expected_total = int(manifest["component_bytes"])
    declared_total = sum(int(artifact["bytes"]) for artifact in files)
    if declared_total != expected_total:
        raise RuntimeError(
            "manifest component byte total is inconsistent: "
            f"{declared_total} != {expected_total}"
        )

    output_root = args.output.resolve()
    output_root.mkdir(parents=True, exist_ok=True)
    resolved, mirror_info = resolve_sources(
        manifest, args.source, args.modelscope_repo, args.modelscope_revision
    )
    targets = {
        str(artifact["path"]): safe_target(output_root, str(artifact["path"]))
        for artifact in files
    }
    for artifact, destination in zip(files, targets.values()):
        free = disk_free_bytes(output_root)
        if free < int(artifact["bytes"]):
            raise RuntimeError(
                f"insufficient free space for {artifact['path']}: "
                f"{free} < {artifact['bytes']}"
            )

    record: dict = {
        "schema_version": 1,
        "kind": "windows-parity-gate0-model-transfer",
        "manifest": str(manifest_path),
        "manifest_id": manifest.get("id"),
        "repository": manifest.get("repository"),
        "revision": manifest.get("revision"),
        "output_root": str(output_root),
        "component_bytes": expected_total,
        "component_manifest_sha256": hashlib.sha256(
            manifest_path.read_bytes()
        ).hexdigest(),
        "declared_file_count": len(files),
        "transfer": {
            "source": "modelscope_mirror" if args.source == "modelscope-mirror" else "manifest_url",
            "modelscope_mirror_repository": args.modelscope_repo,
            "modelscope_mirror_listing_revision": (
                args.modelscope_revision if args.source == "modelscope-mirror" else None
            ),
            "official_manifest_urls_preserved": True,
            "integrity_basis": "fixed manifest bytes and SHA-256 verified per file",
            "mirror_verification": mirror_info,
            "endpoint_probes": [
                probe_endpoint(url, args.probe_timeout_seconds)
                for url in args.probe_url
            ],
        },
        "machine": {
            "hostname": socket.gethostname(),
            "os": os.name,
            "platform": sys.platform,
            "python": sys.version.split()[0],
        },
        "started_at_utc": utc_now(),
        "files": [],
    }
    disk_free_before = disk_free_bytes(output_root)
    record["disk_free_bytes_before"] = disk_free_before

    ordered = sorted(files, key=lambda artifact: -int(artifact["bytes"]))
    monitor = ProgressMonitor(
        output_root,
        [(artifact, targets[str(artifact["path"])]) for artifact in files],
        expected_total,
        args.progress,
    )
    started = time.monotonic()
    monitor.start()
    results: list[dict] = []
    errors: list[str] = []
    try:
        with ThreadPoolExecutor(max_workers=args.workers) as executor:
            futures = {}
            for artifact in ordered:
                path = str(artifact["path"])
                futures[
                    executor.submit(
                        transfer_file,
                        artifact,
                        resolved[path],
                        targets[path],
                        args.attempts,
                        args.speed_limit,
                        args.speed_time,
                    )
                ] = artifact
            for future in as_completed(futures):
                artifact = futures[future]
                path = str(artifact["path"])
                try:
                    result = future.result()
                except BaseException as error:  # noqa: BLE001 - record and report
                    errors.append(f"{path}: {error}")
                    log(f"FAILED {path}: {error}")
                    continue
                result.update(
                    {
                        "declared_url": resolved[path]["declared_url"],
                        "source_url": resolved[path]["url"],
                        "source_kind": resolved[path]["source_kind"],
                        "source_revision": resolved[path]["source_revision"],
                    }
                )
                results.append(result)
                log(f"complete {path} ({result['status']})")
        record["files"] = [
            dict(result)
            for artifact in files
            for result in results
            if result["path"] == str(artifact["path"])
        ]
        record["elapsed_seconds"] = round(time.monotonic() - started, 3)
        record["disk_free_bytes_after"] = disk_free_bytes(output_root)
        if errors or len(record["files"]) != len(files):
            record["status"] = "failed"
            record["errors"] = errors
            exit_code = 1
        else:
            record["status"] = "passed"
            record["verified_files"] = len(record["files"])
            record["verified_bytes"] = sum(item["bytes"] for item in record["files"])
            record["all_files_size_and_sha256_match_manifest"] = True
            reused = [item for item in record["files"] if item["status"] == "reused"]
            record["reused_files"] = len(reused)
            exit_code = 0
    except BaseException as error:  # noqa: BLE001 - record and report
        record["status"] = "failed"
        record["errors"] = [*errors, f"{type(error).__name__}: {error}"]
        record["files"] = results
        record["elapsed_seconds"] = round(time.monotonic() - started, 3)
        record["disk_free_bytes_after"] = disk_free_bytes(output_root)
        exit_code = 1
        raise
    finally:
        monitor.stop()
        record["finished_at_utc"] = utc_now()
        if args.report:
            write_json(args.report.resolve(), record)
        log(
            "transfer "
            + record["status"]
            + f": {len(record['files'])} files, {expected_total} bytes"
        )
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
