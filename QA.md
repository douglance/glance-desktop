# QA — 2026-10-03

## Result

39 automated tests pass. Native desktop testing remains blocked: computer-use
access to Pachiri was denied. These results cover a virtual GPUI window and
model/rendering logic, not the physical app's visual layout or input latency.

## Bugs found and fixed

- Drawing menu key bindings intercepted ordinary letters while editing text,
  switching tools and closing the text box. Bindings now require the canvas
  context. The regression test types every tool shortcut through GPUI's full
  event dispatcher, then verifies Enter commits and Cmd-Z undoes once.
- Deleting an earlier numbered callout could duplicate a remaining step number.
  New callouts now follow the highest remaining number.
- Crop edge snapping could choose the opposite edge on small/zoomed-out images.
  Snap distance is now capped separately on each axis.

## Arrow and selection update

New annotations remain selected without switching away from the drawing tool.
Regression coverage checks independent endpoint drags, midpoint curvature,
whole-arrow movement, immediate deletion, cancelled drags, one-step undo/redo,
Shift angle snapping and preservation of the pointer's handle grab offset.
Curve geometry and antialiased filled heads are shared by GPU overlays,
hit-testing and export. Tests also cover curved PNG output, crop/resize history,
zoom-relative handle targeting and zero-length arrows. A rendered curved-arrow
fixture was visually inspected. The text editor's fill is now transparent.

Release curved-arrow path preparation measured p50 0.005 ms / p95 0.006 ms
(1,000 samples). This measures CPU geometry preparation, not display latency.

## Automated coverage

Eleven GPUI interaction tests exercise drawing, picking, moving, duplication,
deleting, undo, Shift constraints, Escape cancellation, Space pan, pinch message
handling, fit/quick zoom, selected styling, grouped keyboard nudges, Unicode text,
IME composition, and rejection of outdated worker previews. Global shortcuts
and AppKit gesture monitors are disabled in the virtual test platform.

A deterministic stress test runs 20 sessions of 100 edits. Every operation checks
undo and redo against the rendered image. Operations include annotations, move,
delete, solid/gradient backdrops, rounded corners, shadows, rotation, crop and
plain/smart resize. Every tenth operation also checks lossless PNG encode/decode
(200 round trips total). Existing tests cover shared history pixels, alpha-aware
resize, export geometry and long GPU paths.

## Release benchmark

GPU path preparation p95: 0.093 ms for 500 points, 0.195 ms for 2,000
points, and 0.982 ms for 10,000 points. A 4K history commit measured 0.002 ms
p95. These are CPU preparation measurements from this run, not physical input
latency or frame-time guarantees.

## Animated backdrops and native video export

Flow, Lava, Starfield and Painterly are deterministic periodic scenes. Tests
compare phase 0 and 1, check actual motion at phase 0.37, and verify every opaque
foreground pixel stays identical. Additional coverage checks duration clamping,
phase preservation while changing duration, pause/play, Escape cancellation,
undo/redo, shared source pixels and even output dimensions bounded to 1920 px.

The explicit native integration test generated real H.264 MP4s: three 5-second
videos with 150 frames and one 10-second video with 300 frames, all at 30 fps.
ffprobe verified codec, dimensions, frame counts and exact durations; FFmpeg was
used only for independent decoding in QA, not by the app. Posters were visually
inspected and a stepped-ring artifact was replaced with continuous Gaussian
shading. Actual in-flight cancellation preserved each existing destination.
The encoder streams frames with a pixel-buffer pool and temporary-file commit.
Independent decoded foreground samples varied by a mean 0.64 RGB levels out of
255 (maximum 14), consistent with lossy H.264; the pre-encoding foreground is
identical. A higher bitrate budget and disabled frame reordering improved text
stability. No canceled-export temporary files remained.

Reproduce the native integration pass after building the app:

```sh
./scripts/bundle.sh release
cargo test --release --locked native_motion_export_qa -- --ignored --nocapture
```

## Remaining desktop pass

- Actual mouse/trackpad feel, pinch and smart zoom, wheel momentum.
- Native menu shortcuts and mouse activation, toolbar fit at minimum width.
- Capture selection/cancellation and screen-recording permissions.
- Clipboard import/export, Finder file drop, save/open dialog cancellation.
- Visible selection outlines, text caret/IME candidate positioning and framing.

Run the reproducible checks:

```sh
cargo test --locked
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --release --locked drawing_preparation_benchmark -- --ignored --nocapture
```

The benchmark measures CPU preparation, not input-to-display latency.
