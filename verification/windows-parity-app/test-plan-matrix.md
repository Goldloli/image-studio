# Windows parity test-plan matrix (result of the installed-app round)

Date: 2026-11-04. Installed app: `Image Studio_0.1.0_x64-setup.exe`, isolated
data root `C:\ist-a`, 6/7 models installed from the verified cache seed, real
generations through the native UI. Machine-readable evidence:
[results.json](results.json), [lifecycle.json](lifecycle.json),
[not-run.json](not-run.json); per-model Gate 0 records under
`verification/windows-parity-gate0/`.

| Test-plan ID | Result | Evidence / note |
|---|---|---|
| G2-K1 Klein Gate 0 | pass | `verification/windows-parity-gate0/klein-gate0-fresh-process-nf4.json`; installed-app Klein t2i + edit jobs succeeded |
| G2-B1 BiRefNet Gate 0 | pass | `birefnet-gate0.json`; installed-app RGBA+mask job succeeded |
| G2-R1 Real-ESRGAN Gate 0 | pass | `verification/windows-parity-realesrgan/`; installed-app 4096² upscale job succeeded |
| G2-Q1 Qwen base Gate 0 | pass | `staged-loader-qwen21-base.md`; installed-app 40-step job succeeded |
| G2-Q2 Qwen Turbo Gate 0 | pass | `staged-loader-qwen21-turbo.md`; installed-app 8-step job succeeded |
| G2-M1 MOSS Gate 0 | pass | `staged-loader-moss-image-sft.md`; installed-app 50-step job succeeded |
| I2-1 empty-cache install per model | **blocked** | PyPI wheel throughput ~0.2–0.3 MB/s stopped the shared-runtime download; see `install-birefnet-empty-cache-network-blocked.json`. Cache-hit installs for all six models passed instead |
| I2-2 bad checksum / missing file | **partial** | corrupt-cache rejection + re-download + no activation verified (`lifecycle.json`); full missing-file rejection not completed under the same network blocker |
| I2-3 interrupt/resume | pass (cache path) | cache-hit installs resumed by the installer after the first stopped attempt; the interrupted state required no re-download of verified files |
| I2-4 network-denied install | waived_not_run | user decision 2026-11-03 (`verification/windows-network-denial-decision.json`) |
| P2-1 per-model real generation | pass | `results.json`: seven capabilities with frozen plans, seeds, sizes, devices, runtimes and output content hashes |
| P2-2 Klein single-image edit | pass | `job-dcb27d…`, source asset preserved, 1024² RGB output |
| P2-3 BiRefNet RGBA + mask | pass | RGBA `3a5089bd…` + mask `d0348d86…`, alpha==mask at 21,609 sampled pixels (0 mismatch), works count only primary |
| P2-4 Real-ESRGAN 4× | pass | 1024² → 4096², export `984b9045…`, exact 4× |
| P2-5 cancel / hard-kill / restart | pass | `lifecycle.json`: cancel, supervisor hard-kill with tree reclaim and no fabricated asset, restart recovery |
| P2-6 native UI send/view/export/tasks/works | pass | screenshots + exports in `artifacts/`; exported bytes equal the formal asset content hashes |
| P2-7 hardware gate + license display | pass | model page shows 7 entries and license notes; host gates below-threshold sends/installs (M4); unit evidence in `verification/windows-ui-projection/` |
| R2-1 full quality gates | pass | `verification/windows-parity-quality-gate.json`; final installer build ran the gates and succeeded |
| R2-2 Z-Image and history regression | not_run | Z-Image not reinstalled in this isolated root; first-round evidence stands, shared tests green |
| R2-3 docs / compatibility | pass | README Windows section + model READMEs + this acceptance README |
| R2-4 P0 summary and archive | pending | this matrix is the summary; archive waits for user sign-off on the not-run rows and quality samples |

## Acceptance caveats

- The empty-cache network path is blocked by the environment's current PyPI
  throughput, not by a product defect; it must not be counted as a pass.
- A data root deeper than the legacy MAX_PATH window fails the onnxruntime probe
  (`install-failure-long-data-root.json`); the product default root is short and
  the acceptance root `C:\ist-a` works. Long-root hardening is a follow-up.
- The four defects found during this round (native-runtime restart readiness,
  ncnn probe verbatim paths, Windows asset pixel budgets, send allowlist) are
  fixed and covered by unit tests; the failed Real-ESRGAN job is retained as
  failure evidence.
- Quality sign-off of the sample images (4.4) and the Archive step (5.4) need
  the user's confirmation.
