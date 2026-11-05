# Security Policy

Image Studio runs image models locally and does not operate a hosted image
service. Security reports should still cover the desktop host, IPC boundary,
model installation and archive extraction, worker supervision, update/release
workflows, and bundled dependencies.

## Supported Versions

Security fixes target the latest published `0.1.x` release and `main`. Older
pre-release builds may be investigated but are not automatically patched.

## Reporting a Vulnerability

Do not open a public issue for an exploitable vulnerability or include API keys,
model credentials, private paths, or user images in a report.

Use the repository’s **Security → Report a vulnerability** private advisory flow:

<https://github.com/Goldloli/image-studio/security/advisories/new>

Include the affected version or commit, platform, reproduction steps, expected
impact, and the smallest sanitized proof of concept. If private reporting is
unavailable, open a non-sensitive issue asking the maintainer to establish a
private channel without disclosing the exploit.

## Scope

In scope:

- Tauri IPC and capability bypasses;
- path traversal, symlink/reparse-point escapes, archive extraction flaws;
- arbitrary command or worker execution;
- model download integrity or activation bypasses;
- secret exposure and release artifact tampering;
- denial of service that survives a normal stop/restart;
- dependency vulnerabilities with a reachable product impact.

Out of scope without product-specific impact:

- model-generated content quality or prompt safety;
- unsigned-build Gatekeeper/SmartScreen warnings already documented;
- denial of service requiring direct access to a trusted local administrator;
- vulnerabilities only in optional services that Image Studio does not use.

Please allow time for triage and a coordinated fix before public disclosure.
