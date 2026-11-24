#!/usr/bin/env python3
"""Download and verify immutable model files in bounded HTTP ranges.

This is experiment tooling for the MiniMax-H3-Image gate. Whole-file resume is
not stable enough for multi-GB transfers through redirecting mirrors, so each
fixed range is downloaded independently, checked for exact size, and assembled
only after every chunk succeeds.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path, PurePosixPath

CHUNK_BYTES = 64 * 1024 * 1024
USER_AGENT = "ImageStudio-MiniMaxH3Gate/1"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(8 * 1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def safe_target(root: Path, relative: str) -> Path:
    parsed = PurePosixPath(relative)
    if parsed.is_absolute() or ".." in parsed.parts or not parsed.parts:
        raise RuntimeError(f"unsafe manifest path: {relative!r}")
    return root.joinpath(*parsed.parts)


def log(message: str) -> None:
    print(f"[{time.strftime('%H:%M:%S')}] {message}", flush=True)


def download_chunk(
    url: str,
    target: Path,
    start: int,
    end: int,
    attempts: int,
) -> int:
    expected = end - start + 1
    if target.is_file() and target.stat().st_size == expected:
        return expected
    temporary = target.with_name(target.name + ".part")
    if temporary.exists():
        temporary.unlink()
    command = [
        "curl",
        "--fail",
        "--location",
        "--http1.1",
        "--silent",
        "--show-error",
        "--retry",
        "5",
        "--retry-all-errors",
        "--retry-delay",
        "2",
        "--connect-timeout",
        "30",
        "--max-time",
        "900",
        "--range",
        f"{start}-{end}",
        "--output",
        str(temporary),
        "--user-agent",
        USER_AGENT,
        url,
    ]
    error = ""
    for attempt in range(1, attempts + 1):
        result = subprocess.run(command, capture_output=True, text=True, check=False)
        if temporary.is_file() and temporary.stat().st_size == expected:
            os.replace(temporary, target)
            return expected
        error = (result.stderr or result.stdout or "").strip()[:500]
        if temporary.exists():
            temporary.unlink()
        if attempt < attempts:
            time.sleep(min(10, attempt * 2))
    raise RuntimeError(
        f"range {start}-{end} failed after {attempts} attempts: {error}"
    )


class Progress:
    def __init__(self, total_chunks: int) -> None:
        self.total_chunks = total_chunks
        self.done_chunks = 0
        self.done_bytes = 0
        self.started = time.monotonic()
        self.lock = threading.Lock()

    def add(self, size: int) -> None:
        with self.lock:
            self.done_chunks += 1
            self.done_bytes += size
            elapsed = max(time.monotonic() - self.started, 1e-6)
            speed = self.done_bytes / elapsed / 1024**2
            print(
                f"[{time.strftime('%H:%M:%S')}] chunk "
                f"{self.done_chunks}/{self.total_chunks} "
                f"({self.done_bytes / 1024**3:.2f} GiB, {speed:.1f} MiB/s)",
                flush=True,
            )


def transfer_file(
    artifact: dict,
    destination: Path,
    workers: int,
    attempts: int,
    reuse_verified: bool,
) -> dict:
    expected_bytes = int(artifact["bytes"])
    expected_sha = str(artifact["sha256"]).lower()
    if destination.is_file():
        if (
            reuse_verified
            and destination.stat().st_size == expected_bytes
            and sha256_file(destination) == expected_sha
        ):
            log(f"reuse verified {artifact['path']}")
            return {
                "path": artifact["path"],
                "bytes": expected_bytes,
                "sha256": expected_sha,
                "status": "reused",
            }
        preserved = destination.with_name(
            f"{destination.name}.mismatch-{int(time.time())}"
        )
        log(f"preserve mismatched {destination.name} -> {preserved.name}")
        os.replace(destination, preserved)

    chunk_root = (
        destination.parent
        / ".chunks"
        / destination.name
        / expected_sha
    )
    chunk_root.mkdir(parents=True, exist_ok=True)
    ranges = [
        (index, start, min(start + CHUNK_BYTES - 1, expected_bytes - 1))
        for index, start in enumerate(range(0, expected_bytes, CHUNK_BYTES))
    ]
    progress = Progress(len(ranges))
    started = time.monotonic()
    with ThreadPoolExecutor(max_workers=workers) as executor:
        futures = {
            executor.submit(
                download_chunk,
                str(artifact["url"]),
                chunk_root / f"{index:05d}.chunk",
                start,
                end,
                attempts,
            ): (index, start, end)
            for index, start, end in ranges
        }
        try:
            for future in as_completed(futures):
                progress.add(future.result())
        except BaseException:
            for future in futures:
                future.cancel()
            raise

    assembled = destination.with_name(destination.name + ".assembled")
    if assembled.exists():
        assembled.unlink()
    with assembled.open("wb") as output:
        for index, _start, _end in ranges:
            chunk = chunk_root / f"{index:05d}.chunk"
            with chunk.open("rb") as source:
                while data := source.read(8 * 1024 * 1024):
                    output.write(data)
        output.flush()
        os.fsync(output.fileno())
    if assembled.stat().st_size != expected_bytes:
        raise RuntimeError(
            f"assembled size mismatch for {artifact['path']}: "
            f"{assembled.stat().st_size} != {expected_bytes}"
        )
    actual = sha256_file(assembled)
    if actual != expected_sha:
        preserved = assembled.with_name(
            assembled.name + f".sha256-mismatch-{actual[:16]}"
        )
        os.replace(assembled, preserved)
        raise RuntimeError(
            f"SHA-256 mismatch for {artifact['path']}: "
            f"expected {expected_sha}, actual {actual}"
        )
    os.replace(assembled, destination)
    return {
        "path": artifact["path"],
        "bytes": expected_bytes,
        "sha256": actual,
        "status": "downloaded",
        "elapsed_seconds": round(time.monotonic() - started, 3),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--workers", type=int, default=8)
    parser.add_argument("--attempts", type=int, default=5)
    parser.add_argument("--path", action="append", default=[])
    args = parser.parse_args()
    if not 1 <= args.workers <= 16:
        raise RuntimeError("workers must be between 1 and 16")

    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    files = list(manifest["files"])
    if args.path:
        selected = set(args.path)
        files = [entry for entry in files if entry["path"] in selected]
        missing = selected - {entry["path"] for entry in files}
        if missing:
            raise RuntimeError("unknown manifest paths: " + ", ".join(sorted(missing)))
    declared_total = sum(int(entry["bytes"]) for entry in files)
    expected_total = int(manifest["component_bytes"])
    if not args.path and declared_total != expected_total:
        raise RuntimeError(
            f"manifest total mismatch: {declared_total} != {expected_total}"
        )

    args.output.mkdir(parents=True, exist_ok=True)
    results = []
    started = time.monotonic()
    for artifact in files:
        destination = safe_target(args.output, str(artifact["path"]))
        log(f"start {artifact['path']} ({int(artifact['bytes']) / 1024**3:.2f} GiB)")
        results.append(
            transfer_file(
                artifact,
                destination,
                args.workers,
                args.attempts,
                reuse_verified=True,
            )
        )
        log(f"complete {artifact['path']}")

    report = {
        "schema_version": 1,
        "manifest": str(args.manifest.resolve()),
        "output": str(args.output.resolve()),
        "status": "passed",
        "workers": args.workers,
        "selected_paths": args.path,
        "verified_files": len(results),
        "verified_bytes": sum(int(item["bytes"]) for item in results),
        "elapsed_seconds": round(time.monotonic() - started, 3),
        "results": results,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    temporary = args.report.with_suffix(args.report.suffix + ".pending")
    temporary.write_text(
        json.dumps(report, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    os.replace(temporary, args.report)
    log(
        f"passed {len(results)} files, "
        f"{report['verified_bytes'] / 1024**3:.2f} GiB"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
