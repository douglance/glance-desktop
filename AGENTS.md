# Working on Glance

Glance is a native macOS screenshot editor in Rust/GPUI. It edits one document,
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
- `src/platform.rs`: macOS capture, dialogs, and clipboard.
- `src/glance.rs`: encrypted remote-sharing protocol.
- `src/mcp.rs`, `src/automation.rs`, `src/editor/automation.rs`: stdio MCP and IPC.
- `native/`: AVFoundation helpers. `scripts/bundle.sh`: app packaging and signing.

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

## MCP parity

- Every user-facing command or editable setting needs a semantic MCP path through
  the shared application/document actions, plus state read-back where relevant.
  Implement UI and MCP support in the same change.
- Update `src/mcp.rs` discovery/schema and `mcp/README.md` with action, parameter,
  annotation, or export changes. A serializable Rust action alone is not enough:
  the published MCP schema must accept and describe it.
- Keep `src/mcp/contract_tests.rs` passing. It compares Serde's action inventory
  with the published schema and validates a complete round-trip payload for every
  exposed action. Add parameterized fixtures for new actions and behavior tests
  through the bridge/shared dispatcher for changed semantics, including state,
  undo and revision conflicts where applicable.
- Internal prepared results and pointer gestures may stay unexposed; record a
  reason and semantic alternative in the contract test's explicit exclusions.
  Prefer revision-scoped document tools over raw annotation indices. Never expose
  worker completion actions or require fabricated input events for parity.

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
