# Image Studio 0.2.3 Release Evidence

Date: 2026-11-14
Platform: macOS arm64, debug release-verification bundle
Logical viewport: 1280x800, 1 px/pt

## Screenshots

- `settings-1280-light.jpg`: six-group settings center with persisted accent and
  row-based controls.
- `about-1280-light.jpg`: About at version `0.2.3`, with one GitHub project
  entry and GitHub as the only author link.
- `updates-1280-light.jpg`: successful check against the public
  `Goldloli/image-studio` GitHub Releases API. The current development build
  reports `已是最新` because `0.2.3` is higher than the latest public stable
  release at check time.

## Verified

- A real update check succeeds with the host's system proxy configuration.
- The settings UI reports the exact application version `0.2.3`.
- About contains GitHub, Releases, Issues, license, and third-party links without
  a Bilibili button.
- Frontend tests, TypeScript, production build, Rust formatting, Clippy,
  workspace tests, OpenSpec, workflow validation, and actionlint are required to
  pass before tagging.

## Not Run

- A real download, SHA-256 verification over release bytes, cancellation,
  opening the DMG, and macOS installation were not exercised because no newer
  stable release existed at check time.
- Windows NSIS download, installation, restart, and active-task refusal remain
  `not_run` without a Windows session.
- The macOS build is unsigned and not notarized; no Gatekeeper bypass was
  attempted.
