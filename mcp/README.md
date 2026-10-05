# Glance local MCP companion

Image entrances are available through `dispatch_action`: `toggle_animation_panel`,
`select_entrance` (`effect`: `none`, `diagonal`, `pop`, or `tilt`),
`set_image_animation` (`animation`: `effect`, `duration_ms`, `delay_ms`, `seconds`,
`exit`), `set_animation_control`, `seek_animation` (`seconds`), and
`replay_animation`. `get_document` includes `image_animation`; `get_editor_state`
includes the Animation panel, playback time and `playback.preparing`.
The native canvas shows “Preparing preview…” during initial rendering and seeking;
playback time holds until a matching frame is ready. MP4/GIF export accepts an image
entrance over a still or absent backdrop and always begins at time zero.
The native Animation panel focuses on the foreground track and playback;
backdrop settings live in Backdrop and exports in the toolbar Export menu.
These commands remain available through the same shared MCP actions.
Image-entrance sampling uses the same cached Metal path for preview, PNG read-back,
and MP4/GIF on macOS, with a CPU fallback. Effect names, timing, undo and revision
conflicts keep their existing semantics.

ChatGPT or another MCP client can edit the **real native GPUI window** using structured tools. GPUI stays native; there is no web canvas or screenshot-click automation. The stdio companion connects to the opted-in editor over a private Unix socket.

## Build and start

```sh
./scripts/bundle.sh release
./target/Glance.app/Contents/MacOS/Glance --automation
```

On Omarchy, install the Arch package and run `glance --automation`. Configure
MCP with `command: "/usr/bin/glance"` and `args: ["--mcp"]`. The socket lives in
`$XDG_CACHE_HOME/glance/automation` (or `~/.cache/glance/automation`).

Start this build as your editor. If another automation-enabled Glance is already running, stop that instance first. Ordinary launches without `--automation` do not expose the bridge. Keep the editor running while using MCP.

Configure a local stdio MCP client with an **absolute path** to this checkout's `scripts/mcp.sh`:

```json
{
  "mcpServers": {
    "glance": {
      "command": "/absolute/path/to/glance/scripts/mcp.sh",
      "args": []
    }
  }
}
```

For an installed bundle you can instead use `/Applications/Glance.app/Contents/MacOS/Glance` as the command with `args: ["--mcp"]`, provided that bundle contains this build. Launch that same bundle's executable with `--automation` for the native editor.

## Connect ChatGPT

Use [Secure MCP Tunnel](https://developers.openai.com/api/docs/guides/secure-mcp-tunnels) with its local stdio profile, setting the MCP command to the absolute `scripts/mcp.sh` path. Follow the official tunnel setup/login instructions, then keep `tunnel-client run` running. In ChatGPT, enable developer mode in Settings → Security and login, add a Plugin connection using **Tunnel**, and select that tunnel. Availability depends on account and workspace policy. See [Connect and test](https://developers.openai.com/plugins/deploy/connect-chatgpt).

This repo supplies the MCP server and native bridge. It does not create an OpenAI tunnel, store account credentials, or automatically install a ChatGPT connection. No OpenAI API call or API key is needed by Glance itself. A stdio process alone cannot be reached from ChatGPT's cloud without a supported transport such as the tunnel.

## Tools

- `open_editor`: bring the connected native app forward.
- `dispatch_action`: submit the same typed action as toolbar buttons and shortcuts.
- `get_editor_state`: read tool, selection, zoom, panels, status, and operation progress, including while workers are busy. Animation export progress is the same percentage shown in the window's persistent progress bar; `cancel_export` through `dispatch_action` uses the same cancellation path as its button.
- `get_document`: dimensions, revision, backdrop, image animation, editable marks and current IDs.
- `import_image`: exactly one local `path`, image `base64`, or `clipboard: true`. Replaces the current document.
- `add_annotation`, `update_annotation`, `move_annotation`, `delete_annotation`: editable pen, arrow (including quadratic curve), box, text, highlight, pixelate, counter objects.
- `crop_image`, `resize_image`, `set_backdrop`, `undo`, `redo`: native document edits.
- `read_image`: rasterize the current canvas into model-visible PNG content, optionally at animation `phase` 0..1. Preview maximum edge defaults to 1600; never upscales.
- `export_png`: full resolution image to a new local file.
- `export_mp4`: backdrop motion and/or image entrance, H.264/30fps, 2–15 seconds, max edge 1920. Requires a motion backdrop or image entrance; starts at time zero.
- `export_gif`: the same composition with infinite repeat, 20fps, max edge 960.
- `read_video_frame`: rasterize an MP4 at a timestamp into model-visible PNG content using AVFoundation on macOS or FFmpeg on Linux. Requires an absolute path to a local regular file; remote URLs and network protocols are rejected.

Example workflow:

1. Import an image from local disk or clipboard, then call `get_document`.
2. Add an arrow:
   ```json
   {"mark":{"tool":"arrow","points":[[100,100],[400,250]],"curve":[260,60],"color":[255,56,100,255],"width":5,"text":""}}
   ```
3. Add text using one point, `tool: "text"`, and the label in `text`. Font size is `width × 7` pixels.
4. Move/delete/update using an ID returned by the latest state. IDs include a revision and expire after every edit. Optional `expected_revision` rejects stale mutations.
5. Set a moving backdrop:
   ```json
   {"backdrop":{"motion":"lava","preset":0,"padding":100,"inside_padding":24,"seconds":5,"inner_radius":18,"shadow":24}}
   ```
   `padding` is the backdrop margin; `inside_padding` repeats the nearest
   screenshot edge pixels before rounding and shadow. Both accept 0–512 physical
   pixels. Annotation coordinates continue to refer to the original image. The
   former `outer_radius` option has been removed.

6. Call `read_image` to inspect, `export_mp4` to encode, and `read_video_frame` with the returned path and `seconds: 2.5` to inspect a video frame.

Local paths refer to the Mac. ChatGPT upload/file IDs are not native file paths; the client must supply image bytes as base64 or stage the image locally. There is no automatic ChatGPT attachment download integration in this prototype. PNG image tool responses are inline; MP4 exports return a **local path**, not a cloud-downloadable attachment.

## Behavior and limits

Document tools prepare heavy edits on the IPC worker using the same document actions as the UI. They apply successful edits through the editor dispatcher only if the revision is unchanged and no manual gesture/text edit is in progress. Native undo history is preserved. The newest object is selected for direct manual editing. Requests are serialized; a synchronous `export_mp4` or `export_gif` tool call can delay the next MCP call while the native window remains responsive. `dispatch_action` starts editor background operations and returns an acceptance receipt immediately.

The Unix socket is in `~/Library/Caches/sh.glance.desktop/automation/editor.sock`, inside a mode-0700 directory, with mode-0600 socket access. This is local-account access, not isolation from other processes running as you. The bridge does not listen on a TCP port. Exports default to the private `automation/exports` folder; explicit output paths must be absolute and existing files are never overwritten.

Images imported through MCP are limited to 16 MiB encoded / 32 megapixels. Drawing schemas and bounds are validated. The bridge refuses edits against stale revisions and unfinished manual operations. Import starts a new document and discards its previous undo history; the path-based export/edit tools do not prompt for save dialogs.

## Editor actions

Call `dispatch_action` with an `action` object. The `type` chooses the intent;
its other fields carry the parameters. For example:

```json
{"action":{"type":"select_tool","tool":"arrow"}}
{"action":{"type":"fit"}}
{"action":{"type":"pan_by","delta":[40,0]}}
{"action":{"type":"copy_image"}}
{"action":{"type":"resize","scale":2,"smart":true},"expected_revision":7}
{"action":{"type":"set_backdrop","backdrop":{"motion":"aurora","padding":80}}}
{"action":{"type":"set_backdrop_format","format":"shorts"}}
{"action":{"type":"toggle_backdrop_enabled"}}
{"action":{"type":"export_animation","format":"gif"}}
```

These execute through the same dispatcher as the native interface, including
validation, busy checks, undo, and copy feedback. `copy`, `cut`, `paste`, `undo`,
`redo`, and `delete` act on inline text while editing it. `copy_image`,
`copy_remote`, and `paste_image` explicitly act on the image and commit inline
text first. Revision-scoped annotation tools remain available for object edits.

A response such as `{"revision":7,"operation_id":12}` means background work
was accepted; it does not mean the operation completed successfully. Call
`get_editor_state` to inspect `busy`, the active operation ID/kind/progress,
and the status message. Busy actions return an error; cancellation and state
inspection remain available. Capture, open, save, and animation-export actions
have the same native permissions/dialogs as their toolbar counterparts. Use
the path-based export tools when a native save dialog is unwanted.

Restart the automation-enabled editor and MCP companion after rebuilding to
use the new action tools.

Every user-facing command and setting should have a semantic MCP path in the
same change. `src/mcp/contract_tests.rs` compares the actual serializable action
inventory with discovery and checks complete payloads against both the schema
and Rust deserialization. CI runs these checks with `cargo test --locked`.
Raw `edit` actions use indices and are intentionally replaced by revision-scoped
document tools. Pointer slider gestures use `set_backdrop_control` or
`set_animation_control` instead. Worker completion actions are not serializable.

## Verification

`cargo test --locked` covers MCP discovery, schemas, import/edit/read-back, arrow geometry, undo, stale IDs, output-file protection, and GPUI-thread application/conflict handling on GPUI's virtual platform. `cargo test --release --locked native_mcp_video_roundtrip -- --ignored --nocapture` additionally exercises the built encoder/decoder against real MP4 and PNG files. Native desktop visibility and ChatGPT account/tunnel connection require a manual acceptance pass.

### Focus tools and GIF export

`add_annotation` / `update_annotation` also accept `spotlight` and `magnifier` marks. Spotlight uses two opposite rectangle corners. Magnifier uses two points: source center and lens center; its radius is `width × 12` pixels (18–300), and `text` specifies zoom (1.5–4, default 2). Source and lens can be moved independently by replacing the mark's points. These objects support native selection, delete, and undo.

`export_gif` uses the same moving backdrop and output-path rules as `export_mp4`, returning `image/gif`, local path, duration and fps. GIFs repeat indefinitely at 20 fps, max edge 960. The palette is learned across one cycle and reused for every frame. The native bridge must be restarted on the new build to use new tool/object types.

### Annotation options

`get_editor_state` includes `tool_options` for the selected annotation or active
tool. The native sidebar and MCP share these actions:

```json
{"type":"set_appearance","style":{"dash":"dashed","start":"none","end":"arrow"}}
{"type":"set_appearance","style":{"fill":"filled","radius":12}}
{"type":"set_appearance","style":{"cleanup":"adaptive"}}
{"type":"add_line_point"}
{"type":"straighten_line"}
{"type":"set_magnifier_zoom","zoom":3}
{"type":"set_counter_number","number":5}
{"type":"set_crop_ratio","ratio":1.7777778}
{"type":"set_color","color":[18,171,239,128]}
{"type":"sample_tool_color","position":[20,40]}
{"type":"pick_tool_screen_color"}
```

Appearance replaces the style (omitted fields take their default values), edits
the selection, and remembers it for that tool's next annotation. `add_line_point`
and `straighten_line` require a selected line. Other settings work before drawing.
A mark's optional `style` object also round-trips through annotation tools. Legacy
marks retain their original appearance. Lines support up to 32 points; `curve`
controls a two-point line. Opacity is the alpha channel of `set_color`.

The custom color picker uses `set_color`; numeric fields use the existing size,
color and appearance actions. `sample_tool_color` reads an unannotated source
pixel, preserves opacity, updates the selected annotation or active tool's
defaults, and supports native undo for selected annotations. It rejects points
outside the source image and tools without a color option. `pick_tool_screen_color`
opens the native interactive sampler and returns an operation ID; results require
the same operation, document revision and annotation/tool target. Neither action
changes the backdrop. `get_editor_state.sampling_tool_color` reports an internal
canvas sampling gesture. The internal `begin_tool_color_sampling` action is
excluded from discovery; use `sample_tool_color` with source coordinates instead.

Stored annotation widths and corner radii can exceed sidebar ranges after a
resize. Annotation tools accept these physical sizes up to 32768 pixels (width
must be positive) so marks returned by `get_document` remain editable. The
`set_stroke_width` action still uses the toolbar's 0.5–64 pixel range. Resizes
reject transformed geometry outside document limits before changing history.

Local image imports also require an absolute path to a regular file. The 16 MiB
file-size limit is enforced on the actual bytes read, including growing files;
devices, directories, and FIFOs are rejected. Capture and video frame scratch
files live in private temporary directories and are removed after use.

## Custom backdrop colors

`set_backdrop` accepts `colors: [[r,g,b],[r,g,b]]` (opaque sRGB channels 0–255),
or `colors: null` to use the selected `preset`. Older payloads keep preset colors.
`get_document` returns these custom endpoints. Solid fills use the first endpoint;
gradient and motion use both, including PNG/GIF/MP4 output.

The shared dispatcher exposes:

- `set_backdrop_color`: `stop` (0 or 1) and `rgb` (three channels). The other endpoint
  is preserved, initially from the current preset. Changes are undoable.
- `sample_backdrop_color`: `stop` and `position: [x,y]` in source image pixels.
  Samples the original imported/pasted image, excluding annotations and backdrop.
  Out-of-bounds positions are rejected without modifying the document.
- `pick_backdrop_screen_color`: `stop`. Opens an interactive native macOS color
  sampler, or `hyprpicker` on Omarchy. Returns an operation ID; use
  `get_editor_state` until completion. Cancellation keeps the document unchanged,
  and delayed results require the same operation and document revision.

Use `expected_revision` for each dispatch. `set_backdrop_preset` clears custom
colors; changing fill or motion preserves them. The picker offers screen sampling;
automation can sample original source pixels with `sample_backdrop_color`.

## Randomizing backdrop motion

Nebula is the renamed Starfield effect. Use `"motion":"nebula"` with
`set_backdrop` or `select_motion`. The legacy `"stars"` value remains accepted;
`get_document` returns `"nebula"` for either value.

`dispatch_action` accepts `{"type":"randomize_motion"}` to pick a fresh seed for
the current motion. The action requires a moving backdrop and is undoable; it
preserves the effect, colors, framing, duration and paused preview time.
`get_document` returns the selected `backdrop.seed`. Pass that same seed to
reproduce the variation:

```json
{"action":{"type":"randomize_motion","seed":42},"expected_revision":3}
```

Seeds range from 0 to 4294967295; 0 restores the original composition. Omitted or
null seeds choose a new nonzero value. `set_backdrop` also accepts `seed` when
configuring a complete backdrop. All eight effects loop seamlessly for every
seed, and preview and PNG/GIF/MP4 exports use the same variation.

## Incurs and Code Mode

The shell command provides `glance --mcp` for the complete Incurs editor
interface and `Glance --codemode-mcp` for the five Code Mode lifecycle tools.
Start `glance code serve` for shared execution state. The app bundle's original
`Glance --mcp` remains supported; `Glance --cli --mcp` selects Incurs explicitly. See
[CLI, MCP, and Code Mode](../docs/interfaces.md) for setup and command mappings.
