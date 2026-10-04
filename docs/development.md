# Developing Glance

[Build from source](../BUILD.md) · [Contributing](../CONTRIBUTING.md) · [Architecture](architecture.md)

## Workflow

Use `cargo run --locked` for UI iteration and the [bundled app](../BUILD.md#macos)
for capture permissions and native media helpers. Save or copy your image before
quitting and reopening a rebuilt app.

The [contributor checks](../CONTRIBUTING.md#make-a-pull-request) run formatting,
Clippy, and the regular tests. [QA](../QA.md) covers desktop acceptance and opt-in
tests; [performance notes](../PERFORMANCE.md) cover benchmark methodology.

### Optional pre-commit hook

```sh
./scripts/setup-dev.sh
```

The hook runs formatting and Clippy. Disable it with
`git config --local --unset core.hooksPath`. Both commands change this
repository's Git configuration, shared by its worktrees.

## CI

[CI](../.github/workflows/ci.yml) runs formatting, Clippy, tests, and a real
MP4 encode/PNG decode round trip on macOS and Arch Linux for pull requests and
pushes to `main`. Release packaging runs separately when a GitHub release is published.

### Linux dependency cache

The [dependency warmer](../.github/workflows/linux-dependencies.yml) caches optimized
locked dependencies on `main` when Cargo/toolchain or build configuration changes.
Release jobs restore that cache alongside the regular check/test cache, then
compile and link the app. Warmers run serially so later jobs can reuse earlier work.

After a toolchain update or cache eviction, warm it manually:

```sh
gh workflow run linux-dependencies.yml --ref main
```

Keep the warmer and release job's Rust version, profile environment, and
`linux-release-deps-v1` shared key aligned. Tags can restore caches from `main`;
caches saved under one tag cannot warm another. GPUI 0.2.2's legacy xattr dependency
also requires the current libc compatibility pin; revisit it when upgrading GPUI.

## Releases

1. Complete the [desktop acceptance checklist](../QA.md) on macOS and Omarchy.
2. Move the relevant `Unreleased` changelog entries into a dated version section.
   Leave released sections immutable and start a fresh `Unreleased` section.
3. Align Cargo and bundle versions. Use the released changelog section as the
   GitHub release body with `gh release create/edit --notes-file`.
4. Publish the release when authorized, then verify its downloadable assets.

The [release workflow](../.github/workflows/release.yml) checks out the published
tag, runs checks, and builds an Apple Silicon ZIP, an Omarchy x86_64 Arch package,
and a Linux archive. It verifies the installed Arch package. After both platform
jobs succeed, the upload job attaches packages, PKGBUILD, and SHA-256 files to the
release. Download assets appear after packaging finishes.

Current macOS packages use ad-hoc signing (`GLANCE_CODESIGN_IDENTITY=-`).
For notarized distribution, configure a Developer ID Application identity,
hardened runtime/timestamp signing for all executables, `notarytool`, and a
stapled ticket. Keep signing credentials outside Git and confined to the relevant
jobs; only the asset-upload job currently has repository write permission.
