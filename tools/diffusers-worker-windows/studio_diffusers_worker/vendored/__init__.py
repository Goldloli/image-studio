"""Vendored reviewed MOSS-Image pipeline module (Apache-2.0).

The upstream snapshot ships ``pipeline_moss_image.py`` (25,060 bytes, SHA-256
``1dac1aca...``) and the product must not execute Python from an installed model
directory (design.md section 10.2 item 5, tasks.md 1.5 review). The exact
reviewed bytes are vendored here, verified against the frozen digest before
import, and accompanied by the upstream Apache-2.0 license text.
"""

from __future__ import annotations

import hashlib
import importlib.util
import sys
from pathlib import Path

MOSS_PIPELINE_FILENAME = "pipeline_moss_image.py"
MOSS_PIPELINE_BYTES = 25060
MOSS_PIPELINE_SHA256 = (
    "1dac1aca9feb1bfdfc727a401e4b67d61a505d3190706d1b7d2d784f88678b31"
)
MOSS_LICENSE_FILENAME = "LICENSE-MOSS-Image.txt"
_MODULE_NAME = "studio_diffusers_worker._vendored_pipeline_moss_image"


def pipeline_path() -> Path:
    return Path(__file__).with_name(MOSS_PIPELINE_FILENAME)


def verify_pipeline() -> str:
    """Verify the vendored pipeline bytes; return the verified SHA-256."""
    path = pipeline_path()
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if len(data) != MOSS_PIPELINE_BYTES or digest != MOSS_PIPELINE_SHA256:
        raise RuntimeError(
            "vendored MOSS-Image pipeline module does not match the reviewed "
            f"frozen bytes (bytes={len(data)}, sha256={digest})"
        )
    return digest


def load_moss_pipeline():
    """Import the verified vendored MOSS-Image pipeline module."""
    verify_pipeline()
    if _MODULE_NAME in sys.modules:
        return sys.modules[_MODULE_NAME]
    spec = importlib.util.spec_from_file_location(_MODULE_NAME, pipeline_path())
    if spec is None or spec.loader is None:  # pragma: no cover - defensive
        raise RuntimeError("cannot load the vendored MOSS-Image pipeline module")
    module = importlib.util.module_from_spec(spec)
    sys.modules[_MODULE_NAME] = module
    spec.loader.exec_module(module)
    return module


__all__ = [
    "MOSS_PIPELINE_BYTES",
    "MOSS_PIPELINE_SHA256",
    "MOSS_LICENSE_FILENAME",
    "verify_pipeline",
    "load_moss_pipeline",
]
