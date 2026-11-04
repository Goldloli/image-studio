"""Unit tests for the Windows BiRefNet ONNX Runtime CPU worker.

The Windows v2 runtime is Python 3.12.10 without pytest, so these tests use the
standard library ``unittest`` runner. The frozen protocol/identity tests are
derived from ``tools/onnx-worker/tests/test_onnx_worker.py``; the real
end-to-end exchange lives in ``tests/run_real_protocol.py``.
"""

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
from studio_onnx_worker import (  # noqa: E402
    ACCEPTED_MODEL_IDS,
    BUILD_ID,
    CAPABILITIES,
    CAPABILITY,
    MODEL_ID,
    MODEL_SNAPSHOT_ID,
    RECIPE_ID,
    RUNTIME_LOCK_ID,
    expected_parameters,
    model_spec,
    worker_identity,
)
from studio_onnx_worker.__main__ import (  # noqa: E402
    DEVICE,
    parse_arguments,
    probe,
    read_frame,
    validate_request,
)
from studio_onnx_worker.backend import (  # noqa: E402
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
    name = "input_image"
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
        return [
            types.SimpleNamespace(
                name="output_image", type="tensor(float)", shape=[1, 1, 1024, 1024]
            )
        ]

    def run(self, output_names, feeds):
        self.assert_shape = feeds["input_image"].shape
        return [np.zeros((1, 1, 1024, 1024), dtype=np.float32)]


def fake_runtime():
    return types.SimpleNamespace(
        SessionOptions=FakeSessionOptions,
        InferenceSession=FakeSession,
        ExecutionMode=types.SimpleNamespace(ORT_SEQUENTIAL=1),
    )


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
            model_id=MODEL_ID,
            capability_id=CAPABILITY,
            recipe_id=RECIPE_ID,
            recipe_version=1,
            runtime_lock_id=RUNTIME_LOCK_ID,
            model_snapshot_id=MODEL_SNAPSHOT_ID,
            instruction="",
            seed=None,
            sampling_steps=None,
            parameters=expected_parameters(),
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

    def test_frozen_identity_matches_design_section_11(self):
        self.assertEqual(BUILD_ID, "studio-onnx-worker-windows-0.1.0")
        self.assertEqual(
            RUNTIME_LOCK_ID,
            "windows-python31210-torch2100-cu126-diff041-transformers53-v2",
        )
        self.assertEqual(MODEL_ID, "birefnet-general-epoch-244-windows-v1")
        self.assertEqual(
            MODEL_SNAPSHOT_ID, "birefnet-general-epoch-244-onnx-windows-v1"
        )
        self.assertEqual(
            CAPABILITY,
            "image.remove-background.birefnet-general-epoch-244.windows-v1",
        )
        self.assertEqual(CAPABILITIES, [CAPABILITY])
        self.assertEqual(RECIPE_ID, "birefnet-general-cpu-windows-v1")
        self.assertEqual(
            worker_identity(),
            {
                "build_id": BUILD_ID,
                "runtime_lock_id": RUNTIME_LOCK_ID,
                "model_snapshot_id": MODEL_SNAPSHOT_ID,
            },
        )
        self.assertEqual(
            model_spec(),
            {
                "snapshot_id": MODEL_SNAPSHOT_ID,
                "model_id": MODEL_ID,
                "capability": CAPABILITY,
                "recipe": RECIPE_ID,
            },
        )
        # The installer's model manifest id is accepted as a launch alias and echoed.
        self.assertEqual(
            worker_identity(MODEL_ID)["model_snapshot_id"], MODEL_ID
        )
        self.assertEqual(ACCEPTED_MODEL_IDS, (MODEL_SNAPSHOT_ID, MODEL_ID))
        for bad in ("", "unknown-snapshot", None, 7, MODEL_ID + " "):
            with self.assertRaises(ValueError):
                worker_identity(bad)
            with self.assertRaises(ValueError):
                model_spec(bad)

    def test_frozen_effective_parameters_match_design_section_11(self):
        self.assertEqual(
            expected_parameters(),
            {
                "recipe": RECIPE_ID,
                "recipe_version": 1,
                "provider": "CPUExecutionProvider",
                "precision": "float32",
                "preprocessing": "birefnet-1024-rgb-v1",
                "postprocessing": "birefnet-logits-sigmoid-v1",
                "intra_op_threads": 4,
                "inter_op_threads": 1,
                "execution_mode": "sequential",
            },
        )

    def test_parse_arguments_accepts_installer_forms(self):
        self.assertEqual(
            parse_arguments([]),
            {"snapshot_id": MODEL_SNAPSHOT_ID, "probe": False, "model_path": None},
        )
        self.assertEqual(
            parse_arguments(["--model-snapshot-id", MODEL_ID])["snapshot_id"], MODEL_ID
        )
        with tempfile.TemporaryDirectory() as temporary:
            model = Path(temporary).resolve()
            arguments = parse_arguments(
                ["--model-snapshot-id", MODEL_SNAPSHOT_ID, "--probe", str(model)]
            )
            self.assertTrue(arguments["probe"])
            self.assertEqual(arguments["model_path"], model)
            legacy = parse_arguments(["--probe", str(model)])
            self.assertTrue(legacy["probe"])
            self.assertEqual(legacy["snapshot_id"], MODEL_SNAPSHOT_ID)
        for bad in (
            ["--model-snapshot-id"],
            ["--model-snapshot-id", "unknown-snapshot"],
            ["--model-snapshot-id", MODEL_ID, "--model-snapshot-id", MODEL_ID],
            ["--probe"],
            ["--unknown"],
            ["extra"],
            ["--probe", "relative/path"],
        ):
            with self.assertRaises(ValueError):
                parse_arguments(bad)

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

    def test_strict_request_rejects_wrong_identity_and_tool_fields(self):
        identity = worker_identity()
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary).resolve())
            validate_request(request, identity)
            for changes in (
                {"model_id": "birefnet-general-epoch-244"},
                {"capability_id": "image.remove-background.birefnet-general-epoch-244.v1"},
                {"recipe_id": "birefnet-general-cpu-v1"},
                {"runtime_lock_id": "onnxruntime-1.31.1-py3.13.12-macos-arm64-cpu-v1"},
                {"model_snapshot_id": MODEL_ID},
                {"instruction": "remove it"},
                {"seed": 42},
                {"sampling_steps": 4},
                {"parameters": {**request["parameters"], "postprocessing": "double"}},
                {"parameters": {**request["parameters"], "provider": "CUDAExecutionProvider"}},
            ):
                with self.assertRaises(ValueError):
                    validate_request({**request, **changes}, identity)
            wrong_outputs = [
                dict(request["outputs"][0]),
                dict(request["outputs"][0], role="mask", data_type="mask"),
            ]
            with self.assertRaises(ValueError):
                validate_request({**request, "outputs": wrong_outputs[:1]}, identity)
            # Output specs are role-keyed, so wire order alone is not identity.
            validate_request(
                {**request, "outputs": list(reversed(request["outputs"]))}, identity
            )

    def test_fake_onnx_session_returns_rgba_and_mask(self):
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary).resolve())
            with patch.dict(sys.modules, {"onnxruntime": fake_runtime()}):
                candidates = generate(request, lambda *args: None, threading.Event())
            self.assertEqual([item["role"] for item in candidates], ["primary", "mask"])
            output = Path(request["output_dir"])
            from PIL import Image

            with Image.open(output / "primary.png") as image:
                self.assertEqual(image.mode, "RGBA")
                self.assertEqual(image.size, (20, 24))
                self.assertEqual(image.getpixel((0, 0)), (12, 34, 56, 100))
            with Image.open(output / "mask.png") as mask:
                self.assertEqual(mask.mode, "L")
                self.assertEqual(mask.getpixel((0, 0)), 100)

    def test_probe_reports_windows_cpu_identity(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            model = root / "model"
            model.mkdir()
            (model / "BiRefNet-general-epoch_244.onnx").write_bytes(b"fixture")
            identity = worker_identity()
            with patch.dict(sys.modules, {"onnxruntime": fake_runtime()}):
                report = probe(str(model), identity)
            self.assertEqual(report["identity"], identity)
            self.assertEqual(report["device"], DEVICE)
            self.assertEqual(report["device"], "windows-cpu")
            self.assertTrue(report["device_name"])
            self.assertEqual(
                report["python"],
                f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}",
            )
            self.assertEqual(
                report["graph"]["providers"], ["CPUExecutionProvider"]
            )
            self.assertEqual(report["graph"]["input"]["name"], "input_image")
            self.assertEqual(report["graph"]["model_file"], "BiRefNet-general-epoch_244.onnx")

    def test_protocol_emits_two_candidates(self):
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary).resolve())
            identity = worker_identity()
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
                    expected_worker=identity,
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
            package_root = str(Path(__file__).resolve().parents[1])
            code = (
                f"import sys; sys.path.insert(0, {package_root!r}); "
                "from studio_onnx_worker import backend; "
                f"backend.generate=lambda request,progress,cancel: {candidates!r}; "
                "from studio_onnx_worker.__main__ import main; raise SystemExit(main())"
            )
            child = subprocess.Popen(
                [sys.executable, "-c", code],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                env=os.environ.copy(),
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
                self.assertEqual(child.wait(timeout=10), 0, child.stderr.read().decode())
            finally:
                if child.poll() is None:
                    child.kill()
                    child.wait()
                child.stdin.close()
                child.stdout.close()
                child.stderr.close()
            self.assertEqual(messages[0]["message"]["type"], "hello_accepted")
            self.assertEqual(messages[0]["message"]["identity"], identity)
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
