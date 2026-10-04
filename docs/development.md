# Developing Glance

[Contributing](../CONTRIBUTING.md) · [Architecture](architecture.md)

## Build and run


On macOS, requires macOS 12+, Xcode Command Line Tools and a current stable Rust
toolchain. For Linux dependencies and packaging, see [Omarchy](linux.md).

```sh
cargo run --locked
```

For video export, run the bundle build below once to compile the native encoder.

Build a locally signed app bundle:

```sh
./scripts/bundle.sh
open target/Glance.app
```

To launch through Spotlight or Raycast, link the bundle into Applications:

```sh
ln -s "$(pwd)/target/Glance.app" /Applications/Glance.app
```

Rebuilding updates the linked app. Save or copy your current image before
quitting and relaunching to use a new build.

When upgrading from Pachiri, replace the old Applications shortcut with
`/Applications/Glance.app`. Glance uses the new bundle identity
`sh.glance.desktop`, so grant Screen Recording to Glance once after the rename.
The bundle build migrates existing local signing files into Glance's Signing
directory and reuses the certificate; its original common name may still show
the former app name. New certificates are named Glance Local Development.

Use the packaged app consistently so macOS can associate screen-recording
permission with `sh.glance.desktop`. On first capture, grant access in **System
Settings → Privacy & Security → Screen & System Audio Recording**, then relaunch.
Local builds use a persistent development certificate, kept in
`~/Library/Application Support/Glance/Signing`. The first build prepares it;
run `./scripts/trust-local-signing.sh` once to trust it for code signing, then
repeat the bundle build. That setup changes user certificate trust for code
signing only. Distribution signing/notarization is not configured.

### Screen Recording enabled but capture fails

Earlier ad-hoc builds gave each changed executable a different designated
requirement. macOS may display the old grant as enabled while rejecting the
rebuilt app.
Quit Glance, remove its entry from **Screen & System Audio Recording** with **−**,
add `/Applications/Glance.app` again with **+**, enable it and reopen. Re-grant
once after switching to the persistent signing identity. Subsequent builds use
the same certificate and requirement. Keep the Signing directory when cleaning
`target/` or moving the checkout; replacing the certificate changes the identity.
The app now checks permission before hiding and preserves other capture errors.

For development with a stable code-signing certificate already in your Keychain:

```sh
GLANCE_CODESIGN_IDENTITY="Your code-signing certificate name" ./scripts/bundle.sh
```

Use the same certificate for subsequent builds. Set the identity to `-` only
when explicitly testing ad-hoc signing; that mode can invalidate grants again.
See Apple's
[code-signing requirement explanation](https://developer.apple.com/documentation/technotes/tn3127-inside-code-signing-requirements).

## Contributor checks

The repository uses stable Rust with rustfmt and Clippy. Install Rust through
[rustup](https://rustup.rs/) and run:

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

For a faster development bundle, use `./scripts/bundle.sh debug`. The icon
renderer still builds in release mode. `cargo run --locked` is useful for UI
iteration; use the packaged app for consistent Screen Recording permissions and
native video helpers. Ordinary builds do not expose the MCP bridge.

Enable the optional pre-commit hook:

```sh
./scripts/setup-dev.sh
```

It runs formatting and Clippy before commits. To disable it, run
`git config --local --unset core.hooksPath`. Both commands change only this
repository's Git configuration, which is shared by its worktrees.

## CI

Every pull request and main-branch push runs formatting, Clippy, tests, and a
real MP4 encode/PNG decode round trip on macOS and Arch Linux. These checks do
not build release packages or upload downloads.

The separate [release workflow](../.github/workflows/release.yml) runs only
when a GitHub release is published, including a prerelease. It checks out that
release's tag, runs the checks, and builds an Apple Silicon macOS ZIP and an
Omarchy x86_64 Arch package/Linux archive. The Arch job installs its generated
package and checks its CLI and desktop entry. After both jobs succeed, packages,
the PKGBUILD, and SHA-256 files are attached to the existing GitHub release.
It does not create releases; publishing one is an explicit maintainer action.
Intermediate Actions artifacts are retained for 14 days; release assets remain
available on the [Releases page](https://github.com/modem-dev/glance-desktop/releases).

Linux releases restore two compiled-dependency caches: the regular CI check/test
cache and an optimized release cache. The
[dependency warmer](../.github/workflows/linux-dependencies.yml) saves the
optimized cache on `main` when Cargo/toolchain or build-workflow configuration
changes. It builds the locked dependencies with a temporary placeholder binary
and the real release profile. Packaging and download uploads stay release-only.
The app itself and its final optimization/linking still run for each release.
Warmers run one at a time; a newer update queues behind the active build so it
can restore the libraries already compiled instead of cancelling that work.

Warm the cache manually from `main` after a Rust toolchain update or cache
eviction:

```sh
gh workflow run linux-dependencies.yml --ref main
```

Release and warmer jobs must keep their Rust version, profile environment, and
`linux-release-deps-v1` shared key aligned. Rust-cache also keys compiled outputs
by architecture, Rust environment, and dependency configuration. Tags can restore
caches from `main`; caches saved under one tag cannot warm a different tag.

Release packaging currently uses ad-hoc macOS signing
(`GLANCE_CODESIGN_IDENTITY=-`) and verifies the bundle and notices. Only the
asset-upload job has repository write permission; no signing secrets are needed.
A public macOS release still needs a Developer ID Application identity,
hardened runtime/timestamp signing for all executables, notarization with
`notarytool`, and a stapled ticket. Intel/universal builds are not configured.
CI’s virtual UI tests do not replace a real desktop acceptance pass.

To verify packaging locally without replacing your normal bundle, choose a
separate output path. Use ad-hoc signing only for this isolated check:

```sh
GLANCE_BUNDLE_DEST="$(pwd)/target/packaging-check/Glance.app" \
  GLANCE_CODESIGN_IDENTITY=- ./scripts/bundle.sh debug
```

The regular tests exercise the document model, exports, local HTTP protocol
fixtures, and GPUI's virtual platform. They do not capture the desktop or upload
to production. Ignored tests are opt-in: some write media, use Metal/native
helpers, benchmark rendering, or make a live Glance upload.

See [QA](../QA.md) for the manual desktop acceptance checks and explicit test
commands, and [performance notes](../PERFORMANCE.md) for benchmark boundaries.

## Releases

The initial release is
[v0.1.0](https://github.com/modem-dev/glance-desktop/releases/tag/v0.1.0).
Before distributing a release, verify
capture permissions, clipboard, dialogs, native input, and media exports on a
real Mac. Public distribution also needs an Apple distribution-signing and
notarization process; the local development certificate is not that process.

Maintain [CHANGELOG.md](../CHANGELOG.md). Put upcoming user-visible changes under
`Unreleased`; when releasing, move them into a dated version section and use
that section as the release notes. Keep the Cargo package version and bundle
version aligned.

The repository remains private during preparation. When you are ready to open
it publicly, change its visibility in GitHub settings and enable **Private
vulnerability reporting** so the security policy's private reporting form is
available. Review the pending changes before that visibility change.
