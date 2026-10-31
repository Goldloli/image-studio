import json
import os
import sys
from pathlib import Path
import subprocess
import tempfile
import threading
import types
import unittest
from unittest.mock import patch

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
from studio_mflux_worker_v3.backend import generate, token_budget


class TemplateTokenizer:
    template = "<user>{}</user>"
    use_chat_template = False
    add_special_tokens = True

    def __init__(self, count):
        self.tokenizer = self
        self.count = count

    def __call__(self, prompt, **kwargs):
        assert prompt.startswith("<user>") and prompt.endswith("</user>")
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
        assert len(messages) == 1 and messages[0]["role"] == "user"
        assert kwargs == dict(
            tokenize=False,
            add_generation_prompt=True,
            enable_thinking=True,
        )
        return "chat hello"

    def __call__(self, prompt, **kwargs):
        assert prompt.startswith("chat ")
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

    def test_backend_branches_use_exact_model_configs_and_sampling(self):
        calls = []

        class Image:
            def save(self, path):
                Path(path).write_bytes(b"png-data")

        class BaseQwen:
            def __init__(self, **kwargs):
                calls.append(("qwen-init", kwargs))
                self.callbacks = self

            def register(self, value):
                calls.append(("callback", value))

            def generate_image(self, **kwargs):
                calls.append(("qwen-generate", kwargs))
                return Image()

        class BaseMoss:
            def __init__(self, **kwargs):
                calls.append(("moss-init", kwargs))
                self.callbacks = self

            def register(self, value):
                calls.append(("callback", value))

            def generate_image(self, **kwargs):
                calls.append(("moss-generate", kwargs))
                return Image()

        class Config:
            @staticmethod
            def qwen_image_21():
                return "qwen-base"

            @staticmethod
            def qwen_image_21_turbo():
                return "qwen-turbo"

            @staticmethod
            def moss_image_sft():
                return "moss-sft"

        tokenizer = TemplateTokenizer(100)
        fake = {
            "mflux.models.common.tokenizer.tokenizer_loader": types.SimpleNamespace(
                TokenizerLoader=types.SimpleNamespace(
                    load_all=lambda definitions, path: {"qwen21": tokenizer}
                )
            ),
            "mflux.models.qwen21.weights.qwen_image21_weight_definition": types
            .SimpleNamespace(
                QwenImage21WeightDefinition=types.SimpleNamespace(
                    get_tokenizers=lambda: []
                )
            ),
            "mflux.models.common.config.model_config": types.SimpleNamespace(
                ModelConfig=Config
            ),
            "mflux.models.qwen21.variants.txt2img.qwen_image_21": types.SimpleNamespace(
                QwenImage21=BaseQwen
            ),
        }
        request = self.request(
            QWEN_TURBO_SNAPSHOT_ID, MODEL_SPECS[QWEN_TURBO_SNAPSHOT_ID]
        )
        with patch.dict(sys.modules, fake):
            candidate = generate(request, lambda *args: None, threading.Event())
        self.assertEqual(candidate["size_bytes"], 8)
        self.assertEqual(
            [value for kind, value in calls if kind == "qwen-init"][0]["model_config"],
            "qwen-turbo",
        )
        qwen_generate = [
            value for kind, value in calls if kind == "qwen-generate"
        ][0]
        self.assertEqual(qwen_generate["num_inference_steps"], 8)
        self.assertEqual(qwen_generate["guidance"], 1.0)

        calls.clear()
        (Path(request["output_dir"]) / "primary.png").unlink()
        tokenizer = ChatTokenizer(100)
        fake = {
            "mflux.models.common.tokenizer.tokenizer_loader": types.SimpleNamespace(
                TokenizerLoader=types.SimpleNamespace(
                    load_all=lambda definitions, path: {"qwen3_5": tokenizer}
                )
            ),
            "mflux.models.moss_image.weights.moss_image_weight_definition": types
            .SimpleNamespace(
                MOSSImageWeightDefinition=types.SimpleNamespace(
                    get_tokenizers=lambda: []
                )
            ),
            "mflux.models.common.config.model_config": types.SimpleNamespace(
                ModelConfig=Config
            ),
            "mflux.models.moss_image": types.SimpleNamespace(MOSSImage=BaseMoss),
        }
        request = self.request(
            MOSS_SNAPSHOT_ID, MODEL_SPECS[MOSS_SNAPSHOT_ID]
        )
        with patch.dict(sys.modules, fake):
            candidate = generate(request, lambda *args: None, threading.Event())
        self.assertEqual(candidate["size_bytes"], 8)
        self.assertEqual(
            [value for kind, value in calls if kind == "moss-init"][0]["model_config"],
            "moss-sft",
        )
        moss_generate = [
            value for kind, value in calls if kind == "moss-generate"
        ][0]
        self.assertEqual(moss_generate["num_inference_steps"], 50)
        self.assertEqual(moss_generate["guidance"], 3.0)
        self.assertEqual(moss_generate["uncond_skip_layers"], (22,))

    def test_protocol_v2_success_uses_selected_snapshot_identity(self):
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(QWEN_BASE_SNAPSHOT_ID, MODEL_SPECS[QWEN_BASE_SNAPSHOT_ID])
            worker_identity = identity(QWEN_BASE_SNAPSHOT_ID)
            hello = {
                "family": "image-studio.generation",
                "version": None,
                "session_id": "session",
                "request_id": "hello",
                "sequence": 0,
                "execution": None,
                "message": {
                    "type": "hello",
                    "versions": {"min": 2, "max": 2},
                    "build_id": "host",
                    "expected_worker": worker_identity,
                    "capabilities": [MODEL_SPECS[QWEN_BASE_SNAPSHOT_ID]["capability"]],
                },
            }
            execute = {
                "family": "image-studio.generation",
                "version": 2,
                "session_id": "session",
                "request_id": "execute",
                "sequence": 1,
                "execution": {
                    "job_id": "job",
                    "attempt_id": "attempt",
                    "execution_token": "token",
                },
                "message": {"type": "execute_v2", "request": request},
            }
            code = (
                "from studio_mflux_worker_v3 import backend; "
                "backend.generate=lambda request,progress,cancel: dict("
                "role='primary',data_type='image_rgb',relative_path='primary.png',"
                "width=1024,height=1024,size_bytes=9,sha256='a'*64); "
                "from studio_mflux_worker_v3.__main__ import main; raise SystemExit(main())"
            )
            child = subprocess.Popen(
                [
                    sys.executable,
                    "-c",
                    code,
                    "--model-snapshot-id",
                    QWEN_BASE_SNAPSHOT_ID,
                ],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                env={
                    **os.environ,
                    "PYTHONPATH": str(Path(__file__).resolve().parents[1]),
                },
            )
            try:
                child.stdin.write(
                    (json.dumps(hello) + "\n" + json.dumps(execute) + "\n").encode()
                )
                child.stdin.flush()
                accepted = json.loads(child.stdout.readline())
                self.assertEqual(accepted["message"]["identity"], worker_identity)
                self.assertEqual(
                    accepted["message"]["capabilities"],
                    [MODEL_SPECS[QWEN_BASE_SNAPSHOT_ID]["capability"]],
                )
                candidate = json.loads(child.stdout.readline())
                self.assertEqual(candidate["message"]["type"], "output_candidate_v2")
                self.assertEqual(candidate["execution"], execute["execution"])
                self.assertEqual(child.wait(timeout=5), 0, child.stderr.read().decode())
            finally:
                if child.poll() is None:
                    child.kill()
                    child.wait()
                child.stdin.close()
                child.stdout.close()
                child.stderr.close()


if __name__ == "__main__":
    unittest.main()
