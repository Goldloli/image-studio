"""Build a deterministic Gate 0 runtime manifest from a pip install report."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
from urllib.parse import unquote, urlparse


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def artifact(path: Path, url: str, **extra: object) -> dict[str, object]:
    if not path.is_file():
        raise RuntimeError(f"missing runtime artifact: {path}")
    result: dict[str, object] = {
        "path": path.name,
        "url": url,
        "bytes": path.stat().st_size,
        "sha256": sha256(path),
    }
    result.update(extra)
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--downloads", type=Path, required=True)
    parser.add_argument("--python-archive", type=Path, required=True)
    parser.add_argument("--python-url", required=True)
    parser.add_argument("--requirements-lock", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    report = json.loads(args.report.read_text(encoding="utf-8"))
    wheels = []
    for installed in report["install"]:
        download = installed["download_info"]
        url = download["url"]
        filename = unquote(Path(urlparse(url).path).name)
        path = args.downloads / filename
        record = artifact(
            path,
            url,
            package=installed["metadata"]["name"].lower().replace("_", "-"),
            version=installed["metadata"]["version"],
        )
        archive_info = download["archive_info"]
        expected = archive_info.get("hashes", {}).get("sha256")
        if expected is None and "hash" in archive_info:
            expected = archive_info["hash"].removeprefix("sha256=")
        if expected is not None and record["sha256"] != expected:
            raise RuntimeError(f"SHA-256 mismatch for {filename}")
        wheels.append(record)
    wheels.sort(key=lambda record: (record["package"], record["path"]))

    interpreter = artifact(
        args.python_archive,
        args.python_url,
        package="cpython",
        version="3.12.10",
        implementation="CPython",
        platform="Windows embeddable x64",
    )
    all_artifacts = [interpreter, *wheels]
    manifest = {
        "schema_version": 1,
        "id": "windows-gate0-python31210-torch2100-cu126-bnb0482-v1",
        "kind": "windows-gate0-runtime-closure",
        "status": "content_verified_not_yet_model_certified",
        "platform": "windows-amd64",
        "python": "3.12.10",
        "torch": "2.10.0+cu126",
        "cuda": "12.6",
        "bitsandbytes": "0.48.2",
        "requirements_lock": artifact(
            args.requirements_lock,
            "repository:resources/windows-foundation/python-runtime/requirements.lock",
        ),
        "interpreter": interpreter,
        "wheels": wheels,
        "artifact_count": len(all_artifacts),
        "total_bytes": sum(record["bytes"] for record in all_artifacts),
        "source_policy": {
            "binary_only": True,
            "system_python_modified": False,
            "target_platform": "windows-amd64",
            "python_abi": "cp312",
            "cuda_index": "https://download.pytorch.org/whl/cu126",
            "pypi_index": "https://pypi.org/simple",
        },
        "limitations": [
            "This manifest proves fixed binary content only.",
            "A passing runtime probe does not prove full model inference or resource budgets.",
            "Python 3.12.10 is the last 3.12 release with official Windows binaries; later 3.12 releases are source-only.",
        ],
    }
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_suffix(output.suffix + ".pending")
    temporary.write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    os.replace(temporary, output)
    print(
        json.dumps(
            {
                "output": str(output),
                "artifact_count": manifest["artifact_count"],
                "total_bytes": manifest["total_bytes"],
            }
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
