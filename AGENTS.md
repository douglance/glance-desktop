# Working on Glance

Glance is a native macOS and experimental Omarchy screenshot editor in Rust/GPUI. It edits one document,
exports PNG/GIF/MP4, shares images through glance.sh, and offers opt-in local MCP
control. Use Cargo and the existing Rust tooling; do not add a JavaScript toolchain.

## Entry points and architecture

- `src/main.rs`: startup and window creation.
- `src/editor/actions.rs` and `dispatch.rs`: common typed application actions;
  toolbar, menus, shortcuts, gestures, and MCP use this path.
- `src/document.rs` and `document/actions.rs`: image model, validated edits, undo.
- `src/editor/input.rs`, `view.rs`, `canvas.rs`, `panels/`: input and native UI.
- `src/editor/jobs.rs`: workers, operation IDs, stale-result rejection.
- `src/animation/preview.rs`, `preview/quality.rs`, `src/shaders/motion.metal`:
  worker-rendered shader previews, adaptive quality, periodic motion effects.
- `src/platform.rs`: platform capture, dialogs, fonts, and clipboard; Linux lives in `src/platform/linux.rs`.
- `src/glance.rs`: encrypted remote-sharing protocol.
- `src/mcp.rs`, `src/automation.rs`, `src/editor/automation.rs`: stdio MCP and IPC.
- `native/`: AVFoundation/FFmpeg helpers. `scripts/bundle.sh`: macOS packaging;
  `scripts/package-linux.sh`: Linux archive and Arch package recipe.

See `docs/architecture.md` for details and `docs/usage.md` for current behavior.
`PLAN.md` is historical; do not use its early scope as the current feature list.

## Working rules

- Keep rendering/export work off the UI thread. Preserve undo and editable
  annotations; guard asynchronous results with operation/revision checks.
- Dispatch typed actions from new UI or automation triggers. Do not fabricate
  keyboard events to invoke commands.
- Keep local editing offline. Do not upload captures, expose the automation
  bridge, or change certificate trust as an incidental validation step.
- Use synthetic samples for docs and tests. Preserve third-party licenses and
  provenance. Keep signing material and credentials outside Git.
- Keep PRs focused and update usage docs when behavior or shortcuts change.

## Validation

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

For native-helper or packaging changes, build with `./scripts/bundle.sh debug`.
For CI/isolated packaging checks, set `GLANCE_CODESIGN_IDENTITY=-` to avoid
certificate trust setup. Ad-hoc bundles have unstable capture permissions;
do not replace a user's installed app with one as part of validation.

Tests use GPUI's virtual platform. Check `QA.md` for manual desktop verification.
Ignored tests are opt-in and may write media, use native helpers, benchmark, or
upload to production; read the test before explicitly running it.

## Changelog and releases

Maintain top-level `CHANGELOG.md` for user-visible changes. Add entries to the
existing `Added`, `Changed`, or `Fixed` subsection of `Unreleased`; do not create
duplicate headings. Omit internal-only refactors unless behavior changes.

On release, move the relevant entries into a dated version section, leave that
released section immutable, and start a fresh `Unreleased` section. Align Cargo
and bundle versions. Use the released section as the GitHub release body and
verify any generated notes. Use `gh release create/edit --notes-file` for
multiline notes. Publishing releases or changing visibility requires a user
request; preparing the repository does not make it public.

Commit titles use Conventional Commits: `<type>[scope]: <description>`.
