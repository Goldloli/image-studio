import sys
from pathlib import Path
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from studio_mflux_worker_v3 import (
    MOSS_SNAPSHOT_ID,
    MODEL_SPECS,
    QWEN_BASE_SNAPSHOT_ID,
    QWEN_TURBO_SNAPSHOT_ID,
    expected_parameters,
    identity,
)
from studio_mflux_worker_v3.__main__ import parse_arguments, validate_request
from studio_mflux_worker_v3.backend import token_budget


class TemplateTokenizer:
    template = "<user>{}</user>"
    use_chat_template = False
    add_special_tokens = True

    def __init__(self, count):
        self.tokenizer = self
        self.count = count

    def __call__(self, prompt, **kwargs):
        assert prompt == "<user>hello</user>"
        assert kwargs == dict(
            padding=False, truncation=False, add_special_tokens=True
        )
        return {"input_ids": list(range(self.count))}


class ChatTokenizer:
    template = None
    use_chat_template = True
    chat_template_kwargs = {"enable_thinking": True}
    add_special_tokens = True

    def __init__(self, count):
        self.tokenizer = self
        self.count = count

    def apply_chat_template(self, messages, **kwargs):
        assert messages == [{"role": "user", "content": "hello"}]
        assert kwargs == dict(
            tokenize=False,
            add_generation_prompt=True,
            enable_thinking=True,
        )
        return "chat hello"

    def __call__(self, prompt, **kwargs):
        assert prompt == "chat hello"
        assert kwargs == dict(
            padding=False, truncation=False, add_special_tokens=True
        )
        return {"input_ids": list(range(self.count))}


class WorkerTests(unittest.TestCase):
    def request(self, snapshot_id, spec):
        root = Path(self.temp.name).resolve()
        model = root / snapshot_id
        model.mkdir(exist_ok=True)
        output = root / "attempt"
        output.mkdir(exist_ok=True)
        return dict(
            model_id=spec["model_id"],
            capability_id=spec["capability"],
            recipe_id=spec["recipe"],
            recipe_version=1,
            runtime_lock_id=identity(snapshot_id)["runtime_lock_id"],
            model_snapshot_id=snapshot_id,
            instruction="a clean cobalt cube",
            seed=42,
            sampling_steps=spec["steps"],
            parameters=expected_parameters(snapshot_id, 42),
            inputs=[],
            outputs=[
                {
                    "role": "primary",
                    "data_type": "image_rgb",
                    "width": 1024,
                    "height": 1024,
                    "max_bytes": 32 * 1024 * 1024,
                }
            ],
            model_path=str(model),
            output_dir=str(output),
        )

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()

    def tearDown(self):
        self.temp.cleanup()

    def test_all_exact_recipes_validate_and_reject_cross_model_identity(self):
        for snapshot_id, spec in MODEL_SPECS.items():
            request = self.request(snapshot_id, spec)
            validate_request(request, identity(snapshot_id), spec)
            request["parameters"]["steps"] += 1
            with self.assertRaises(ValueError):
                validate_request(request, identity(snapshot_id), spec)

            request = self.request(snapshot_id, spec)
            request["capability_id"] = MODEL_SPECS[QWEN_BASE_SNAPSHOT_ID]["capability"]
            if snapshot_id == QWEN_BASE_SNAPSHOT_ID:
                request["capability_id"] = MODEL_SPECS[QWEN_TURBO_SNAPSHOT_ID][
                    "capability"
                ]
            with self.assertRaises(ValueError):
                validate_request(request, identity(snapshot_id), spec)

    def test_qwen_turbo_and_moss_keep_distinct_sampling_and_precision(self):
        turbo = expected_parameters(QWEN_TURBO_SNAPSHOT_ID, 7)
        moss = expected_parameters(MOSS_SNAPSHOT_ID, 7)
        self.assertEqual(turbo["steps"], 8)
        self.assertEqual(turbo["sampling_schedule"], "qwen-turbo-model-index-v1")
        self.assertEqual(turbo["token_budget"], 2048)
        self.assertEqual(moss["steps"], 50)
        self.assertEqual(moss["token_budget"], 4096)
        self.assertEqual(moss["uncond_skip_layers"], [22])
        self.assertTrue(moss["thinking"])
        self.assertNotEqual(
            turbo["quantized_components"], moss["quantized_components"]
        )

    def test_token_budget_counts_template_without_truncation(self):
        self.assertEqual(token_budget(TemplateTokenizer(2048), "hello", 2048), 2048)
        self.assertEqual(token_budget(ChatTokenizer(4096), "hello", 4096), 4096)
        with self.assertRaises(ValueError):
            token_budget(TemplateTokenizer(2049), "hello", 2048)
        with self.assertRaises(ValueError):
            token_budget(ChatTokenizer(4097), "hello", 4096)

    def test_worker_requires_explicit_snapshot_argument(self):
        self.assertEqual(
            parse_arguments(
                ["--model-snapshot-id", QWEN_BASE_SNAPSHOT_ID, "--probe"]
            ),
            (QWEN_BASE_SNAPSHOT_ID, True),
        )
        with self.assertRaises(ValueError):
            parse_arguments(["--probe"])
        with self.assertRaises(ValueError):
            parse_arguments(["--model-snapshot-id", "unknown"])


if __name__ == "__main__":
    unittest.main()
