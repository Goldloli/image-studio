"""BiRefNet CPU inference after stdout has been isolated by __main__."""

import hashlib
import os
from pathlib import Path

import numpy as np


MEAN = np.asarray([0.485, 0.456, 0.406], dtype=np.float32)
STD = np.asarray([0.229, 0.224, 0.225], dtype=np.float32)
NETWORK_SIZE = 1024
MAX_SIDE = 4096
MAX_PIXELS = 4 * 1024 * 1024


class Cancelled(Exception):
    pass


def checkpoint(cancel):
    if cancel.is_set():
        raise Cancelled()


def _regular_directory(value):
    path = Path(value)
    return (
        isinstance(value, str)
        and path.is_absolute()
        and not path.is_symlink()
        and path.is_dir()
        and path.resolve() == path
    )


def model_file(model_path):
    root = Path(model_path)
    if not _regular_directory(model_path):
        raise ValueError("BiRefNet 模型目录必须是规范化本地目录。")
    candidates = sorted(root.glob("*.onnx"))
    if len(candidates) != 1:
        raise ValueError("BiRefNet 安装目录必须恰好包含一个 ONNX 模型。")
    candidate = candidates[0]
    if not candidate.is_file() or candidate.is_symlink():
        raise ValueError("BiRefNet ONNX 必须是普通文件。")
    return candidate


def load_source(path, expected_sha256, output_dir):
    from PIL import Image

    source = Path(path)
    output = Path(output_dir)
    if (
        not source.is_absolute()
        or source.is_symlink()
        or not source.is_file()
        or source.resolve() != source
        or source.parent != output
    ):
        raise ValueError("BiRefNet 源图必须是 Attempt 目录内的普通文件。")
    with source.open("rb") as stream:
        if hashlib.file_digest(stream, "sha256").hexdigest() != expected_sha256:
            raise ValueError("BiRefNet 源图哈希不符。")
    with Image.open(source) as opened:
        opened.load()
        if (
            opened.width <= 0
            or opened.height <= 0
            or opened.width > MAX_SIDE
            or opened.height > MAX_SIDE
            or opened.width * opened.height > MAX_PIXELS
        ):
            raise ValueError("BiRefNet 源图尺寸超过 4 MP 或单边 4096 像素。")
        if opened.mode not in ("RGB", "RGBA"):
            raise ValueError("BiRefNet 源图必须是规范化 RGB 或 RGBA PNG。")
        rgba = opened.convert("RGBA")
        return rgba.copy()


def preprocess(image):
    pixels = resize_half_pixel_uint8(
        np.asarray(image.convert("RGB"), dtype=np.uint8),
        NETWORK_SIZE,
        NETWORK_SIZE,
    ).astype(np.float32) / 255.0
    normalized = (pixels - MEAN) / STD
    return np.ascontiguousarray(normalized.transpose(2, 0, 1)[None, ...])


def _half_pixel_positions(source_size, output_size):
    coordinates = (
        (np.arange(output_size, dtype=np.float32) + 0.5)
        * (source_size / output_size)
        - 0.5
    )
    coordinates = np.clip(coordinates, 0.0, source_size - 1)
    lower = np.floor(coordinates).astype(np.intp)
    upper = np.minimum(lower + 1, source_size - 1)
    return lower, upper, (coordinates - lower).astype(np.float32)


def resize_half_pixel_uint8(values, width, height):
    """Match the official cv2.INTER_LINEAR preprocessing with NumPy only."""
    array = np.asarray(values)
    if array.ndim not in (2, 3) or array.dtype != np.uint8:
        raise ValueError("cv2-compatible resize requires a uint8 array")
    if width <= 0 or height <= 0:
        raise ValueError("resize dimensions must be positive")
    source_height, source_width = array.shape[:2]
    y0, y1, wy = _half_pixel_positions(source_height, height)
    x0, x1, wx = _half_pixel_positions(source_width, width)
    if array.ndim == 3:
        wy = wy[:, None, None]
        wx = wx[None, :, None]
        top = array[y0][:, x0] * (1.0 - wx) + array[y0][:, x1] * wx
        bottom = array[y1][:, x0] * (1.0 - wx) + array[y1][:, x1] * wx
        return np.floor(top * (1.0 - wy) + bottom * wy + 0.5).clip(0, 255).astype(np.uint8)
    wy = wy[:, None]
    wx = wx[None, :]
    top = array[y0][:, x0] * (1.0 - wx) + array[y0][:, x1] * wx
    bottom = array[y1][:, x0] * (1.0 - wx) + array[y1][:, x1] * wx
    return np.floor(top * (1.0 - wy) + bottom * wy + 0.5).clip(0, 255).astype(np.uint8)


def sigmoid_logits(logits):
    values = np.asarray(logits, dtype=np.float32)
    positive = values >= 0
    result = np.empty_like(values)
    result[positive] = 1.0 / (1.0 + np.exp(-values[positive]))
    exponent = np.exp(values[~positive])
    result[~positive] = exponent / (1.0 + exponent)
    return result


def _resize_axis(values, output_size, axis):
    source_size = values.shape[axis]
    if source_size == output_size:
        return values
    if source_size == 1:
        shape = [1, 1]
        shape[axis] = output_size
        return np.repeat(values, output_size, axis=axis)
    coordinates = np.linspace(0.0, source_size - 1, output_size, dtype=np.float32)
    lower = np.floor(coordinates).astype(np.intp)
    upper = np.minimum(lower + 1, source_size - 1)
    weight = (coordinates - lower).astype(np.float32)
    if axis == 1:
        return (
            values[:, lower] * (1.0 - weight)
            + values[:, upper] * weight
        ).astype(np.float32, copy=False)
    return (
        values[lower, :] * (1.0 - weight[:, None])
        + values[upper, :] * weight[:, None]
    ).astype(np.float32, copy=False)


def resize_align_corners(values, width, height):
    array = np.asarray(values, dtype=np.float32)
    if array.ndim != 2 or array.shape[0] == 0 or array.shape[1] == 0:
        raise ValueError("BiRefNet 概率图必须是二维非空数组。")
    if width <= 0 or height <= 0:
        raise ValueError("BiRefNet 输出尺寸必须为正。")
    horizontal = _resize_axis(array, width, 1)
    return _resize_axis(horizontal, height, 0)


def compose_mask(probabilities, source_alpha):
    probability = np.clip(np.asarray(probabilities, dtype=np.float32), 0.0, 1.0)
    alpha = np.asarray(source_alpha, dtype=np.uint8)
    combined = probability * alpha.astype(np.float32)
    return np.floor(combined + 0.5).clip(0, 255).astype(np.uint8)


def _candidate(role, data_type, path, width, height):
    size = path.stat().st_size
    if not 0 < size <= 32 * 1024 * 1024:
        raise ValueError("BiRefNet 候选 PNG 超过 32 MiB 上限。")
    with path.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
        os.fsync(stream.fileno())
    return dict(
        role=role,
        data_type=data_type,
        relative_path=path.name,
        width=width,
        height=height,
        size_bytes=size,
        sha256=digest,
    )


def generate(request, progress, cancel):
    checkpoint(cancel)
    from PIL import Image
    import onnxruntime as ort

    output_dir = Path(request["output_dir"])
    if not _regular_directory(request["output_dir"]):
        raise ValueError("BiRefNet 输出目录必须是规范化本地目录。")
    source = request["inputs"][0]
    image = load_source(source["path"], source["sha256"], output_dir)
    if (image.width, image.height) != (source["width"], source["height"]):
        raise ValueError("BiRefNet 源图描述尺寸不符。")
    progress("loading")
    options = ort.SessionOptions()
    options.intra_op_num_threads = 4
    options.inter_op_num_threads = 1
    options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
    session = ort.InferenceSession(
        str(model_file(request["model_path"])),
        sess_options=options,
        providers=["CPUExecutionProvider"],
    )
    if session.get_providers() != ["CPUExecutionProvider"]:
        raise ValueError("BiRefNet 只能使用 CPUExecutionProvider。")
    inputs = session.get_inputs()
    outputs = session.get_outputs()
    if len(inputs) != 1 or len(outputs) < 1:
        raise ValueError("BiRefNet ONNX 输入输出数量不符。")
    input_spec = inputs[0]
    if input_spec.type != "tensor(float)":
        raise ValueError("BiRefNet ONNX 输入必须是 float32。")
    checkpoint(cancel)
    tensor = preprocess(image)
    progress("inference")
    raw = session.run(None, {input_spec.name: tensor})[0]
    checkpoint(cancel)
    progress("decoding")
    probability = sigmoid_logits(raw)
    if probability.ndim != 4 or probability.shape[0] != 1 or probability.shape[1] != 1:
        raise ValueError("BiRefNet ONNX 输出必须是 batch=1、channel=1 的概率图。")
    resized = resize_align_corners(
        probability[0, 0], image.width, image.height
    )
    alpha = compose_mask(resized, image.getchannel("A"))
    rgba = np.asarray(image, dtype=np.uint8).copy()
    rgba[..., 3] = alpha

    primary = output_dir / "primary.png"
    mask = output_dir / "mask.png"
    for path in (primary, mask):
        if path.exists() or path.is_symlink():
            raise ValueError("BiRefNet Attempt 输出路径已存在，拒绝覆盖。")
    progress("writing")
    Image.fromarray(rgba, "RGBA").save(primary, format="PNG")
    checkpoint(cancel)
    Image.fromarray(alpha, "L").save(mask, format="PNG")
    checkpoint(cancel)
    return [
        _candidate("primary", "image_rgba", primary, image.width, image.height),
        _candidate("mask", "mask", mask, image.width, image.height),
    ]
