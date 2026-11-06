# Windows coverage — measured results (2026-11-06)

Machine: Windows 11 Enterprise 22631 / i9-9900K / RTX 2060 SUPER 8 GB (sm75) / 31.9 GiB RAM.
Data root `D:\ist-8gb\appdata`, app built from `feat/support-8gb-2060s-vram`.

## Capabilities exercised in the product

| ID | Capability | Result | Measured |
|---|---|---|---|
| C1 | Qwen-Image 2.1 Turbo text-to-image | ✅ | 20 consecutive sends, 101–116 s each |
| C2 | FLUX.2 Klein 4B text-to-image | ✅ | 177 s (includes 155 s component load) |
| C3 | FLUX.2 Klein 4B single-image edit | ✅ | **36 s** for the product run (warm load) |
| C4 | MOSS-Image-SFT text-to-image | ✅ | 50 steps, **~18.5 min** end to end (another app held several CPU cores; the isolated Gate 0 measurement was 752 s) |
| C5 | BiRefNet background removal | ✅ | **12 s**, produced RGBA + mask (`查看蒙版` shown) |
| C6 | Real-ESRGAN 4× upscale | ✅ | **4 s**, 220×220 → **880×880** |
| C7 | Qwen-Image 2.1 base (40 steps) | ✅ | **~14 min** end to end under the same CPU-contended environment (Gate 0 isolated: 391 s) |
| C8 | Z-Image Turbo on 8 GB | not supported | four measured failures in `doc/16` |

## How the tool-driven runs were issued

The Tauri IPC accepts `draft_save` then `conversation_send`, but the send only succeeds when the
saved draft's prompt and params match the request exactly; otherwise the library returns
`会话或发送轮次状态已变化`. The harness therefore saves the draft first and sends with the
returned version. Source images are imported through the native dialog, which a Win32 helper fills
(`Edit` control 1148 + `WM_COMMAND IDOK`).

## Install-time defects found and fixed during this pass

**Real-ESRGAN could never install.** The downloader verified the `.part` file, found a digest
mismatch, left the bad part in the cache and treated the failure as non-retryable, so every later
attempt resumed on top of the corrupt bytes and failed again — permanently. The same run also
showed the retry counter staying at "1 次" because digest failures skipped the retry loop.

Fixes:

- a failed part is deleted together with its `.resume.json` before the error returns;
- a digest failure now gets one fresh attempt (the part is gone, so it re-downloads);
- the final error keeps the digest detail even when later attempts fail for network reasons;
- the message no longer says "请继续以修复".

After the fix the same install completed: `downloading → verifying → probing → ready`, and the
model row showed the live phase throughout.

## Model selector

Clicking the composer model chip opens a popover listing every installed model with its status
(`模型已就绪 · 紧凑档 768×768 · float16`), Z-Image disabled with its VRAM reason, and a check mark
on the active one. The earlier "no menu" observation was a test-timing artefact, not a defect.

## Observations worth a follow-up

- The app exited once during a Qwen base generation (turn recovered as `interrupted` on restart,
  no Windows Error Reporting entry). Memory was around 14 GiB available; the cause is unknown.
- A nonsense Chinese prompt (`Qwen 基础版出图验证二`) produced a photorealistic lemur from the base
  model, which is plausible model behaviour but was re-checked with a concrete prompt.
## Export

`导出 PNG` opened the native save dialog; the helper filled the name box and confirmed. The
written file (1,085,419 bytes) matches the stored asset byte-for-byte:
`2839f307941409db546b57623fe53c412f6819bf8acd2f12515ab2ab840c5d32`.

## Prompt fidelity spot check

A concrete prompt (`一只橘色的猫坐在木桌上，写实摄影，自然光`) produced an orange cat on a wooden
table, so the base model does follow instructions; the earlier lemur came from a meaningless test
prompt, not from a prompt/asset mix-up.
## Fault injection without administrator rights

Two of the four "impossible" checks turned out to be testable once the app was launched with the
right environment:

**Network fault.** The installer's HTTP client honours `HTTP(S)_PROXY`, so starting the app with
`HTTPS_PROXY=http://127.0.0.1:9` makes every download fail at the socket. Installing Real-ESRGAN in
that state produced `下载失败（已尝试 5 次）：网络中断：…` — the retry budget ran and the error names
the file. Relaunching without the proxy and installing again succeeded, so the failure is clean and
recoverable. No firewall rule or hosts edit was needed.

**Non-ASCII and long path.** A fresh data root at `D:\ist-8gb\数据 测试\appdata` (Chinese, spaces)
exposed a real defect: the ncnn probe log is written in the OEM code page, so `read_to_string`
failed, the GPU identity line was dropped, and *every* install in that root failed with
`ncnn Vulkan 探针未报告 GPU 0`. Decoding the log lossily fixed it; the same root then completed a
full install, a source import through the native dialog, and a 4× upscale (220→880).

**Disk space.** The live "disk full" case still needs a small volume, but the decision itself
(`installation_space_available`) now has a unit test covering the same-volume double-count and the
per-volume split; the macOS acceptance run already recorded a full install-space sample.

**Forced reaping.** The cancel deadline predicate (`cancel_deadline_passed`) is extracted and unit
tested (missing timestamp, fresh timestamp, exited child, overdue child). A full-process version
still needs a Windows supervisor stub, because the existing generation fixture is Unix-only.

How to close it: add a tiny `[[bin]] stub-supervisor` that ignores stdin and sleeps for a minute,
point `GenerationController::new` at `env!("CARGO_BIN_EXE_stub-supervisor")` inside a Windows-only
integration test, start the fixture job, cancel it, and assert the controller leaves `busy` within
`cancel_grace_ms + cleanup deadline + slack` (~15 s) and that the child process is gone. The app
already spawns the supervisor suspended inside a job object, so the kill path is the same one
production uses.