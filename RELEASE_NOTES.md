# Image Studio {{TAG}}

Image Studio is a local-first image creation desktop app built with Rust,
Tauri and React. This draft contains unsigned test builds for Windows and
Apple Silicon macOS.

## Downloads

- Windows x64: NSIS current-user installer.
- macOS arm64: zipped `Image Studio.app`.
- Verify downloads with the matching `SHA256SUMS-*.txt` file.

## Installation Notes

- The Windows build is unsigned. SmartScreen may show “Windows protected your
  PC”; choose “More info” and “Run anyway” only after verifying the checksum.
- The macOS build is unsigned and not notarized. Gatekeeper may block the first
  launch. After verifying the checksum, Control-click the app and choose Open,
  or run `xattr -dr com.apple.quarantine "Image Studio.app"`.
- macOS artifacts require Apple Silicon and macOS 11 or newer.
- Model weights and private Python runtimes are downloaded separately and are
  not included in these archives.

## Licensing

Image Studio is licensed under Apache-2.0. See `LICENSE`, `NOTICE`, and
`THIRD_PARTY_NOTICES.md`. Model and runtime materials remain subject to their
upstream licenses.

## Validation Scope

CI verifies formatting, Clippy, workspace tests, frontend tests/build,
OpenSpec, package architecture, artifact layout and SHA-256 checksums. CI does
not download model weights or run GPU inference; model acceptance evidence is
recorded in the repository’s `verification/` directories.
