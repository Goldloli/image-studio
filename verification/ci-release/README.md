# Cross-platform CI/release evidence

Status: **implemented and locally validated; user-gated rows remain** (2026-11-04).

Change: `openspec/changes/introduce-ci-release-pipeline`. Workflow files:
`.github/workflows/{quality,build,release}.yml`; docs: [doc/14-ci-release.md](../../doc/14-ci-release.md).

## Locally verified (this machine)

| Item | Result | Evidence |
|---|---|---|
| Workflow YAML syntax/structure | PASS | PyYAML parse + jobs/steps checks (`target/parity-gates/validate-workflows.py`) |
| rustfmt / clippy / workspace tests | PASS | [local-validation.json](local-validation.json) |
| Frontend test / check / build | PASS | 52 tests, tsc, vite build |
| `openspec validate --all --strict` | PASS | 24/24 |
| Windows CI build command (`scripts/build-windows-internal.ps1 -SkipQualityGates`) | PASS | installer `fb37d774…` (245,021,486 B), [internal-package.json](internal-package.json), [checksums](windows-SHA256SUMS.sample) |
| macOS packaging helper (`--profile release`) | static PASS | `py_compile`, `--help`; real build requires macOS |

## User-gated (needs owner action)

1. `git push origin main` — local commits are ready but push needs explicit authorization.
2. Enable GitHub Actions and run `quality` once (Windows + macOS).
3. Manually run `build-artifacts`, download both artifacts, verify `SHA256SUMS-*.txt`.
4. Push a `v*` tag → `release-draft` creates a **draft** Release; review and publish manually.
5. Record the first run URLs and downloaded checksums here.

Until these are done the change stays un-archived and the rows must not be counted as
passes. Unsigned-artifact behaviour is documented in
[doc/14-ci-release.md](../../doc/14-ci-release.md).
