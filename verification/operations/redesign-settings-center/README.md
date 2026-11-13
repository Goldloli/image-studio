# Redesign Settings Center Evidence

Date: 2026-11-14
Platform: macOS arm64, debug application bundle, diagnostic viewport 1 px/pt
Data roots: isolated `/tmp` directories passed through `--data-dir`

## UI Matrix

The six settings groups were captured in both themes at the two required logical window sizes:

- `960-light-01-appearance.jpg` through `960-light-06-about.jpg`
- `960-dark-01-appearance.jpg` through `960-dark-06-about.jpg`
- `1280-light-01-appearance.jpg` through `1280-light-06-about.jpg`
- `1280-dark-01-appearance.jpg` through `1280-dark-06-about.jpg`

The native screenshots show the two-panel preference layout, the accent selector, row-style
controls, space summary and path rows, runtime combinations, the update state controls, and the
GitHub/project/author sections on About.

`1280-light-update-current.jpg` records a successful real check against the public
`Goldloli/image-studio` GitHub Releases API. The newest stable release is `0.2.2`, equal to the
current application version, so the UI correctly reports `已是最新`.

## Verified

- All six groups and both themes render at 1280x800 and 960x640 without clipped primary controls.
- The content panel scrolls independently while the category panel and title header remain fixed.
- Accent presets exclude green; custom colors normalize only valid `#RRGGBB` values.
- Preferences schema v2 migrates v1 fields, persists accent/update settings, and rolls invalid
  accent writes back without replacing the file.
- Fixed stable-release selection ignores draft/prerelease entries and validates platform asset and
  checksum names.
- The public GitHub API check succeeds on macOS and returns `已是最新`.
- About opens only fixed allowlisted project links through the host command; no arbitrary URL is
  accepted.
- Update action helpers never expose install/open actions for an error, downloading, verifying,
  stale-version, or unverified package.

## Not Run

- No newer stable Release exists, so a real package download, SHA-256 verification, cancellation,
  and DMG installation were not exercised against release bytes.
- Windows NSIS download, installation, restart, and active-task refusal were not run because there
  is no current Windows session.
- The macOS package remains unsigned and unnotarized; no Gatekeeper bypass was attempted.

Unit and component coverage for these boundaries lives in:

- `apps/desktop/src/adapters/app_update.rs`
- `apps/desktop/src/adapters/preferences.rs`
- `apps/desktop/src/studio.rs`
- `apps/frontend/src/app-update.test.ts`
- `apps/frontend/src/settings-appearance.test.ts`
- `apps/frontend/src/settings-center.test.ts`
