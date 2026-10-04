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

- Capture, annotation, clipboard copy, and file export run locally.
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
