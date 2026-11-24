# Third-Party Notices

Image Studio itself is licensed under the Apache License 2.0. See
[LICENSE](LICENSE) and [NOTICE](NOTICE).

The application depends on open-source libraries whose own licenses govern
their use and redistribution. Exact Rust dependency versions are frozen in
`Cargo.lock`; exact frontend dependency versions are frozen in
`apps/frontend/package-lock.json`.

## Principal Runtime Dependencies

| Component | Copyright/license source | License |
|---|---|---|
| Tauri and Tauri runtime crates | Tauri contributors | MIT OR Apache-2.0 |
| React and React DOM | Meta Platforms, Inc. and contributors | MIT |
| Vite, Vitest, TypeScript toolchain | respective contributors | MIT or Apache-2.0 as declared by each package |
| Lucide React icons | Lucide contributors | ISC |
| Noto Sans CJK SC | Adobe / Noto contributors | SIL Open Font License 1.1 |
| Other Rust crates | crate authors | declared in Cargo metadata and crate sources |

The embedded font, its fixed upstream revision, checksum, copyright and license
record are documented in [resources/fonts/README.md](resources/fonts/README.md).
The original font license text is
[resources/fonts/OFL.txt](resources/fonts/OFL.txt).

## Prompt Library

The prompt-library catalog and example-image handling are documented in
[resources/prompt-library/README.md](resources/prompt-library/README.md).
The upstream catalog license text is
[resources/prompt-library/LICENSE](resources/prompt-library/LICENSE). Third-party
example images and prompts may carry additional restrictions; the application
preserves source attribution and does not claim those examples were generated
locally.

## Models and Runtimes

Model weights, Python runtimes, native inference libraries and model-specific
code are installed separately by the user and are not bundled in the application
installer or macOS archive. Each model and runtime remains subject to its own
upstream license and acceptable-use terms. License identifiers and source
records are available in `resources/model-catalog/catalog.json`,
`resources/planned-models/`, and `resources/windows-parity/`.

Qwen model materials are marked as non-commercial in the application catalog;
users must review the upstream Qwen license before use or redistribution.

MiniMax-H3-Image materials are subject to the MiniMax-H3-Image Community
License Agreement, including its incorporated Acceptable Use Policy, territory,
revenue, hosted-service, and distribution terms. The fixed license text and
NOTICE are archived under `resources/minimax-h3-image/macos-v1/` and are copied
into the application-managed license directory after explicit user
acknowledgement. The dedicated runtime is built from `leejet/stable-diffusion.cpp`
under its MIT license; the fixed runtime archive also carries the upstream
stable-diffusion.cpp, ggml, WebM, WebP, and CUTLASS notices.

The MiniMax-H3-Image model files are installed separately and are not bundled in
the application installer or macOS archive. See
`resources/minimax-h3-image/`, `resources/model-catalog/catalog.json`, and
`verification/minimax-h3-image-20261124/` for pinned sources, hashes, and
acceptance evidence.

## Reporting an Attribution Issue

Please open an issue at <https://github.com/Goldloli/image-studio/issues> with
the component name, file path and expected license notice.
