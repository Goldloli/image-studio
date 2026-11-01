"""Resume and verify the fixed Windows Gate 0 model closure."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import threading
import time
import urllib.parse
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path, PurePosixPath


CHUNK_BYTES = 8 * 1024 * 1024
USER_AGENT = "ImageStudio-Gate0/1"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(CHUNK_BYTES), b""):
            digest.update(chunk)
    return digest.hexdigest()


def safe_target(root: Path, relative: str) -> Path:
    path = PurePosixPath(relative)
    if path.is_absolute() or ".." in path.parts or not path.parts:
        raise RuntimeError(f"unsafe manifest path: {relative}")
    return root.joinpath(*path.parts)


def resume_download(
    artifact: dict[str, object], source_url: str, destination: Path
) -> None:
    expected = int(artifact["bytes"])
    destination.parent.mkdir(parents=True, exist_ok=True)
    partial = destination.with_name(destination.name + ".part")
    if partial.exists() and partial.stat().st_size > expected:
        raise RuntimeError(f"partial file is larger than manifest: {partial}")

    offset = partial.stat().st_size if partial.exists() else 0
    if offset == expected:
        os.replace(partial, destination)
        return
    curl = shutil.which("curl")
    if curl is None:
        raise RuntimeError("curl is required for resumable Gate 0 downloads")
    command = [
        curl,
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
        "10240",
        "--speed-time",
        "60",
        "--continue-at",
        "-",
        "--output",
        str(partial),
        "--user-agent",
        USER_AGENT,
        source_url,
    ]
    result = subprocess.run(command, check=False)
    if result.returncode != 0:
        raise RuntimeError(
            f"curl failed for {artifact['path']} with exit code {result.returncode}"
        )
    if partial.stat().st_size != expected:
        raise RuntimeError(
            f"size mismatch for {artifact['path']}: "
            f"{partial.stat().st_size} != {expected}"
        )
    os.replace(partial, destination)


def preserve_bad(path: Path, expected: str) -> Path:
    actual = sha256(path)
    target = path.with_name(f"{path.name}.sha256-mismatch-{actual[:16]}")
    if target.exists():
        target.unlink()
    shutil.move(path, target)
    raise RuntimeError(
        f"SHA-256 mismatch for {path}: expected {expected}, actual {actual}; "
        f"preserved as {target.name}"
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--report", type=Path)
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument(
        "--mirror-base",
        help="optional fixed Hugging Face mirror base for the same repository revision",
    )
    parser.add_argument(
        "--modelscope",
        action="store_true",
        help="use the ModelScope source declared in the model manifest",
    )
    args = parser.parse_args()
    if not 1 <= args.workers <= 8:
        raise RuntimeError("workers must be between 1 and 8")

    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    if args.mirror_base and args.modelscope:
        raise RuntimeError("--mirror-base and --modelscope are mutually exclusive")
    mirror_base = args.mirror_base
    if args.modelscope:
        template = manifest["download_source"]["url_template"]
        mirror_base = template.split("{path}", 1)[0].rstrip("/")
    files = manifest["files"]
    expected_total = int(manifest["component_bytes"])
    if (
        sum(
            int(artifact["bytes"])
            for artifact in files
            if artifact["path"] != "README.md"
        )
        != expected_total
    ):
        raise RuntimeError("manifest component byte total is inconsistent")
    targets = {
        artifact["path"]: safe_target(args.output, artifact["path"])
        for artifact in files
    }
    lock = threading.Lock()

    def download(artifact: dict[str, object]) -> None:
        path = targets[artifact["path"]]
        if not path.exists():
            source_url = str(artifact["url"])
            if mirror_base:
                source_url = (
                    mirror_base.rstrip("/")
                    + "/"
                    + urllib.parse.quote(str(artifact["path"]), safe="/")
                )
            with lock:
                print(f"start {artifact['path']}", flush=True)
            resume_download(artifact, source_url, path)
            with lock:
                print(f"downloaded {artifact['path']}", flush=True)

    started = time.monotonic()
    with ThreadPoolExecutor(max_workers=args.workers) as executor:
        futures = {executor.submit(download, artifact): artifact for artifact in files}
        for future in as_completed(futures):
            future.result()

    verified = []
    for artifact in files:
        path = targets[artifact["path"]]
        if path.stat().st_size != int(artifact["bytes"]):
            raise RuntimeError(f"size mismatch for {artifact['path']}")
        actual = sha256(path)
        if actual != artifact["sha256"]:
            preserve_bad(path, str(artifact["sha256"]))
        verified.append(
            {
                "path": artifact["path"],
                "bytes": path.stat().st_size,
                "sha256": actual,
            }
        )
        print(f"verified {artifact['path']}", flush=True)

    report = {
        "schema_version": 1,
        "kind": "windows-gate0-model-download",
        "status": "passed",
        "repository": manifest["repository"],
        "revision": manifest["revision"],
        "component_bytes": expected_total,
        "verified_files": verified,
        "elapsed_seconds": time.monotonic() - started,
        "component_manifest_sha256": hashlib.sha256(
            args.manifest.read_bytes()
        ).hexdigest(),
        "official_component_urls_preserved": True,
        "transfer_provider": (
            "modelscope"
            if args.modelscope
            else "explicit_mirror"
            if mirror_base
            else "official_manifest_url"
        ),
        "transfer_mirror_base": mirror_base,
    }
    if args.report:
        report_path = args.report.resolve()
        report_path.parent.mkdir(parents=True, exist_ok=True)
        temporary = report_path.with_suffix(report_path.suffix + ".pending")
        temporary.write_text(
            json.dumps(report, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
            newline="\n",
        )
        os.replace(temporary, report_path)
    print(
        json.dumps(
            {"status": "passed", "files": len(verified), "bytes": expected_total}
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
