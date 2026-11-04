# Cross-platform model-selection warning acceptance

Status: **fixed and verified on the installed Windows app; macOS rows user-gated**
(2026-11-04). Change: `fix-cross-platform-model-selection`; commit `959d95c`.

## What was wrong

`legacyWindowsTextModel` compared the platform-resolved `Z_IMAGE_MODEL_ID` with
the Windows constant, which is tautologically true on Windows. The warning was
correct when Windows only had Z-Image, but after the parity change every native
Windows text model (Klein / Qwen / MOSS) was falsely flagged as a legacy
cross-platform selection. macOS had the mirror gap: Windows-saved sessions only
showed "未就绪" with no cross-platform explanation.

## Verified on the installed app (Windows)

| Case | Result | Evidence |
|---|---|---|
| S1 Klein / Qwen Turbo / MOSS | no cross-platform warning, normal labels | [screenshot](windows-native-models-no-warning.png), results.json |
| S3 legacy macOS Z-Image id in draft | new Windows-family warning + `当前平台未接入` label + send disabled | [screenshot](legacy-macos-selection-warning.png), results.json |
| Unit matrix | 7 native ids/side un-warned, 7 foreign ids/side warned, tool/unknown ids silent; 57 frontend tests | results.json |

The S3 scenario was produced by temporarily rewriting the isolated
`C:\ist-a` draft params to the macOS Z-Image id, relaunching, capturing the
warning, then restoring the backed-up library. The old warning text is gone.

## Not run (honest rows)

- **S2/S4 same-platform not-ready label**: Windows Z-Image is not installed in
  the isolated root and its menu option is disabled, so this label could not be
  exercised live; the pure-function matrix covers the branch.
- **M1/M2 macOS**: macOS app cannot run on this machine. The shared frontend
  logic is unit-tested for both platforms; the user should confirm the macOS
  session hint on the Mac. These rows are not counted as passes.

## Quality gates

Final installer build (same commit) passed cargo fmt/clippy/workspace tests and
the frontend gates (`target/parity-gates/build-cross-platform-warning-2.log`);
`openspec validate --all --strict` passed. A flaky inference-supervisor flood
test failed once under load (bd `image-studio-...`) and passed on rerun; it is
tracked separately.
