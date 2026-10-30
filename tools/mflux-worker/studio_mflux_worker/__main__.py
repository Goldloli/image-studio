"""Generation v1 JSONL. This module imports no ML library until stdout is isolated."""
import json
import os
import sys
import threading
from pathlib import Path
from . import IDENTITY, CAPABILITY

MAX_FRAME = 64 * 1024
FAMILY = 'image-studio.generation'


def isolate_stdout():
    sys.stdout.flush()
    protocol = os.fdopen(os.dup(1), 'w', encoding='utf-8', buffering=1)
    os.dup2(2, 1)  # Python print and C/native writes to fd 1 both become logs.
    sys.stdout = sys.stderr
    return protocol


def read_frame(stream):
    line = stream.readline(MAX_FRAME + 2)
    if not line:
        raise EOFError('control channel closed')
    if not line.endswith(b'\n') or len(line.rstrip(b'\r\n')) > MAX_FRAME:
        raise ValueError('control frame exceeds 64 KiB or lacks newline')
    return json.loads(line)


def fields(value, expected):
    if not isinstance(value, dict) or set(value) != set(expected):
        raise ValueError('unknown or missing protocol fields')


def validate_envelope(value):
    fields(value, ('family', 'version', 'session_id', 'request_id', 'sequence', 'execution', 'message'))
    if value['family'] != FAMILY or type(value['sequence']) is not int or value['sequence'] < 0:
        raise ValueError('wrong generation family/sequence')
    for key in ('session_id', 'request_id'):
        ident = value[key]
        if not isinstance(ident, str) or not 0 < len(ident.encode()) <= 256 or any(ord(c) < 32 for c in ident):
            raise ValueError('invalid identity')


def validate_request(request):
    fields(request, ('prompt', 'width', 'height', 'seed', 'num_inference_steps', 'quantization_bits', 'model_path', 'output_dir'))
    prompt = request['prompt']
    if not isinstance(prompt, str) or not prompt.strip() or '\0' in prompt or len(prompt.encode()) > 16384:
        raise ValueError('提示词为空或超过 16 KiB 上限。')
    if (request['width'], request['height']) not in ((1024, 1024), (1024, 768), (768, 1024)) or request['num_inference_steps'] != 9 or request['quantization_bits'] != 4:
        raise ValueError('不支持的生成配方。')
    if type(request['seed']) is not int or not 0 <= request['seed'] <= 4294967295:
        raise ValueError('随机种子必须为 u32。')
    for name in ('model_path', 'output_dir'):
        path = Path(request[name])
        if not path.is_absolute() or path.is_symlink() or not path.is_dir() or path.resolve() != path:
            raise ValueError('仅允许规范化本地目录。')
    if Path(request['model_path']) == Path(request['output_dir']) or Path(request['model_path']) in Path(request['output_dir']).parents:
        raise ValueError('输出目录不能位于模型内。')


def probe():
    import importlib.metadata
    import platform
    import mlx.core as mx
    import mflux  # noqa: F401: verify real import, no model loading
    import torch  # noqa: F401
    import transformers  # noqa: F401
    mx.set_default_device(mx.gpu)
    result = mx.array([1.0, 2.0]) + mx.array([2.0, 3.0])
    mx.eval(result)
    if result.tolist() != [3.0, 5.0] or platform.machine() != 'arm64':
        raise RuntimeError('MLX device probe failed')
    return dict(identity=IDENTITY, python=platform.python_version(), device='gpu',
                device_name=mx.metal.device_info()['device_name'],
                packages={name: importlib.metadata.version(name) for name in ('mflux', 'mlx', 'torch', 'transformers')})


def run(protocol, stream):
    from .backend import Cancelled, generate
    hello = read_frame(stream)
    validate_envelope(hello)
    msg = hello['message']
    fields(msg, ('type', 'versions', 'build_id', 'expected_worker', 'capabilities'))
    if (msg['type'] != 'hello' or hello['sequence'] != 0 or hello['version'] is not None or hello['execution'] is not None or msg['expected_worker'] != IDENTITY or msg['capabilities'] != [CAPABILITY] or not msg['versions']['min'] <= 1 <= msg['versions']['max']):
        raise ValueError('generation handshake identity mismatch')
    sequence = 0
    active = None

    def send(message):
        nonlocal sequence
        source = active or hello
        envelope = dict(family=FAMILY, version=1, session_id=hello['session_id'],
                        request_id=source['request_id'], sequence=sequence,
                        execution=source['execution'], message=message)
        data = json.dumps(envelope, ensure_ascii=False, separators=(',', ':'))
        if len(data.encode()) > MAX_FRAME:
            raise ValueError('outgoing frame too large')
        protocol.write(data + '\n')
        protocol.flush()
        sequence += 1

    send(dict(type='hello_accepted', versions=dict(min=1, max=1), selected_version=1,
              identity=IDENTITY, capabilities=[CAPABILITY]))
    active = read_frame(stream)
    validate_envelope(active)
    fields(active['message'], ('type', 'request'))
    if active['session_id'] != hello['session_id'] or active['sequence'] != 1 or active['version'] != 1 or active['message']['type'] != 'execute':
        raise ValueError('execute identity mismatch')
    fields(active['execution'], ('job_id', 'attempt_id', 'execution_token'))
    cancel = threading.Event()
    control_error = []

    def control():
        try:
            value = read_frame(stream)
            validate_envelope(value)
            if value['session_id'] != active['session_id'] or value['request_id'] != active['request_id'] or value['execution'] != active['execution'] or value['sequence'] != 2 or value['version'] != 1 or value['message'] != {'type': 'cancel'}:
                raise ValueError('invalid cancel')
            cancel.set()
        except EOFError:
            # Supervisor vanished: native/GPU work must not outlive its control owner.
            os._exit(70)
        except Exception as exc:
            control_error.append(exc)
            cancel.set()

    threading.Thread(target=control, daemon=True).start()
    try:
        request = active['message']['request']
        validate_request(request)
        def progress(stage, completed=None, total=None):
            send(dict(type='progress', stage=stage, completed=completed, total=total))
        candidate = generate(request, progress, cancel)
        if cancel.is_set():
            raise Cancelled()
        send(dict(type='output_candidate', candidate=candidate))
    except Cancelled:
        if control_error:
            send(dict(type='error', code='protocol_error', summary=str(control_error[0])[:1024]))
        else:
            send(dict(type='cancelled'))
    except Exception as exc:
        code = 'validation_error' if isinstance(exc, ValueError) else 'inference_error'
        send(dict(type='error', code=code, summary=(str(exc) or type(exc).__name__)[:1024]))
        return 1
    return 0


def main():
    protocol = isolate_stdout()
    for key in ('HF_HUB_OFFLINE', 'TRANSFORMERS_OFFLINE', 'HF_HUB_DISABLE_TELEMETRY', 'DO_NOT_TRACK'):
        os.environ[key] = '1'
    os.environ['TOKENIZERS_PARALLELISM'] = 'false'
    if sys.argv[1:] == ['--probe']:
        protocol.write(json.dumps(probe()) + '\n')
        return 0
    if sys.argv[1:]:
        raise ValueError('unsupported worker arguments')
    # A daemon blocked in BufferedReader.readline can hold stdin's buffer lock
    # during Python finalization and turn a normal completion into a fatal abort.
    # Use a dedicated unbuffered FD for the lifetime/control thread instead.
    control = os.fdopen(os.dup(0), 'rb', buffering=0)
    return run(protocol, control)


if __name__ == '__main__':
    try:
        sys.exit(main())
    except Exception as exc:
        print(f'worker startup failed: {type(exc).__name__}: {str(exc)[:1024]}', file=sys.stderr)
        sys.exit(70)
