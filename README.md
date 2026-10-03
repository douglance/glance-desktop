# Pachiri

A native macOS screenshot and annotation proof of concept written in Rust with
GPUI. Metal shaders compile at runtime, so full Xcode is not required. Inspired by Shottr's fast capture → markup → copy workflow.

## Run

Requires macOS 12+, Xcode Command Line Tools and a current stable Rust toolchain.

```sh
cargo run --locked
```

Build a locally signed app bundle:

```sh
./scripts/bundle.sh
open target/Pachiri.app
```

To launch through Spotlight or Raycast, link the bundle into Applications:

```sh
ln -s "$(pwd)/target/Pachiri.app" /Applications/Pachiri.app
```

Rebuilding updates the linked app. Save or copy your current image before
quitting and relaunching to use a new build.

Use the packaged app consistently so macOS can associate screen-recording
permission with `dev.benv.pachiri`. On first capture, grant access in **System
Settings → Privacy & Security → Screen & System Audio Recording**, then relaunch.
This local bundle uses an ad-hoc signature; distribution signing/notarization is
not configured.

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
- **⌘Z / ⌘⇧Z** undo/redo; **⌘C** copies the composed image; **⌘S** saves PNG.
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
clipboard import and PNG save. A single icon toolbar keeps image dimensions and
zoom visible; native File, Edit, Draw, Zoom and Help menus expose the commands.
See [PLAN.md](PLAN.md) for architecture and the intended proof-of-concept scope.

Deferred: OCR, scrolling capture, uploads, floating pins, image backdrops, custom
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
