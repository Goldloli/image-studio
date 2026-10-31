"""Qwen/MOSS inference after stdout isolation in __main__."""

import hashlib
import os
from pathlib import Path

from . import MOSS_SNAPSHOT_ID, QWEN_TURBO_SNAPSHOT_ID, model_spec


class Cancelled(Exception):
    pass


def checkpoint(cancel):
    if cancel.is_set():
        raise Cancelled()


def token_budget(tokenizer, prompt, limit):
    """Count the exact model template without allowing tokenizer truncation."""
    raw = tokenizer.tokenizer
    if tokenizer.template:
        formatted = tokenizer.template.format(prompt)
    elif tokenizer.use_chat_template:
        formatted = raw.apply_chat_template(
            [{"role": "user", "content": prompt}],
            tokenize=False,
            add_generation_prompt=True,
            **tokenizer.chat_template_kwargs,
        )
    else:
        formatted = prompt
    ids = raw(
        formatted,
        padding=False,
        truncation=False,
        add_special_tokens=tokenizer.add_special_tokens,
    )["input_ids"]
    if len(ids) > limit:
        raise ValueError(
            f"提示词含模板共 {len(ids)} token，超过 {limit} token 上限；请缩短提示词。"
        )
    return len(ids)


def _steps(cancel, progress, total):
    class Steps:
        def call_before_loop(self, **kwargs):
            checkpoint(cancel)
            progress("sampling", 0, total)

        def call_in_loop(self, t, **kwargs):
            checkpoint(cancel)
            progress("sampling", int(t) + 1, total)

        def call_after_loop(self, **kwargs):
            checkpoint(cancel)

    return Steps()


def _load_tokenizers(snapshot_id, model_path):
    from mflux.models.common.tokenizer.tokenizer_loader import TokenizerLoader

    if snapshot_id == MOSS_SNAPSHOT_ID:
        from mflux.models.moss_image.weights.moss_image_weight_definition import (
            MOSSImageWeightDefinition,
        )

        definitions = MOSSImageWeightDefinition.get_tokenizers()
        name = "qwen3_5"
    else:
        from mflux.models.qwen21.weights.qwen_image21_weight_definition import (
            QwenImage21WeightDefinition,
        )

        definitions = QwenImage21WeightDefinition.get_tokenizers()
        name = "qwen21"
    tokenizers = TokenizerLoader.load_all(definitions, model_path)
    return tokenizers[name]


def _qwen_model(snapshot_id, model_path, cancel, progress):
    from mflux.models.common.config.model_config import ModelConfig
    from mflux.models.qwen21.variants.txt2img.qwen_image_21 import QwenImage21

    class InstrumentedQwen(QwenImage21):
        def _encode_prompt(self, prompt):
            checkpoint(cancel)
            progress("encoding")
            result = super()._encode_prompt(prompt)
            checkpoint(cancel)
            return result

    model_config = (
        ModelConfig.qwen_image_21_turbo()
        if snapshot_id == QWEN_TURBO_SNAPSHOT_ID
        else ModelConfig.qwen_image_21()
    )
    return InstrumentedQwen(
        quantize=8,
        model_path=model_path,
        model_config=model_config,
    )


def _moss_model(model_path, cancel, progress):
    from mflux.models.common.config.model_config import ModelConfig
    from mflux.models.moss_image import MOSSImage

    class InstrumentedMoss(MOSSImage):
        def _encode_prompt(self, prompt):
            checkpoint(cancel)
            progress("encoding")
            result = super()._encode_prompt(prompt)
            checkpoint(cancel)
            return result

    return InstrumentedMoss(
        quantize=8,
        model_path=model_path,
        model_config=ModelConfig.moss_image_sft(),
    )


def generate(request, progress, cancel):
    checkpoint(cancel)
    snapshot_id = request["model_snapshot_id"]
    spec = model_spec(snapshot_id)
    tokenizer = _load_tokenizers(snapshot_id, request["model_path"])
    token_budget(tokenizer, request["instruction"], spec["token_budget"])
    checkpoint(cancel)

    progress("loading")
    if snapshot_id == MOSS_SNAPSHOT_ID:
        model = _moss_model(request["model_path"], cancel, progress)
    else:
        model = _qwen_model(
            snapshot_id, request["model_path"], cancel, progress
        )
    checkpoint(cancel)
    model.callbacks.register(_steps(cancel, progress, spec["steps"]))

    common = dict(
        seed=request["seed"],
        prompt=request["instruction"],
        num_inference_steps=spec["steps"],
        width=1024,
        height=1024,
        guidance=spec["guidance"],
    )
    if snapshot_id == MOSS_SNAPSHOT_ID:
        image = model.generate_image(
            **common,
            negative_prompt="",
            uncond_skip_layers=(22,),
        )
    else:
        image = model.generate_image(
            **common,
            scheduler="linear",
            negative_prompt=None,
        )
    checkpoint(cancel)
    progress("writing")
    output = Path(request["output_dir"]) / "primary.png"
    if output.exists() or output.is_symlink():
        raise ValueError("Attempt 输出路径已存在，拒绝覆盖。")
    image.save(path=str(output))
    checkpoint(cancel)
    size = output.stat().st_size
    if not 0 < size <= 32 * 1024 * 1024:
        raise ValueError("PNG 候选超过 32 MiB 上限。")
    with output.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
        os.fsync(stream.fileno())
    return dict(
        role="primary",
        data_type="image_rgb",
        relative_path="primary.png",
        width=1024,
        height=1024,
        size_bytes=size,
        sha256=digest,
    )
