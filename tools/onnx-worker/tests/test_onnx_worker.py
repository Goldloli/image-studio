import hashlib
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

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from studio_onnx_worker import CAPABILITIES, IDENTITY
from studio_onnx_worker.__main__ import (
    EXPECTED_PARAMETERS,
    read_frame,
    run,
    validate_request,
)
from studio_onnx_worker.backend import (
    compose_mask,
    generate,
    resize_half_pixel_uint8,
    resize_align_corners,
    sigmoid_logits,
)


class FakeSessionOptions:
    def __init__(self):
        self.intra_op_num_threads = None
        self.inter_op_num_threads = None
        self.execution_mode = None


class FakeInput:
    name = "input"
    type = "tensor(float)"
    shape = [1, 3, 1024, 1024]


class FakeSession:
    def __init__(self, *args, **kwargs):
        self.args = args
        self.kwargs = kwargs

    def get_providers(self):
        return ["CPUExecutionProvider"]

    def get_inputs(self):
        return [FakeInput()]

    def get_outputs(self):
        return [types.SimpleNamespace(name="logits", type="tensor(float)", shape=[1, 1, 1024, 1024])]

    def run(self, output_names, feeds):
        self.assert_shape = feeds["input"].shape
        return [np.zeros((1, 1, 1024, 1024), dtype=np.float32)]


class WorkerTests(unittest.TestCase):
    def request(self, root):
        model = root / "model"
        model.mkdir(exist_ok=True)
        (model / "BiRefNet-general-epoch_244.onnx").write_bytes(b"fixture")
        output = root / "attempt"
        output.mkdir(exist_ok=True)
        from PIL import Image

        source = output / "input.png"
        Image.new("RGBA", (20, 24), (12, 34, 56, 200)).save(source)
        sha256 = hashlib.sha256(source.read_bytes()).hexdigest()
        return dict(
            model_id="birefnet-general-epoch-244",
            capability_id=CAPABILITIES[0],
            recipe_id="birefnet-general-cpu-v1",
            recipe_version=1,
            runtime_lock_id=IDENTITY["runtime_lock_id"],
            model_snapshot_id=IDENTITY["model_snapshot_id"],
            instruction="",
            seed=None,
            sampling_steps=None,
            parameters=dict(EXPECTED_PARAMETERS),
            inputs=[
                dict(
                    role="source",
                    path=str(source.resolve()),
                    width=20,
                    height=24,
                    sha256=sha256,
                )
            ],
            outputs=[
                dict(
                    role="primary",
                    data_type="image_rgba",
                    width=20,
                    height=24,
                    max_bytes=32 * 1024 * 1024,
                ),
                dict(
                    role="mask",
                    data_type="mask",
                    width=20,
                    height=24,
                    max_bytes=32 * 1024 * 1024,
                ),
            ],
            model_path=str(model.resolve()),
            output_dir=str(output.resolve()),
        )

    def test_resize_and_alpha_rules(self):
        values = np.asarray([[0.0, 1.0], [1.0, 0.0]], dtype=np.float32)
        resized = resize_align_corners(values, 4, 3)
        self.assertEqual(resized.shape, (3, 4))
        self.assertEqual(float(resized[0, 0]), 0.0)
        self.assertEqual(float(resized[0, -1]), 1.0)
        alpha = compose_mask(
            np.asarray([[0.5, 0.5]], dtype=np.float32),
            np.asarray([[200, 0]], dtype=np.uint8),
        )
        self.assertEqual(alpha.tolist(), [[100, 0]])
        logits = np.asarray([[-1000.0, 0.0, 1000.0]], dtype=np.float32)
        probabilities = sigmoid_logits(logits)
        self.assertEqual(probabilities[0, 0], 0.0)
        self.assertAlmostEqual(float(probabilities[0, 1]), 0.5, places=6)
        self.assertEqual(probabilities[0, 2], 1.0)

    def test_preprocessing_matches_official_cv2_linear_resize(self):
        try:
            import cv2
        except ImportError:
            self.skipTest("OpenCV reference is not installed")
        rng = np.random.default_rng(7)
        source = rng.integers(0, 256, size=(17, 13, 3), dtype=np.uint8)
        expected = cv2.resize(source, (1024, 1024), interpolation=cv2.INTER_LINEAR)
        actual = resize_half_pixel_uint8(source, 1024, 1024)
        self.assertLessEqual(
            int(np.abs(expected.astype(np.int16) - actual.astype(np.int16)).max()),
            1,
        )

    def test_strict_request_rejects_fake_tool_fields(self):
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary).resolve())
            validate_request(request)
            for changes in (
                {"instruction": "remove it"},
                {"seed": 42},
                {"sampling_steps": 4},
                {"parameters": {**request["parameters"], "postprocessing": "double"}},
            ):
                with self.assertRaises(ValueError):
                    validate_request({**request, **changes})

    def test_fake_onnx_session_returns_rgba_and_mask(self):
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary).resolve())
            runtime = types.SimpleNamespace(
                SessionOptions=FakeSessionOptions,
                InferenceSession=FakeSession,
                ExecutionMode=types.SimpleNamespace(ORT_SEQUENTIAL=1),
            )
            from PIL import Image

            with patch.dict(sys.modules, {"onnxruntime": runtime}):
                candidates = generate(request, lambda *args: None, threading.Event())
            self.assertEqual([item["role"] for item in candidates], ["primary", "mask"])
            output = Path(request["output_dir"])
            with Image.open(output / "primary.png") as image:
                self.assertEqual(image.mode, "RGBA")
                self.assertEqual(image.size, (20, 24))
                self.assertEqual(image.getpixel((0, 0)), (12, 34, 56, 100))
            with Image.open(output / "mask.png") as mask:
                self.assertEqual(mask.mode, "L")
                self.assertEqual(mask.getpixel((0, 0)), 100)

    def test_protocol_emits_two_candidates(self):
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary).resolve())
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
            candidates = [
                dict(
                    role=role,
                    data_type=data_type,
                    relative_path=f"{role}.png",
                    width=20,
                    height=24,
                    size_bytes=9,
                    sha256="a" * 64,
                )
                for role, data_type in (
                    ("primary", "image_rgba"),
                    ("mask", "mask"),
                )
            ]
            code = (
                "from studio_onnx_worker import backend; "
                f"backend.generate=lambda request,progress,cancel: {candidates!r}; "
                "from studio_onnx_worker.__main__ import main; raise SystemExit(main())"
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
                messages = [
                    json.loads(child.stdout.readline()),
                    json.loads(child.stdout.readline()),
                    json.loads(child.stdout.readline()),
                ]
                self.assertEqual(child.wait(timeout=5), 0, child.stderr.read().decode())
            finally:
                if child.poll() is None:
                    child.kill()
                    child.wait()
                child.stdin.close()
                child.stdout.close()
                child.stderr.close()
            self.assertEqual(messages[0]["message"]["type"], "hello_accepted")
            self.assertEqual(
                [item["message"]["candidate"]["role"] for item in messages[1:]],
                ["primary", "mask"],
            )

    def test_frame_limit(self):
        self.assertEqual(read_frame(io.BytesIO(b'{"a":1}\n')), {"a": 1})
        for value in (b" " * 65537 + b"\n", b"{}"):
            with self.assertRaises(ValueError):
                read_frame(io.BytesIO(value))


if __name__ == "__main__":
    unittest.main()
