#!/usr/bin/env python3
"""Verify native window/renderer initialization with networking denied to the test process."""
from pathlib import Path
import argparse
import json
import select
import subprocess
import sys
import tempfile
import time

root = Path(__file__).resolve().parent.parent
parser=argparse.ArgumentParser()
parser.add_argument(
    '--report-prefix',
    choices=[
        'cherry',
        'first-generation',
        'model-settings',
        'model-usability',
        'planned-models',
        'qwen-moss',
    ],
    default='cherry',
)
prefix=parser.parse_args().report_prefix
if sys.platform != "darwin":
    raise SystemExit("This validation uses macOS sandbox-exec.")
profile = root / "verification/deny-network.sb"
network_probe = subprocess.run(
    ["/usr/bin/sandbox-exec", "-f", str(profile), "/usr/bin/curl",
     "--max-time", "3", "-I", "https://example.com"],
    capture_output=True, text=True, check=False,
)
if network_probe.returncode == 0:
    raise SystemExit("The network-denial control probe unexpectedly succeeded.")
with tempfile.TemporaryDirectory(prefix="image-studio-offline-") as temporary:
    process = subprocess.Popen(
        ["/usr/bin/sandbox-exec", "-f", str(profile),
         str(root / "target/debug/image-studio-desktop"),
         "--diagnostics", "--data-dir", str(Path(temporary) / "中文 数据根"),
         "--diagnostic-size", "960x640", "--diagnostic-scale", "2"],
        stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
    )
    captured = []
    ready = False
    try:
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline and process.poll() is None:
            readable, _, _ = select.select([process.stderr], [], [], 0.1)
            if readable:
                line = process.stderr.readline()
                captured.append(line.rstrip())
                if "DIAGNOSTIC_DESKTOP_READY" in line:
                    ready = True
                    break
        if not ready:
            raise SystemExit(f"Native initialization was not observed: {captured}")
        time.sleep(0.3)
        if process.poll() is not None:
            raise SystemExit("Desktop exited immediately after initialization.")
        result = {
            "network_control_exit": network_probe.returncode,
            "network_control_stderr": network_probe.stderr.strip(),
            "native_window_renderer_initialized": True,
            "tauri_frontend_page_finished": True,
            "runtime": "Tauri 2 + embedded React production build",
            "initialization_log": captured,
            "model_installed": False,
            "worker_started": False,
            "observation": "Tauri main WebView PageLoadEvent::Finished log and live process; native UI and embedded images are separately verified with CUA.",
        }
        destination = root / f"verification/{prefix}-offline-startup.json"
        destination.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
        print(destination)
    finally:
        if process.poll() is None:
            process.terminate()
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
