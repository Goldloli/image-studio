import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import types
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from studio_mflux_worker_v2 import CAPABILITIES, IDENTITY, TEXT_CAPABILITY
from studio_mflux_worker_v2.__main__ import read_frame, run, validate_request
from studio_mflux_worker_v2.backend import generate, token_budget


class Tokenizer:
    template = None
    use_chat_template = True
    chat_template_kwargs = {"enable_thinking": False}
    add_special_tokens = True

    def __init__(self, count):
        self.tokenizer = self
        self.count = count
        self.template_called = False

    def apply_chat_template(self, messages, **kwargs):
        assert kwargs == dict(
            tokenize=False, add_generation_prompt=True, enable_thinking=False
        )
        self.template_called = True
        return "template " + messages[0]["content"]

    def __call__(self, prompt, **kwargs):
        assert self.template_called
        assert kwargs == dict(
            padding=False, truncation=False, add_special_tokens=True
        )
        return {"input_ids": list(range(self.count))}


class WorkerTests(unittest.TestCase):
    def request(self, root, edit=False):
        model = root / "model"
        model.mkdir(exist_ok=True)
        output = root / "attempt"
        output.mkdir(exist_ok=True)
        inputs = []
        recipe = "klein4b-text-v1"
        capability = TEXT_CAPABILITY
        if edit:
            from PIL import Image

            source = output / "input.png"
            Image.new("RGB", (20, 24), (12, 34, 56)).save(source)
            inputs = [
                dict(
                    role="source",
                    path=str(source),
                    width=20,
                    height=24,
                    sha256="a" * 64,
                )
            ]
            recipe = "klein4b-edit-v1"
            capability = "image.edit.flux2-klein-4b.v1"
        return dict(
            model_id="flux2-klein-4b-distilled",
            capability_id=capability,
            recipe_id=recipe,
            recipe_version=1,
            runtime_lock_id=IDENTITY["runtime_lock_id"],
            model_snapshot_id=IDENTITY["model_snapshot_id"],
            instruction="make a cobalt cube",
            seed=42,
            sampling_steps=4,
            parameters={
                "seed": 42,
                "steps": 4,
                "quantization": 4,
                "guidance": 1.0,
                "scheduler": "flow_match_euler_discrete",
                "recipe": recipe,
                "recipe_version": 1,
                "token_budget": 512,
                "thinking": False,
                "truncation": False,
            },
            inputs=inputs,
            outputs=[
                dict(
                    role="primary",
                    data_type="image_rgb",
                    width=1024,
                    height=1024,
                    max_bytes=32 * 1024 * 1024,
                )
            ],
            model_path=str(model.resolve()),
            output_dir=str(output.resolve()),
        )

    def test_protocol_v2_success_exits_cleanly(self):
        with tempfile.TemporaryDirectory() as temp:
            request = self.request(Path(temp))
            hello = dict(
                family="image-studio.generation",
                version=None,
                session_id="session",
                request_id="hello",
                sequence=0,
                execution=None,
                message=dict(
                    type="hello",
                    versions=dict(min=2, max=2),
                    build_id="host",
                    expected_worker=IDENTITY,
                    capabilities=CAPABILITIES,
                ),
            )
            execute = dict(
                family="image-studio.generation",
                version=2,
                session_id="session",
                request_id="execute",
                sequence=1,
                execution=dict(
                    job_id="job", attempt_id="attempt", execution_token="token"
                ),
                message=dict(type="execute_v2", request=request),
            )
            code = (
                "from studio_mflux_worker_v2 import backend; "
                "backend.generate=lambda request,progress,cancel: dict("
                "role='primary',data_type='image_rgb',relative_path='primary.png',"
                "width=1024,height=1024,size_bytes=9,sha256='a'*64); "
                "from studio_mflux_worker_v2.__main__ import main; raise SystemExit(main())"
            )
            child = subprocess.Popen(
                [sys.executable, "-c", code],
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
                self.assertEqual(accepted["message"]["selected_version"], 2)
                self.assertEqual(accepted["message"]["capabilities"], CAPABILITIES)
                candidate = json.loads(child.stdout.readline())
                self.assertEqual(candidate["message"]["type"], "output_candidate_v2")
                self.assertEqual(candidate["execution"], execute["execution"])
                self.assertEqual(candidate["sequence"], 1)
                self.assertEqual(child.wait(timeout=5), 0, child.stderr.read().decode())
            finally:
                if child.poll() is None:
                    child.kill()
                    child.wait()
                child.stdin.close()
                child.stdout.close()
                child.stderr.close()

    def test_template_budget_and_strict_request(self):
        self.assertEqual(token_budget(Tokenizer(512), "中文"), 512)
        with self.assertRaisesRegex(ValueError, "513 token"):
            token_budget(Tokenizer(513), "中文")
        with tempfile.TemporaryDirectory() as temp:
            request = self.request(Path(temp))
            validate_request(request)
            mutations = [
                {"seed": None},
                {"sampling_steps": 9},
                {"text": "unknown"},
                {"parameters": {**request["parameters"], "steps": 9}},
            ]
            for changes in mutations:
                with self.assertRaises(ValueError):
                    validate_request({**request, **changes})

    def test_fixed_text_and_edit_branches_do_not_use_img2img(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            calls = []

            class Registry:
                def register(self, value):
                    self.value = value

            class BaseModel:
                def __init__(self, **kwargs):
                    calls.append(("init", kwargs))
                    self.callbacks = Registry()

                def _encode_prompt_pair(self, **kwargs):
                    return None

                def generate_image(self, **kwargs):
                    calls.append(("generate", kwargs))
                    self.callbacks.value.call_before_loop()
                    for step in range(4):
                        self.callbacks.value.call_in_loop(t=step)
                    self.callbacks.value.call_after_loop()
                    return types.SimpleNamespace(
                        save=lambda path: Path(path).open("wb").write(b"primary")
                    )

            class TextModel(BaseModel):
                pass

            class EditModel(BaseModel):
                pass

            fake = {
                "mflux.models.common.tokenizer.tokenizer_loader": types.SimpleNamespace(
                    TokenizerLoader=types.SimpleNamespace(
                        load_all=lambda *args: {"qwen3": Tokenizer(10)}
                    )
                ),
                "mflux.models.flux2.weights.flux2_weight_definition": types.SimpleNamespace(
                    Flux2KleinWeightDefinition=types.SimpleNamespace(
                        get_tokenizers=lambda: []
                    )
                ),
                "mflux.models.flux2.variants.txt2img.flux2_klein": types.SimpleNamespace(
                    Flux2Klein=TextModel
                ),
                "mflux.models.flux2.variants.edit.flux2_klein_edit": types.SimpleNamespace(
                    Flux2KleinEdit=EditModel
                ),
            }
            text_request = self.request(root)
            with patch.dict(sys.modules, fake):
                text = generate(text_request, lambda *args: None, threading.Event())
            self.assertEqual(text["role"], "primary")
            text_generate = [v for kind, v in calls if kind == "generate"][-1]
            self.assertNotIn("image_paths", text_generate)
            self.assertNotIn("image_strength", text_generate)

            edit_request = self.request(root, edit=True)
            edit_request["output_dir"] = str((root / "edit-attempt").resolve())
            Path(edit_request["output_dir"]).mkdir(exist_ok=True)
            edit_request["inputs"][0]["path"] = str(
                (Path(edit_request["output_dir"]) / "input.png").resolve()
            )
            from PIL import Image

            Image.new("RGB", (20, 24), (1, 2, 3)).save(
                edit_request["inputs"][0]["path"]
            )
            calls.clear()
            with patch.dict(sys.modules, fake):
                with patch(
                    "studio_mflux_worker_v2.backend._inspect_edit_source",
                    return_value=None,
                ):
                    edit = generate(
                        edit_request, lambda *args: None, threading.Event()
                    )
            self.assertEqual(edit["role"], "primary")
            edit_generate = [v for kind, v in calls if kind == "generate"][-1]
            self.assertEqual(edit_generate["image_paths"], [edit_request["inputs"][0]["path"]])
            self.assertNotIn("image_strength", edit_generate)

    def test_frame_limit(self):
        self.assertEqual(read_frame(io.BytesIO(b'{"a":1}\n')), {"a": 1})
        for value in (b" " * 65537 + b"\n", b"{}"):
            with self.assertRaises(ValueError):
                read_frame(io.BytesIO(value))

    def test_wrong_identity_rejected_before_inference(self):
        hello = dict(
            family="image-studio.generation",
            version=None,
            session_id="session",
            request_id="hello",
            sequence=0,
            execution=None,
            message=dict(
                type="hello",
                versions=dict(min=2, max=2),
                build_id="host",
                expected_worker={**IDENTITY, "runtime_lock_id": "wrong"},
                capabilities=CAPABILITIES,
            ),
        )
        with self.assertRaises(ValueError):
            run(io.StringIO(), io.BytesIO((json.dumps(hello) + "\n").encode()))


if __name__ == "__main__":
    unittest.main()
