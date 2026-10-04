# Security policy

Glance is in early development. Security fixes target the current `main` branch;
older development builds do not have a separate support policy.

## Report a vulnerability

Use GitHub's private **Report a vulnerability** form on the repository's
[Security page](https://github.com/modem-dev/glance-desktop/security/advisories)
when available. If private reporting is unavailable, open an issue asking for a
private reporting channel **without disclosing the vulnerability or sensitive
data**. A maintainer will arrange a channel before you share details.

Include the affected commit/build, macOS version, reproduction steps, expected
impact, and a minimal proof of concept using synthetic images. Do not attach
live share URLs, personal captures, credentials, or local signing material.

## Data and access boundaries

- Capture, annotation, clipboard copy, and file export run locally. Capture and
  media scratch files use private temporary directories; media helpers do not
  fetch remote URLs.
- **Copy (remote)** explicitly uploads an encrypted PNG using the Glance service
  and Vercel Blob storage. The returned link contains the secret needed to
  retrieve the image; anyone with the link can fetch it before expiry. Link
  expiration is enforced by the hosted service, not an offline desktop guarantee.
- Pixelation is a visual effect. Crop out sensitive material before sharing.
- The automation bridge is enabled only by `--automation`. Its Unix socket is
  restricted to the local account, but is accessible to processes running as
  that account. A trusted MCP client can read images, edit the document, import
  local files, and export files. See [MCP behavior and limits](mcp/README.md#behavior-and-limits).
- Local signing certificates and private keys live outside the checkout. The
  one-time trust script changes user-domain trust for code signing. It is not
  distribution signing or notarization.

Use a local test service or synthetic fixtures when researching upload behavior.
Do not test against other people's images or links.

## Dependency review baseline

The 2026-10-03 review checked 749 registry package versions from `Cargo.lock`
against the OSV database, including RustSec and GitHub advisories. This is a
source review of `main` at `ef61f5c` plus the file-safety fixes, not a guarantee
that an application or every dependency is free of vulnerabilities.

Three upstream bug advisories match locked transitive versions. The two `git2`
entries are test-only: `cargo tree --edges normal -i git2` has no results.

| Dependency | Advisory | Reachability assessment |
| --- | --- | --- |
| `grid` 0.18.0 via Taffy/GPUI | [GHSA-38c5-483c-4qqp](https://github.com/advisories/GHSA-38c5-483c-4qqp) | The reported overflow is in `Grid::expand_rows`. Taffy 0.9.0's occupancy matrix reconstructs its grid with `Grid::from_vec`; neither Glance nor Taffy calls the affected method. |
| `git2` 0.20.4 via GPUI utilities | [RUSTSEC-2026-0183](https://rustsec.org/advisories/RUSTSEC-2026-0183.html) | The affected `Remote::list` API is not called by Glance or GPUI's utility code. Its Git use is repository initialization in test-only support; it is absent from the normal dependency graph. |
| `git2` 0.20.4 via GPUI utilities | [RUSTSEC-2026-0184](https://rustsec.org/advisories/RUSTSEC-2026-0184.html) | Glance and GPUI utilities do not use buffer-created blame hunks or their signatures. |

These assessments reduce the demonstrated exposure; they do not remove the
advisories. The patched releases (`grid` 1.0.1 and `git2` 0.21.0) are outside the
upstream dependency version requirements. Revisit when GPUI/Taffy are upgraded
or any Git/grid integrations change, and rerun the dependency scan.

Maintenance advisories also match `async-std`, `instant`, `paste`,
`proc-macro-error2`, `rustls-pemfile`, `rustybuzz`, and `ttf-parser`. These flag
unmaintained dependencies, not demonstrated machine compromise. Track their
upstream replacements rather than silently suppressing the warnings.
