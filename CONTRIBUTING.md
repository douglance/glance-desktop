# Contributing to Glance

## Get started

Follow [BUILD.md](BUILD.md) for macOS or Omarchy setup. Use
[the architecture guide](docs/architecture.md) to find the relevant modules.

## Report a problem or propose a change

- For bugs, include OS version, architecture, reproduction steps, expected result,
  and what happened. Use synthetic screenshots and remove sensitive data from logs.
- For larger changes, open an issue describing the problem and approach first.
  Small fixes can go straight to a pull request.
- Report vulnerabilities through [SECURITY.md](SECURITY.md).

## Make a pull request

Keep each PR focused. Explain the resulting behavior and how you checked it.
Add a regression test for behavior in the document model or GPUI's virtual platform.
Include a synthetic screenshot or recording for UI changes.

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

For native-helper or packaging changes, also run `./scripts/bundle.sh debug`.
Read opt-in tests before running them; some write media or upload to production.
Use [QA.md](QA.md) for desktop acceptance and [development](docs/development.md)
for hooks, CI, and release procedures.

Update usage docs for behavior or shortcut changes and add user-visible changes
to the existing `Unreleased` subsection in [CHANGELOG.md](CHANGELOG.md).
Use Conventional Commits, such as `fix: preserve selection after cropping`.

## Project conventions

- Use Rust, Cargo, and the native GPUI architecture.
- Dispatch toolbar, menu, shortcut, and MCP commands through shared typed actions.
  Ship new commands/settings with MCP schema, read-back, docs, and contract coverage.
- Keep rendering and exports off the UI thread. Preserve editable annotations and
  undo, and reject stale worker results.
- Keep local editing offline; sharing and automation are explicit user actions.
- Preserve third-party licenses. Keep signing keys, credentials, and private captures outside Git.

See [AGENTS.md](AGENTS.md) for file/process safety and MCP parity requirements.
Keep discussion respectful and focused on the change. Contributions use the
project's [MIT license](LICENSE).
