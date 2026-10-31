#!/usr/bin/env python3
"""Run a direct pinned MFLUX baseline for Qwen 2.1 or MOSS-Image-SFT."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import time


MODELS = {
    "qwen": {
        "steps": 40,
        "guidance": 1.0,
        "scheduler": "linear",
        "budget": 2048,
    },
    "qwen-turbo": {
        "steps": 8,
        "guidance": 1.0,
        "scheduler": "linear",
        "budget": 2048,
    },
    "moss": {
        "steps": 50,
        "guidance": 3.0,
        "scheduler": "moss_image",
        "budget": 4096,
    },
}


def token_count(tokenizer, prompt):
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
    return len(
        raw(
            formatted,
            padding=False,
            truncation=False,
            add_special_tokens=tokenizer.add_special_tokens,
        )["input_ids"]
    )


class Progress:
    def __init__(self):
        self.current = None
        self.completed = 0

    def call_before_loop(self, **kwargs):
        self.current = "sampling"
        self.completed = 0

    def call_in_loop(self, t, **kwargs):
        self.completed = int(t) + 1

    def call_after_loop(self, **kwargs):
        pass


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--family", choices=sorted(MODELS), required=True)
    parser.add_argument("--model-path", type=Path, required=True)
    parser.add_argument("--specs", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    spec = MODELS[args.family]
    model_path = args.model_path.resolve(strict=True)
    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    specs = json.loads(args.specs.read_text(encoding="utf-8"))
    expected_tasks = 4 if args.family == "moss" else 3
    if len(specs) != expected_tasks:
        raise SystemExit(f"{args.family} requires {expected_tasks} task specs")

    os.environ["HF_HUB_OFFLINE"] = "1"
    os.environ["TRANSFORMERS_OFFLINE"] = "1"
    os.environ["TOKENIZERS_PARALLELISM"] = "false"

    from mflux.models.common.config.model_config import ModelConfig
    from mflux.models.common.tokenizer.tokenizer_loader import TokenizerLoader
    import mlx.core as mx

    mx.set_default_device(mx.gpu)
    if args.family == "moss":
        from mflux.models.moss_image import MOSSImage
        from mflux.models.moss_image.weights.moss_image_weight_definition import (
            MOSSImageWeightDefinition,
        )

        definitions = MOSSImageWeightDefinition.get_tokenizers()
        tokenizer_name = "qwen3_5"
    else:
        from mflux.models.qwen21.variants.txt2img.qwen_image_21 import QwenImage21
        from mflux.models.qwen21.weights.qwen_image21_weight_definition import (
            QwenImage21WeightDefinition,
        )

        definitions = QwenImage21WeightDefinition.get_tokenizers()
        tokenizer_name = "qwen21"

    tokenizers = TokenizerLoader.load_all(definitions, str(model_path))
    tokenizer = tokenizers[tokenizer_name]
    started = time.monotonic()
    if args.family == "moss":
        model = MOSSImage(
            quantize=8,
            model_path=str(model_path),
            model_config=ModelConfig.moss_image_sft(),
        )
    else:
        config = (
            ModelConfig.qwen_image_21_turbo()
            if args.family == "qwen-turbo"
            else ModelConfig.qwen_image_21()
        )
        model = QwenImage21(
            quantize=8,
            model_path=str(model_path),
            model_config=config,
        )
    loaded_seconds = time.monotonic() - started
    progress = Progress()
    model.callbacks.register(progress)

    results = []
    for entry in specs:
        count = token_count(tokenizer, entry["prompt"])
        if count > spec["budget"]:
            raise RuntimeError(
                f"{entry['id']} uses {count} tokens, over {spec['budget']}"
            )
        task_started = time.monotonic()
        common = dict(
            seed=int(entry["seed"]),
            prompt=entry["prompt"],
            num_inference_steps=spec["steps"],
            width=1024,
            height=1024,
            guidance=spec["guidance"],
        )
        if args.family == "moss":
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
        output = output_dir / f"{entry['id']}.png"
        if output.exists():
            raise RuntimeError(f"refusing to overwrite {output}")
        image.save(path=str(output))
        from PIL import Image

        with Image.open(output) as decoded:
            decoded.load()
            if decoded.size != (1024, 1024) or decoded.mode != "RGB":
                raise RuntimeError(
                    f"{output} decoded as {decoded.mode} {decoded.size}"
                )
        data = output.read_bytes()
        results.append(
            {
                "id": entry["id"],
                "prompt": entry["prompt"],
                "seed": entry["seed"],
                "tokenCount": count,
                "tokenBudget": spec["budget"],
                "steps": spec["steps"],
                "guidance": spec["guidance"],
                "scheduler": spec["scheduler"],
                "path": str(output),
                "bytes": len(data),
                "sha256": hashlib.sha256(data).hexdigest(),
                "elapsedSeconds": time.monotonic() - task_started,
                "lastObservedSamplingStep": progress.completed,
            }
        )
        print(json.dumps(results[-1], ensure_ascii=False), flush=True)

    print(
        json.dumps(
            {
                "family": args.family,
                "modelPath": str(model_path),
                "quantization": 8,
                "loadSeconds": loaded_seconds,
                "tasks": results,
                "elapsedSeconds": time.monotonic() - started,
            },
            ensure_ascii=False,
        )
    )


if __name__ == "__main__":
    main()
