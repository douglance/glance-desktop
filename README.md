# Glance

Glance is the native macOS desktop companion to [glance.sh](https://glance.sh):
capture your screen, annotate it, and share a temporary image link with a remote
coding agent. Written in Rust with GPUI. Metal shaders compile at runtime, so
full Xcode is not required.

## Run

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

## Workflow

- **⌘⌥2** captures an area using the native macOS selector. Escape cancels.
- **⌘⌥3** captures the main display. The app hides itself before capture.
- **V** selects annotation objects. Click a stroke, shape, label, highlight,
  numbered callout or pixelation region; drag to move it, then press Delete or
  Backspace to remove it. Clicking empty canvas or Escape clears selection.
  The topmost matching object wins. Each move/deletion is one undo step.
  Crop and resize preserve editable annotations; rotation still flattens them.
- Mark up with **P** pen, **A** arrow, **R** rectangle, **T** text, **H** highlight,
  **B** pixelation or **X** crop. Drag to crop immediately; undo restores it.
- New annotations remain selected: drag the mark to move it or press Delete.
  Click empty canvas to draw another with the same tool. Selected arrows have
  endpoint handles to reorient them and a middle handle to bend them; Shift
  snaps endpoint drags to 45°. Text editing has a transparent background.
- Click a color or the stroke-width button (3, 5, 9 px) to change the selected
  annotation, or the next mark when nothing is selected. **⌘D** duplicates the
  selected annotation. **Arrow keys** nudge it 1 pixel; **Shift + arrows** move
  10 pixels. Held key repeats are grouped into one undo step.
- **Shift-drag:** arrows snap to 45° angles; boxes, highlights, pixelation and
  crops become squares; moving an object locks to the dominant axis. Crop
  endpoints snap to image edges within 8 screen pixels.
- **Text:** click the canvas and type directly into the outlined box. Enter or
  clicking outside finishes the label; Escape cancels. Selection, copy/paste,
  text undo/redo and native macOS input methods work while editing. Labels are
  single-line; pasted line breaks become spaces.
- **Backdrop:** open the toolbar panel to frame the image with a solid color or
  gradient. Choose a preset (including teal), then drag padding, shadow, image
  corners and backdrop corners. **Done** closes the panel; **Remove backdrop**
  restores the original framing. Each slider gesture is one undo step. Copy and
  PNG save include the backdrop at full resolution; rounded outer corners are
  transparent in the PNG.
- **Animated backdrops:** choose **Backdrop → Motion**, then Flow, Starfield,
  Aurora, Contours, Painterly, Prism, Liquid or Lava. Screenshot and annotations stay fixed while the
  background moves. Set a **2–15 second** duration (5 seconds by default),
  pause/play the preview, and choose **Export MP4…**. Export streams 30 fps
  H.264 video through macOS AVFoundation, preserving aspect ratio with a maximum
  1920-pixel edge. The bundled encoder needs no FFmpeg installation. Each video
  is one seamless cycle; duration also controls the preview's cycle speed.
  **Cancel export** or Escape stops it. Rounded outer corners use an ivory matte
  in MP4; PNG retains transparency and captures the current animation phase.
  Liquid has flowing ribbons and fine grain; Lava has molten blobs that merge
  and separate; Aurora has rippling light curtains; Contours has terrain lines
  that ripple, swell and curl; Prism has flexing facets and traveling reflections;
  Painterly has brush strokes that sweep, bloom and fade. These
  six effects use distinct procedural Metal shaders. Flow keeps a gentle cloud
  gradient, and Starfield has drifting stars. Each effect starts with a suggested
  palette, and all eight palettes remain available. Shader preview is capped at
  960 pixels and 30 fps, with the same renderer used for full-resolution PNG and
  MP4 export. A CPU fallback preserves the effect when Metal is unavailable.
- **⌘Z / ⌘⇧Z** undo/redo; **⌘C** copies the composed image; **⌘S** saves PNG.
  Copy shows a brief **Copied!** confirmation and a checkmark on its button.
- **Copy (remote) · ⌘⇧C** (cloud-upload icon) uploads the composed PNG (including annotations and
  backdrop) to [Glance](https://glance.sh) and copies `Screenshot: <url>`.
  An **Uploading…** indicator stays visible until completion, then **Link copied!**
  confirms success with the link lifetime. The confirmation dismisses after three seconds.
  Paste it into a remote agent’s chat; the agent can fetch the image directly.
  Links expire after about 30 minutes. Uploads use Glance’s client encryption
  and private Blob storage, require internet, and are limited to 15 MB and
  30 uploads/hour per IP. Failed uploads preserve your clipboard. This shares
  a link; it does not automatically push into an agent’s live session.
  Glance API requests identify the app with `X-Glance-Client: glance-desktop` and
  `X-Glance-Client-Version: <app version>`, alongside `User-Agent: Glance/<app version>`.
- **⌘O** opens PNG/JPEG. **⌘1** fits, **⌘0** uses 100%, **⌘+ / ⌘−** zoom.
  Pinch zooms around the pointer (1–800%); two-finger scrolling pans. **⌘ +
  scroll** zooms; **Shift + wheel** pans horizontally. Hold **Space** and drag
  to pan, or right-drag. Hold **Z** and click to zoom in at that spot; Shift-click
  zooms out. Trackpad smart zoom (two-finger double-tap) toggles 100%/fit.
  Escape cancels an unfinished mark. **⌘Q** quits.
- Drag an image file from Finder onto the canvas to open it. This replaces the
  current image, like Open; save or copy your work first.
- The starter image is a practice canvas. Capture or open replaces it.
- **N** places sequential numbered callouts. Undo removes the last step.
- **⌘V** loads an image from the clipboard when you are not editing text.
- **Image tools** opens resize and rotation controls. Choose 50–400% and apply.
  Smart upscale combines Lanczos resampling with bounded adaptive sharpening;
  it is local image processing, not AI super-resolution, and cannot reconstruct
  missing detail. Vector annotations redraw at the target resolution. Resize
  and 90° clockwise rotation support undo; rotation flattens existing marks.
  Backdrop padding stays in physical pixels. Large resizes are limited to
  16,000 pixels per side and 64 megapixels.

Annotations and export use physical image pixels, including Retina captures.
The preview is capped at 1600×1200 for responsiveness; saved/copied images retain
full resolution. Live pen/arrow/box/highlight strokes use GPU overlays;
compositing, crop and export run on workers. Pixelation shows its selected area
while dragging and applies on release. Text uses the macOS Arial font and an
on-canvas editor. Typing and selection render directly through GPUI.
History retains up to 30 states and shares immutable image pixels between
annotations. Cropped images retain separate pixels so crop can be undone. Closing the window hides the app and keeps global shortcuts active.
Click the Dock icon to reopen; ⌘Q quits. Save/copy before replacing the current capture.

## Scope

Implemented: global area/full-screen capture, pen, arrows, rectangles, text,
highlights, pixelation, crop, backdrops, numbered callouts, smart upscale/resize,
rotation, object selection/movement/deletion, undo/redo, fit/zoom/pan, open,
clipboard import, PNG save, Glance remote copy, spotlight, magnifier, local MCP control and animated backdrop MP4/GIF export. A single icon toolbar keeps image dimensions and
zoom visible; native File, Edit, Draw, Zoom and Help menus expose the commands.
See [PLAN.md](PLAN.md) for architecture and the intended proof-of-concept scope.

Deferred: OCR, scrolling capture, live-session delivery, floating pins, image backdrops, custom
backdrop colors, object resizing, configurable shortcuts and persistent settings.
Pixelation is a visual effect; crop out information you need to fully remove.

```sh
cargo test --locked
cargo fmt --check
```

See [PERFORMANCE.md](PERFORMANCE.md) for the reproducible drawing benchmark and
its measurement boundaries, and [QA.md](QA.md) for interaction/stress test coverage
and the remaining native desktop checks.

## Icons

The toolbar uses embedded Lucide SVGs with hover labels and keyboard shortcuts.
GPUI caches the SVG rendering; the app requires no network connection for icons.
Upstream version and license are in `assets/lucide/SOURCE` and
`assets/lucide/LICENSE`. The license is also included in the app bundle.

### Local MCP companion

Glance can expose its native editor to ChatGPT and local MCP clients: import images, edit selectable objects, crop/resize, set animated backdrops, return PNG previews, export MP4, and decode video frames. Start the editor with `--automation` and the stdio server with `--mcp`. See [setup, tools, and ChatGPT tunnel instructions](mcp/README.md).

### Spotlight, magnifier, and GIF loops

- **S — Spotlight:** drag a focus rectangle. The surrounding image dims; multiple focus windows share one dimming mask in exports. Drag the object to move it, or drag either corner handle to resize it. Undo/Delete work as with other annotations.
- **M — Magnifier:** drag from a detail to where its enlarged lens should appear. The source and lens have separate handles. The toolbar's **2× / 3× / 4×** button changes a selected lens's magnification; **Ø** changes its diameter. It samples the original annotated foreground, so the enlarged detail stays bright even with a spotlight.
- **Backdrop → Motion → GIF…** or **File → Export Looping GIF…** exports an infinitely repeating GIF. MP4 export remains available beside it. GIF uses 20 fps and a maximum edge of 960 pixels; MP4 uses 30 fps and 1920 pixels. Both render one complete cycle, excluding a duplicate endpoint frame. GIF's fixed palette keeps foreground colors stable across frames; rounded corners use the same ivory matte as MP4.
- Spotlight and magnifier remain editable objects and appear in PNG, clipboard, GIF, and MP4 output. Animated backdrops loop while the foreground stays fixed. MP4 repeats when the player is configured to loop; GIF includes infinite-repeat metadata.

## Editor architecture

`main.rs` only launches the app and opens its window. The GPUI editor lives in
`src/editor/`: `mod.rs` constructs the entity, `state.rs` groups its state,
`commands.rs` owns edits and external actions, and `input.rs` translates pointer
and keyboard events into those actions. `view.rs`, `canvas.rs`, and `panels/`
build and paint the interface. `text_input.rs` implements native text input;
`text.rs` owns the Unicode buffer and text history independently of the editor.

One `Gesture` enum represents the active pointer interaction. `jobs.rs` owns
worker dispatch and completion: each external operation has an ID, and stale
results or video progress cannot affect a newer operation. Preview rendering
has its own revision checks and remains independent of external operations.
`feedback.rs` owns transient copy confirmations. `automation.rs` applies local
MCP requests on the UI thread, and `lens.rs` schedules magnifier previews.

The document, geometry, compositors, macOS integration, Glance protocol, and
video encoder remain separate modules. Interaction tests use GPUI's virtual
platform in `src/editor/tests.rs`, without controlling the user's desktop.
