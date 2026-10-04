# Editor architecture


`main.rs` only launches the app and opens its window. The GPUI editor lives in
`src/editor/`: `mod.rs` constructs the entity, `state.rs` groups its state,
`actions.rs` defines typed, serializable application intent and `dispatch.rs`
is the common entry point. Toolbar buttons, native menus, shortcuts, gestures,
global hotkeys, file drops, and MCP adapters dispatch those values. `commands.rs`
implements edits and external work; `input.rs` interprets pointer/keyboard input.
`view.rs`, `canvas.rs`, and `panels/`
build and paint the interface. `text_input.rs` implements native text input;
`text.rs` owns the Unicode buffer and text history independently of the editor.

One `Gesture` enum represents the active pointer interaction. `jobs.rs` owns
worker dispatch and completion: each external operation has an ID, and stale
results or video progress cannot affect a newer operation. Preview rendering
has its own revision checks and remains independent of external operations.
`feedback.rs` owns transient copy confirmations. `automation.rs` applies local
MCP requests on the UI thread, and `lens.rs` schedules magnifier previews.

`document/actions.rs` applies validated, undoable document edits for both the
editor and MCP snapshot workers. Heavy transforms remain on workers; prepared
MCP documents return through the dispatcher with an expected revision. Framing
changes and slider ticks also advance that revision, so stale work cannot
overwrite them. Native caret/IME handling and temporary pointer gestures stay
in input adapters; completed edits become document actions.

New triggers call `editor.dispatch(Action::SelectTool { tool: Tool::Arrow }, cx)`
instead of invoking input handlers or fabricating key events. The MCP
`dispatch_action` tool submits the same actions; `get_editor_state` reports
live state and background progress. See [action examples](../mcp/README.md#editor-actions).

The document, geometry, compositors, macOS integration, Glance protocol, and
video encoder remain separate modules. Interaction tests use GPUI's virtual
platform in `src/editor/tests.rs`, without controlling the user's desktop.

## Rendering and native integration

The app is Rust with GPUI; Metal shaders compile at runtime. Screenshot pixels
are immutable and shared across undo states. Geometry and annotations use
physical image pixels, including Retina captures. Live gestures paint GPU
overlays; compositing and export run on workers. Inside padding repeats the
nearest screenshot edge pixels before rounding and shadow. Worker-built editor
textures carry their padding amount with the revision, so stale textures cannot
change foreground geometry. Annotation coordinates remain in the original
capture.

`src/animation/preview.rs` renders shader previews off the UI thread and adapts
quality to measured render cost. `src/shaders/motion.metal` and the CPU
implementations share the same periodic effects.

`src/platform.rs` owns macOS capture, clipboard, and file dialogs.
`native/video_encoder.swift` and `native/video_frame.swift` are small AVFoundation
helpers for encoding MP4 and reading video frames. They do not own the UI.

`src/glance.rs` implements opt-in remote sharing. `src/mcp.rs` provides the stdio
MCP server, while `src/automation.rs` bridges it to an editor launched with
`--automation` over a local Unix socket.

[Performance notes](../PERFORMANCE.md) · [Test coverage](../QA.md) ·
[Original development history](../PLAN.md)
