# v0.2.2 macOS DMG acceptance

Status: **local DMG acceptance passed; GitHub publication pending.**

Version `0.2.2` replaces the macOS release zip with one arm64 DMG containing
`Image Studio.app` and an Applications shortcut. Historical releases remain
unchanged; this acceptance applies only to the new version.

## Local package

| Field | Value |
|---|---|
| Artifact | `Image-Studio-0.2.2-macos-arm64.dmg` |
| Bytes | 34,661,923 |
| SHA-256 | `56d2f161a4d92c5ac3eae13402cdabc70ac3692fa706d015c5a80526ff0d78f4` |
| Identifier | `io.github.goldloli.image-studio` |
| Version | `0.2.2` |
| Minimum macOS | `11.0` |
| Volume contents | `Image Studio.app`, `Applications -> /Applications` |
| Unexpected zip entries | none |

The four executables `image-studio-desktop`, `diagnostic-worker`,
`inference-supervisor`, and `realesrgan-worker` are arm64. LICENSE, NOTICE,
third-party notices, font license, and prompt-library license are present.

## Checks

| Case | Result |
|---|---|
| `hdiutil create -format UDZO` | pass |
| `hdiutil verify` | pass |
| Read-only mount and detach | pass |
| Bundle identity, version and minimum macOS | pass |
| arm64 main executable and three sidecars | pass |
| Applications shortcut and no zip entry | pass |
| Missing input and wrong output extension fail closed | pass |
| Truncated DMG fails `hdiutil verify` | pass |
| Release fixture rejects zip, missing DMG and bad checksum | pass |
| Copy app from DMG and launch with isolated data root | pass |

The smoke launch remained running for eight seconds and created SQLite, host and
execution locks, and `logs/app.log`. The visible Gatekeeper first-launch dialog
was not exercised and remains `not_run`.

Machine-readable record: [local-dmg-validation.json](local-dmg-validation.json).

## Remaining acceptance

- Run main quality on the release commit.
- Build the DMG through GitHub Actions.
- Verify the downloaded release DMG and publish a draft Release.
- Recheck the public asset, checksum, mount contents, launch smoke, latest
  status, and absence of macOS zip attachments.
