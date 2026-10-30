"""Deliberately faulty process fixture. Never imports model packages or generates an image."""
import hashlib
import json
import os
import signal
import subprocess
import sys
import threading
import time

mode, pid_file = sys.argv[1:]
hello = json.loads(sys.stdin.buffer.readline())
identity = dict(hello["message"]["expected_worker"])
if mode == "bad-identity":
    identity["runtime_lock_id"] = "wrong"
seq = 0


def send(message, request=None):
    global seq
    source = request or hello
    envelope = dict(family="image-studio.generation", version=1,
                    session_id=source["session_id"], request_id=source["request_id"],
                    sequence=seq, execution=source["execution"], message=message)
    os.write(1, (json.dumps(envelope, separators=(",", ":")) + "\n").encode())
    seq += 1


send(dict(type="hello_accepted", versions=dict(min=1, max=1), selected_version=1,
          identity=identity, capabilities=["image.text-to-image.z-image-turbo.v1"]))
execute = json.loads(sys.stdin.buffer.readline())
request = execute["message"]["request"]
descendant = None
if mode in ("descendant", "orphan", "lease", "hold-gil"):
    lease = int(os.environ["STUDIO_EXECUTION_LEASE_FD"])
    descendant = subprocess.Popen([sys.executable, "-c", "import time;time.sleep(60)"],
                                  pass_fds=(lease,), stdin=subprocess.DEVNULL,
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
with open(pid_file, "w") as file:
    json.dump(dict(worker=os.getpid(), descendant=descendant.pid if descendant else None), file)
if mode == "malformed":
    os.write(1, b"{malformed}\n")
    time.sleep(60)
if mode == "oversize":
    os.write(1, b"x" * (64 * 1024 + 1))
    time.sleep(60)
if mode == "crash":
    sys.exit(7)
if mode == "truncated":
    os.write(1, b"{truncated")
    sys.exit(0)
if mode == "flood":
    for _ in range(1152):
        os.write(2, b"x" * 8192)
    for _ in range(4000):
        send(dict(type="progress", stage="sampling", completed=9, total=9), execute)
else:
    stage = "encoding" if mode == "generation-timeout" else "loading"
    send(dict(type="progress", stage=stage, completed=None, total=None), execute)
if mode == "hold-gil":
    import ctypes
    ctypes.PyDLL(None).sleep(60)
elif mode in ("hang", "descendant", "lease", "generation-timeout"):
    time.sleep(60)
elif mode == "cooperate":
    cancel = json.loads(sys.stdin.buffer.readline())
    assert cancel["message"]["type"] == "cancel"
    send(dict(type="cancelled"), execute)
else:
    payload = b"fixture bytes are not a PNG or real inference evidence"
    with open(os.path.join(request["output_dir"], "result.png"), "wb") as file:
        file.write(payload)
    send(dict(type="output_candidate", candidate=dict(relative_path="result.png",
              width=request["width"], height=request["height"], size_bytes=len(payload),
              sha256=hashlib.sha256(payload).hexdigest())), execute)
