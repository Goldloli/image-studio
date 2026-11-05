"""Shared inference helpers for the Windows diffusers worker.

`__init__` and `__main__` stay stdlib-only (protocol shell and probe); this
module and the per-model backends are imported only after the protocol has been
validated, so a wrong request never loads torch.
"""

from __future__ import annotations

import hashlib
import os
import sys
from pathlib import Path


class Cancelled(Exception):
    """Raised when the host cancelled the active execution."""


def checkpoint(cancel) -> None:
    if cancel.is_set():
        raise Cancelled()


def log(message: str) -> None:
    print(message, file=sys.stderr, flush=True)


def warm_imports() -> None:
    import bitsandbytes  # noqa: F401
    import diffusers  # noqa: F401
    import numpy  # noqa: F401
    import PIL  # noqa: F401
    import torch  # noqa: F401
    import transformers  # noqa: F401


def select_device(minimum_memory_gib: int = 10, minimum_capability=(8, 6)):
    """Return (device_index, properties) for a certified NVIDIA card.

    The standard recipes are certified on the sm86 10 GiB-visible target card. The compact
    tier runs on Turing-class 8 GB cards, so its callers pass ``minimum_capability=(7, 5)``
    and ``minimum_memory_gib=8`` instead of relaxing the standard contract.
    """
    import torch

    if not torch.cuda.is_available() or torch.version.cuda != "12.6":
        raise RuntimeError("CUDA 12.6 is unavailable")
    for index in range(torch.cuda.device_count()):
        properties = torch.cuda.get_device_properties(index)
        if (
            "NVIDIA" in properties.name.upper()
            and (properties.major, properties.minor) >= minimum_capability
            # Driver-reserved memory makes an 8 GB card report ~7.9996 GiB, so compare the
            # rounded visible size instead of the raw byte count.
            and round(properties.total_memory / 1024**3) >= minimum_memory_gib
        ):
            return index, properties
    raise RuntimeError(
        f"required NVIDIA compute capability {minimum_capability} with "
        f">= {minimum_memory_gib} GiB was not found"
    )


def prepared_directory(model_path: str | os.PathLike) -> Path:
    """Deterministic prepared-cache location for a staged model installation.

    The v2 request schema is frozen (no extra `prepared_dir` field), so the
    derived cache for Qwen/MOSS lives next to the installed snapshot:
    ``models/<snapshot>`` -> ``models/<snapshot>.prepared``. The installer
    `--prepare` step must use the same directory.
    """
    path = Path(model_path)
    return path.with_name(path.name + ".prepared")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(8 * 1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_source_rgb(request, output_dir: Path):
    """Validate and load the single Klei

    edit source image from inside the Attempt directory."""
    from PIL import Image

    inputs = request["inputs"]
    if len(inputs) != 1:
        raise ValueError("edit requires exactly one source image")
    source = inputs[0]
    path = Path(source["path"])
    if (
        not path.is_absolute()
        or path.is_symlink()
        or not path.is_file()
        or path.resolve() != path
        or path.parent != output_dir
    ):
        raise ValueError("source image must be an Attempt-local regular file")
    if sha256_file(path) != source["sha256"]:
        raise ValueError("source image hash mismatch")
    with Image.open(path) as opened:
        opened.load()
        if opened.width != source["width"] or opened.height != source["height"]:
            raise ValueError("source image size mismatch")
        if opened.mode not in ("RGB", "RGBA"):
            raise ValueError("source image must be RGB or RGBA")
        return opened.convert("RGB")


def candidate_path(output_dir: Path, name: str = "primary.png") -> Path:
    path = output_dir / name
    if path.exists() or path.is_symlink():
        raise ValueError("Attempt output already exists")
    return path


def save_primary_image(
    output_dir: Path,
    image,
    *,
    width: int = 1024,
    height: int = 1024,
    max_bytes: int = 32 * 1024 * 1024,
) -> dict:
    """Save the primary RGB PNG candidate and return its protocol descriptor."""
    if image.size != (width, height):
        raise ValueError(f"pipeline returned {image.size}, expected {(width, height)}")
    if image.mode != "RGB":
        image = image.convert("RGB")
    output = candidate_path(output_dir)
    image.save(output, format="PNG")
    size = output.stat().st_size
    if not 0 < size <= max_bytes:
        raise ValueError("PNG candidate exceeds the output byte limit")
    with output.open("rb+") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
        stream.flush()
        os.fsync(stream.fileno())
    return {
        "role": "primary",
        "data_type": "image_rgb",
        "relative_path": output.name,
        "width": width,
        "height": height,
        "size_bytes": size,
        "sha256": digest,
    }
