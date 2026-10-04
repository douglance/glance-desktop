# Developing Glance

[Contributing](../CONTRIBUTING.md) · [Architecture](architecture.md)

## Build and run


Requires macOS 12+, Xcode Command Line Tools and a current stable Rust toolchain.

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

GitHub Actions runs formatting, Clippy, tests, and an app bundle build on macOS.
CI uses ad-hoc signing (`GLANCE_CODESIGN_IDENTITY=-`), verifies the bundle and
bundled notices, and does not change certificate trust or require signing secrets.
This verifies packaging; it does not produce a notarized release.

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

There are no published app releases yet. Before distributing a release, verify
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
