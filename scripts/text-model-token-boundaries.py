#!/usr/bin/env python3
"""Create real template-token boundary fixtures for Qwen or MOSS."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path


BUDGETS = {"qwen": 2048, "qwen-turbo": 2048, "moss": 4096}


def raw_count(tokenizer, prompt):
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


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--family", choices=sorted(BUDGETS), required=True)
    parser.add_argument("--model-path", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    budget = BUDGETS[args.family]
    model_path = args.model_path.resolve(strict=True)

    os.environ["HF_HUB_OFFLINE"] = "1"
    os.environ["TRANSFORMERS_OFFLINE"] = "1"
    os.environ["TOKENIZERS_PARALLELISM"] = "false"
    from mflux.models.common.tokenizer.tokenizer_loader import TokenizerLoader

    if args.family == "moss":
        from mflux.models.moss_image.weights.moss_image_weight_definition import (
            MOSSImageWeightDefinition,
        )

        definitions = MOSSImageWeightDefinition.get_tokenizers()
        tokenizer_name = "qwen3_5"
    else:
        from mflux.models.qwen21.weights.qwen_image21_weight_definition import (
            QwenImage21WeightDefinition,
        )

        definitions = QwenImage21WeightDefinition.get_tokenizers()
        tokenizer_name = "qwen21"
    tokenizer = TokenizerLoader.load_all(definitions, str(model_path))[tokenizer_name]

    unit = " a"
    low, high = 1, budget * 4
    while low < high:
        middle = (low + high) // 2
        if raw_count(tokenizer, unit * middle) < budget:
            low = middle + 1
        else:
            high = middle
    over = unit * low
    below = unit * (low - 1)
    over_count = raw_count(tokenizer, over)
    below_count = raw_count(tokenizer, below)
    if over_count < budget or below_count >= budget:
        raise RuntimeError("could not bracket the exact token budget")
    if len(over.encode()) > 16 * 1024 or len(below.encode()) > 16 * 1024:
        raise RuntimeError("boundary fixture exceeds the 16 KiB product limit")

    document = {
        "schemaVersion": 1,
        "family": args.family,
        "tokenBudget": budget,
        "below": {
            "tokenCount": below_count,
            "prompt": below,
            "sha256": hashlib.sha256(below.encode()).hexdigest(),
        },
        "over": {
            "tokenCount": over_count,
            "prompt": over,
            "sha256": hashlib.sha256(over.encode()).hexdigest(),
        },
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(document, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(json.dumps(document, ensure_ascii=False))


if __name__ == "__main__":
    main()
