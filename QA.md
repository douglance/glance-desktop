# QA — 2026-10-03

## Result

91 automated tests pass (10 opt-in tests ignored). Native desktop testing remains blocked: computer-use
access to the former desktop app was denied. These results cover a virtual GPUI window and
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

## Motion shaders

Shader previews now render on a persistent worker. Blocking-renderer tests
verify that requests return while rendering is stalled, pending requests
coalesce to the latest frame, and effect switches (including switching back),
pausing and cancellation reject old completions. Suspended redraws retain their
phase; worker shutdown does not wait for the renderer. A real shader-worker
test verifies completions arrive from another thread and match the expected
BGRA frames for all six shader effects. UI atlas upload/display still needs
native latency profiling.

```sh
cargo test --locked animation::preview
```

Adaptive-quality tests use deterministic render durations to check warmup,
sustained overload, emergency downshifts, the 480 px floor, and slow recovery
without oscillation. Paused previews use the 960 px cap. Adaptation measures
worker rendering only; native display FPS and slower-Mac performance remain
unverified. PNG/MP4/GIF exporters do not consult preview quality.

Liquid, Lava, Aurora, Contours, Prism and Painterly are checked against their CPU evaluators in landscape and portrait, across
all eight palettes and three phases (at most two RGB levels of difference).
The shared animation tests verify seamless looping and unchanged opaque
foreground pixels. Visual samples are generated in `target/liquid-qa`.
The additional styles' portrait and landscape samples are in `target/motion-qa`.

```sh
cargo test --locked liquid_visual_qa -- --ignored --nocapture
cargo test --locked liquid_video_qa -- --ignored --nocapture
cargo test --release --locked motion_gallery_qa -- --ignored --nocapture
cargo test --release --locked contours_motion_qa -- --ignored --nocapture
cargo test --release --locked painterly_prism_motion_qa -- --ignored --nocapture
```

The second command requires the bundled native encoder. It creates a ten-second
shader-only MP4 and a screenshot backdrop MP4, and verifies in-flight
cancellation leaves the completed destinations untouched. Native GPUI preview
upload/display timing and interaction feel still need a desktop pass.

The expanded eight-effect export pass produced real H.264 videos for Liquid,
Lava, Aurora, Contours, Prism, Painterly, Flow and Starfield. Cancellation
preserved each completed destination. Independent probing of the three new
styles confirmed 680×470, 30 fps, 150 frames and five-second duration. Samples
were inspected in portrait and landscape; Contours uses analytic pixel coverage
to keep steep lines continuous. CPU/Metal parity, exact loop boundaries and fixed
foreground coverage pass for the expanded set. Compute plus readback at 960×540
measured about 0.24–0.70 ms/frame across the six shader styles in the last pass;
this excludes GPUI texture upload and display latency.

Contours now deforms its terrain with independently traveling waves and moves
the contour levels through that terrain. The evolving-effects continuity test checks
both the half-cycle phase branch and the loop boundary for abrupt pixel jumps.
`contours_motion_qa` produces a six-second frame sequence in `target/contours-qa`
and a portrait sample for reviewing line expansion and local bending.

Painterly grows curved brush strokes from anchored tails on staggered cycles.
Prism animates shared mesh vertices and face reflections while keeping triangle
edges straight. Both use periodic motion in their Metal and CPU evaluators.
`painterly_prism_motion_qa` writes six-second landscape frame sequences and
portrait samples to `target/dynamic-motion-qa`, and measures compute/readback.

## Glance remote copy

Copy (remote), Edit → Copy (remote), and Cmd-Shift-C share the composed PNG
through Glance's existing client-upload protocol. The native flow hasn't been
manually exercised. Virtual GPUI tests verify that upload completion copies
`Screenshot: <url>`, failure preserves the clipboard, concurrent requests are
ignored, and the toolbar fits at its minimum 1050-pixel width.

An independent Node crypto fixture verifies byte-for-byte HKDF/AES-GCM and
storage-path compatibility with Glance. A local HTTP integration test verifies
clock synchronization, proof issuance, client-token exchange, private Blob
headers, encrypted PNG round-trip and absence of the share token from upload
requests. Additional checks cover size boundaries, rate limits, malformed
responses, server errors and invalid clocks/lifetimes.

The opt-in live test uploaded a generated 3×2 PNG to production `glance.sh` and
fetched the returned share link, verifying identical decoded pixels. This test
is excluded from the default suite so routine tests do not upload anything.

```sh
cargo test --locked live_upload_round_trips_through_glance -- --ignored
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

## Local MCP companion

- Regular suite now includes MCP initialization/tool discovery, schema validation, image-byte import, crop/resize/backdrop, model-visible PNG read-back, editable arrow movement/curve, stale IDs, undo, and existing export-file protection.
- A Unix socket-pair integration test exercises serialized requests through the same bridge dispatch used by the native listener.
- GPUI virtual-platform test applies an MCP-generated annotation to the editor, checks automatic selection/native undo, and rejects stale revisions and a busy editor.
- Explicit native MP4 roundtrip test exports a real two-second H.264 clip, decodes a one-second frame with AVFoundation, checks dimensions/foreground color, rejects out-of-duration frame reads and overwriting existing videos.
- Native desktop operation and a live ChatGPT Secure MCP Tunnel connection are not verified by these tests. They require an opt-in editor and an account/workspace with tunnel/developer-mode access.

## Spotlight, magnifier and loop export

- Focus tests cover spotlight union masks, dimming, undo, a bright lens sampling the undimmed annotated source, circular hit testing, independent source/lens handles, and bubble-only texture-key reuse.
- GPUI virtual-platform event tests draw/select/resize a spotlight, create a magnifier, move each endpoint separately, delete and undo.
- GIF tests decode actual exports to verify infinite-repeat metadata, 40 frames / exact two-second duration, stable foreground pixels, cancellation and temporary-file cleanup.
- Loop continuity tests cover all four backgrounds: phase 0 equals phase 1 exactly, and the seam is no larger than a normal animation step within the test tolerance.
- Explicit `focus_and_loop_demo_qa` renders a five-second GIF, MP4 and full-resolution PNG with focus effects for visual inspection. Native live canvas interaction remains separate from virtual-platform tests.
## Screen Recording grant after a rebuild

macOS tccd logs for the former Pachiri app reported “Failed to match existing code requirement” for
`dev.benv.pachiri` / `kTCCServiceScreenCapture`. `codesign -d -r-` showed a
build-specific cdhash designated requirement, and no code-signing identities
were available in the local Keychain. This confirms the enabled Settings entry
was not authorizing the installed build.

Capture now preflights permission and requests the standard macOS grant before
hiding the editor. Rejected grants show remove/re-add instructions; unrelated
capture failures preserve screencapture stderr rather than alleging a missing
permission. Regression coverage checks cancellation and error classification.
Bundle builds now use a persistent development certificate in
`~/Library/Application Support/Glance/Signing`, migrating existing signing files
without replacing the certificate. New certificate trust is an explicit,
one-time user-domain code-signing step via `scripts/trust-local-signing.sh`.
`GLANCE_CODESIGN_IDENTITY` selects an existing certificate; `-` opts into ad-hoc
signing. Build/signing failures preserve the installed bundle. Nested helpers
are signed first, then the staged bundle is signed and strictly verified.

`./scripts/test-local-signing.sh` verifies two different signed bundle hashes
satisfy the same certificate-based requirement. The renamed bundle passed this
check locally. Glance uses `sh.glance.desktop` and needs a one-time Screen
Recording grant after the rename; actual desktop capture remains a manual pass.
