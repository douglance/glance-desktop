# Contributing to Glance

Thanks for helping make screenshots easier to understand. Bug fixes, focused
features, tests, and documentation improvements are welcome.

## Get started

On macOS, you need macOS 12+, Xcode Command Line Tools, and stable Rust through
[rustup](https://rustup.rs/). The repository's `rust-toolchain.toml` selects
stable Rust and installs rustfmt and Clippy. Omarchy contributors should use the
[Linux setup and packaging guide](docs/linux.md).

```sh
git clone https://github.com/modem-dev/glance-desktop.git
cd glance-desktop
cargo run --locked
```

Use the [bundled app setup](README.md#build-and-install) when testing screen
capture or MP4 export. See [development](docs/development.md) for signing,
permissions, and the optional pre-commit hook.

## Pick a change

- For a bug, include your OS version, architecture, steps to reproduce,
  expected result, and what happened. Use a synthetic image when possible;
  remove sensitive information from screenshots and logs.
- For a larger feature or architectural change, open an issue describing the
  problem and proposed approach before investing in an implementation.
- For a small fix or documentation correction, a pull request is enough.
- For security issues, follow [SECURITY.md](SECURITY.md).

## Make a pull request

Keep each PR focused on one problem. Explain the resulting behavior and how
you checked it. Add a regression test when fixing behavior that can be exercised
in the document model or GPUI's virtual platform. UI changes should include a
screenshot or short recording made with non-sensitive sample content.

Run the standard checks:

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

Build an app bundle when changing native helpers or packaging:

```sh
./scripts/bundle.sh debug
```

GitHub Actions runs the same checks and builds the app on macOS. The regular test
suite does not upload to production or control your desktop. Run ignored tests
only when their purpose is relevant; some write media or perform live uploads.

Document user-visible changes in [CHANGELOG.md](CHANGELOG.md) under the existing
`Unreleased` subsection. Update the usage guide when shortcuts or behavior change.
Commit titles should follow Conventional Commits, such as
`fix: preserve selection after cropping` or `docs: explain local signing`.

## Project conventions

- Preserve the native Rust/GPUI architecture. Start with the
  [architecture guide](docs/architecture.md) for entry points.
- Toolbar, menu, shortcut, and MCP triggers should use the common typed action
  dispatcher. Keep expensive rendering and export off the UI thread.
- Keep annotations editable through transforms where supported, preserve undo,
  and reject outdated worker results.
- Keep capture and local editing usable offline. Remote uploads and the local
  automation bridge are explicit user actions.
- Keep third-party attribution and license files with borrowed code or assets.
- Never commit local signing keys, credentials, or real private captures.

Treat other contributors with respect. Keep discussion focused on the problem
and the change. By submitting a contribution, you agree to license your original
contribution under the project's [MIT license](LICENSE).
