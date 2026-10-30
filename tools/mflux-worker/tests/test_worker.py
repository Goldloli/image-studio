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
from studio_mflux_worker import CAPABILITY, IDENTITY
from studio_mflux_worker.__main__ import read_frame, run, validate_request
from studio_mflux_worker.backend import Cancelled, checkpoint, generate, token_budget


class Tokenizer:
    template = None
    use_chat_template = True
    chat_template_kwargs = {'enable_thinking': True}
    add_special_tokens = True

    def __init__(self, count):
        self.tokenizer = self
        self.count = count
        self.template_called = False

    def apply_chat_template(self, messages, **kwargs):
        assert kwargs == dict(tokenize=False, add_generation_prompt=True, enable_thinking=True)
        self.template_called = True
        return 'template ' + messages[0]['content']

    def __call__(self, prompt, **kwargs):
        assert self.template_called and kwargs == dict(padding=False, truncation=False, add_special_tokens=True)
        return {'input_ids': list(range(self.count))}


class WorkerTests(unittest.TestCase):
    def test_protocol_success_exits_cleanly_with_control_reader_waiting(self):
        with tempfile.TemporaryDirectory() as temp:
            request = self.request(Path(temp))
            hello = dict(family='image-studio.generation', version=None, session_id='session', request_id='hello', sequence=0,
                         execution=None, message=dict(type='hello', versions=dict(min=1,max=1), build_id='host', expected_worker=IDENTITY, capabilities=[CAPABILITY]))
            execute = dict(family='image-studio.generation', version=1, session_id='session', request_id='execute', sequence=1,
                           execution=dict(job_id='job',attempt_id='attempt',execution_token='token'),message=dict(type='execute',request=request))
            code = "from studio_mflux_worker import backend; backend.generate=lambda request,progress,cancel: dict(relative_path='result.png',width=request['width'],height=request['height'],size_bytes=9,sha256='a'*64); from studio_mflux_worker.__main__ import main; raise SystemExit(main())"
            child = subprocess.Popen([sys.executable, '-c', code], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                     env={**os.environ, 'PYTHONPATH': str(Path(__file__).resolve().parents[1])})
            try:
                child.stdin.write((json.dumps(hello)+'\n'+json.dumps(execute)+'\n').encode()); child.stdin.flush()
                response = json.loads(child.stdout.readline())
                self.assertEqual(response['message']['type'], 'hello_accepted')
                candidate = json.loads(child.stdout.readline())
                self.assertEqual(candidate['message']['type'], 'output_candidate')
                self.assertEqual(candidate['execution'], execute['execution'])
                self.assertEqual(candidate['sequence'], 1)
                self.assertEqual(child.wait(timeout=5), 0, child.stderr.read().decode())
            finally:
                if child.poll() is None: child.kill(); child.wait()
                child.stdin.close(); child.stdout.close(); child.stderr.close()
    def test_template_budget_includes_special_tokens_without_truncation(self):
        self.assertEqual(token_budget(Tokenizer(512), '中文'), 512)
        with self.assertRaisesRegex(ValueError, '513 token'):
            token_budget(Tokenizer(513), '中文')

    def test_frame_limit_and_incomplete_frame(self):
        self.assertEqual(read_frame(io.BytesIO(b'{"a":1}\n')), {'a': 1})
        for value in (b' ' * 65537 + b'\n', b'{}'):
            with self.assertRaises(ValueError):
                read_frame(io.BytesIO(value))

    def test_native_and_python_stdout_do_not_pollute_protocol(self):
        code = "from studio_mflux_worker.__main__ import isolate_stdout; import os; p=isolate_stdout(); print('python log'); os.write(1,b'native log\\n'); p.write('{}\\n'); p.flush()"
        result = subprocess.run([sys.executable, '-c', code], capture_output=True,
                                env={**os.environ, 'PYTHONPATH': str(Path(__file__).resolve().parents[1])})
        self.assertEqual(result.stdout, b'{}\n')
        self.assertIn(b'native log', result.stderr)
        self.assertIn(b'python log', result.stderr)

    def request(self, root):
        model = root / 'model'; model.mkdir(exist_ok=True)
        output = root / 'attempt'; output.mkdir(exist_ok=True)
        return dict(prompt='test 中文', width=1024, height=768, seed=42,
                    num_inference_steps=9, quantization_bits=4,
                    model_path=str(model.resolve()), output_dir=str(output.resolve()))

    def test_request_recipe_seed_and_local_paths_fail_closed(self):
        with tempfile.TemporaryDirectory() as temp:
            request = self.request(Path(temp)); validate_request(request)
            for changes in ({'seed': True}, {'seed': -1}, {'num_inference_steps': 4},
                            {'width': 1000}, {'model_path': 'Tongyi-MAI/Z-Image-Turbo'}, {'unknown': 1}):
                with self.assertRaises(ValueError):
                    validate_request({**request, **changes})

    def test_fixed_api_calls_and_real_callback_stages(self):
        with tempfile.TemporaryDirectory() as temp:
            request = self.request(Path(temp)); calls = []; progress = []
            class Registry:
                def register(self, value): self.value = value
            class Model:
                def __init__(self, **kwargs): calls.append(kwargs); self.callbacks = Registry()
                def generate_image(self, **kwargs):
                    calls.append(kwargs)
                    self._encode_prompts(); self.callbacks.value.call_before_loop()
                    for t in range(9): self.callbacks.value.call_in_loop(t=t)
                    self.callbacks.value.call_after_loop(); self._decode_latents()
                    return types.SimpleNamespace(save=lambda path: Path(path).write_bytes(b'candidate'))
                def _encode_prompts(self, **kwargs): return None
                def _decode_latents(self, **kwargs): return None
            fake = {
                'mflux.models.common.tokenizer.tokenizer_loader': types.SimpleNamespace(TokenizerLoader=types.SimpleNamespace(load_all=lambda *args: {'z_image': Tokenizer(10)})),
                'mflux.models.z_image.weights.z_image_weight_definition': types.SimpleNamespace(ZImageWeightDefinition=types.SimpleNamespace(get_tokenizers=lambda: [])),
                'mflux.models.z_image.variants.z_image': types.SimpleNamespace(ZImage=Model),
            }
            with patch.dict(sys.modules, fake):
                candidate = generate(request, lambda *args: progress.append(args), threading.Event())
            self.assertEqual(calls[0], {'quantize': 4, 'model_path': request['model_path']})
            self.assertEqual(calls[1]['num_inference_steps'], 9)
            self.assertEqual(calls[1]['scheduler'], 'linear')
            self.assertEqual(calls[1]['guidance'], 0.0)
            self.assertEqual([p[0] for p in progress], ['loading', 'encoding'] + ['sampling']*10 + ['decoding', 'writing'])
            self.assertEqual(candidate['relative_path'], 'result.png')
            self.assertEqual(candidate['size_bytes'], 9)

    def test_cancel_checkpoint_propagates(self):
        event = threading.Event(); event.set()
        with self.assertRaises(Cancelled): checkpoint(event)

    def test_wrong_identity_rejected_before_inference(self):
        hello = dict(family='image-studio.generation', version=None, session_id='session', request_id='hello', sequence=0,
                     execution=None, message=dict(type='hello', versions=dict(min=1,max=1), build_id='host',
                     expected_worker={**IDENTITY,'runtime_lock_id':'wrong'}, capabilities=[CAPABILITY]))
        with self.assertRaises(ValueError):
            run(io.StringIO(), io.BytesIO((json.dumps(hello)+'\n').encode()))


if __name__ == '__main__':
    unittest.main()
