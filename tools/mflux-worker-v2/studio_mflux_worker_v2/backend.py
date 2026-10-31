"""All ML imports happen after stdout isolation in __main__."""

import hashlib
import os
from pathlib import Path


class Cancelled(Exception):
    pass


def checkpoint(cancel):
    if cancel.is_set():
        raise Cancelled()


def token_budget(tokenizer, prompt):
    """Use the exact Klein tokenizer template without truncation."""
    raw = tokenizer.tokenizer
    formatted = prompt
    if tokenizer.template:
        formatted = tokenizer.template.format(prompt)
    elif tokenizer.use_chat_template:
        formatted = raw.apply_chat_template(
            [{"role": "user", "content": prompt}],
            tokenize=False,
            add_generation_prompt=True,
            **tokenizer.chat_template_kwargs,
        )
    ids = raw(
        formatted,
        padding=False,
        truncation=False,
        add_special_tokens=tokenizer.add_special_tokens,
    )["input_ids"]
    if len(ids) > 512:
        raise ValueError(
            f"提示词含模板共 {len(ids)} token，超过 512 token 上限；请缩短提示词。"
        )
    return len(ids)


def _steps(cancel, progress):
    class Steps:
        def call_before_loop(self, **kwargs):
            checkpoint(cancel)
            progress("sampling", 0, 4)

        def call_in_loop(self, t, **kwargs):
            checkpoint(cancel)
            progress("sampling", int(t) + 1, 4)

        def call_after_loop(self, **kwargs):
            checkpoint(cancel)

    return Steps()


def _inspect_edit_source(path):
    from PIL import Image

    source = Path(path)
    if (
        not source.is_absolute()
        or source.is_symlink()
        or not source.is_file()
        or source.resolve() != source
    ):
        raise ValueError("编辑源图必须是规范化本地普通文件。")
    with Image.open(source) as image:
        image.load()
        if image.width < 16 or image.height < 16:
            raise ValueError("编辑源图每边至少需要 16 像素。")
        if image.width * image.height > 4 * 1024 * 1024:
            raise ValueError("编辑源图超过 4 MP 上限。")
        rgba = image.convert("RGBA")
        if rgba.getchannel("A").getextrema()[0] < 255:
            raise ValueError("Klein 单图编辑不接受有效透明像素。")


def generate(request, progress, cancel):
    checkpoint(cancel)
    from mflux.models.common.tokenizer.tokenizer_loader import TokenizerLoader
    from mflux.models.flux2.variants.edit.flux2_klein_edit import Flux2KleinEdit
    from mflux.models.flux2.variants.txt2img.flux2_klein import Flux2Klein
    from mflux.models.flux2.weights.flux2_weight_definition import (
        Flux2KleinWeightDefinition,
    )

    progress("loading")
    tokenizers = TokenizerLoader.load_all(
        Flux2KleinWeightDefinition.get_tokenizers(), request["model_path"]
    )
    token_budget(tokenizers["qwen3"], request["instruction"])
    checkpoint(cancel)

    class InstrumentedText(Flux2Klein):
        def _encode_prompt_pair(self, **kwargs):
            checkpoint(cancel)
            progress("encoding")
            return super()._encode_prompt_pair(**kwargs)

    class InstrumentedEdit(Flux2KleinEdit):
        def _encode_prompt_pair(self, **kwargs):
            checkpoint(cancel)
            progress("encoding")
            return super()._encode_prompt_pair(**kwargs)

    edit = request["recipe_id"] == "klein4b-edit-v1"
    model_type = InstrumentedEdit if edit else InstrumentedText
    model = model_type(quantize=4, model_path=request["model_path"])
    checkpoint(cancel)
    model.callbacks.register(_steps(cancel, progress))
    common = dict(
        seed=request["seed"],
        prompt=request["instruction"],
        num_inference_steps=4,
        width=1024,
        height=1024,
        guidance=1.0,
        scheduler="flow_match_euler_discrete",
    )
    if edit:
        source = request["inputs"][0]["path"]
        _inspect_edit_source(source)
        # Edit conditioning is a reference-image branch, never img2img strength.
        image = model.generate_image(image_paths=[source], **common)
    else:
        image = model.generate_image(**common)
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
