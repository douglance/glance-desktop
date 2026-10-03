# Glance proof of concept

Reference: https://shottr.cc/ and the supplied toolbar/editor screenshot.
Shottr combines global capture shortcuts with immediate markup and export. Its
larger feature set includes OCR, scrolling capture, overlays, rulers, backgrounds,
pinning and uploads. This proof of concept implements roughly the core third of
that workflow, rather than attempting feature parity.

## Scope and implementation
1. Build a native Rust/GPUI editor with a compact toolbar and image workspace.
2. Register Cmd+Option+2 (area) and Cmd+Option+3 (full screen) globally.
3. Delegate capture selection to macOS screencapture, hide the editor during
   capture, and reopen with the result. Keep capture work off the UI thread.
4. Keep annotations in image pixel coordinates: pen, arrow, rectangle, text,
   highlighter, pixelation. Use one raster compositor for preview and export.
5. Add crop, undo/redo, fit/zoom, open image, PNG save and image clipboard copy.
6. Package a macOS .app with a stable bundle identifier for screen permissions.
7. Verify compositor geometry, history, compilation and an interactive launch.

## Deliberate boundaries
No OCR, scrolling capture, cloud upload, automatic updates, pinned windows,
annotation object selection/repositioning or configurable shortcuts yet. Native
macOS area selection supports multiple displays; full-screen capture targets the
main display. Captures preserve physical Retina pixel dimensions.

## Architecture
- main.rs: application startup and window creation.
- editor/: GPUI entity, grouped state, commands, input, worker lifecycles and views.
- document.rs: image document, annotation geometry, pixelation, crop and history.
- platform.rs: native macOS capture, dialogs and clipboard integration.
- scripts/bundle.sh: reproducible app packaging.

Screen capture requires macOS Screen & System Audio Recording permission. The
app reports capture failures instead of assuming permission was granted.

## Verification outcome
- Optimized macOS app bundle built and local code signature verified.
- `cargo test --locked`: 3 compositor tests pass.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` pass.
- Native window launched; toolbar selection, pen/arrow markup and clipboard
  export observed in the UI.
- Capture backend invoked from the toolbar. macOS denied capture because screen
  recording permission is not granted. Successful screen/area capture remains
  to be verified after the user grants that permission and relaunches.
- Global shortcuts register without a reported error; native hotkey delivery
  while another app is focused still needs a manual check. GPUI also handles
  these shortcuts while its editor is focused.

## Responsiveness and visual refinement
- Replaced live raster previews with GPUI GPU stroke paths, antialiasing, round
  caps/joins, and display-frame notifications directly from pointer events.
- Moved completed preview compositing, crop and export to workers; coalesced
  pending preview requests and rejected stale generations.
- Replaced hotkey/completion polling with an async channel; no idle timer.
- Shared screenshot pixels across history/export snapshots with Arc.
- Refined toolbar sizing and spacing, hover/pressed states, canvas shadows,
  selection handles, practice-image typography and the original app icon.
- Added long-stroke chunking and shared-history tests plus a reproducible release
  benchmark. Five functional tests pass; the explicit benchmark also passes.
- See PERFORMANCE.md for timings and measurement limits. Updated UI verification
  awaits Mac unlock; previous-build visual checks do not validate this change.

## Lucide controls
- Replaced markup text buttons with Lucide icons and shortcut tooltips.
- Added matching icons to capture/open/copy/save, undo/redo and zoom controls.
- Embedded original SVG assets and included the upstream license in the bundle.
- Release build, formatting and lint checks pass. A separate native preview was
  inspected: all icons rendered, and clicking Arrow changed the selected tool and
  its footer hint. The preview was closed without replacing the user's session.
- User confirmed the preceding performance build feels fast after relaunch.

## Inline canvas text
- Replaced the native text-entry dialog with a GPUI canvas text box, caret,
  pointer selection, clipboard shortcuts and native UTF-16/IME input handling.
- Enter, clicking outside or changing tools commits one annotation; Escape
  cancels. Text undo/redo stays local until the label is committed.
- Matched raster export font scaling to GPUI so committing preserves label size.
- Seven functional tests pass, including grapheme deletion, Unicode ranges and
  text replacement/history. Formatting, lint and signed release build pass.
- Native preview checks verified typing, selection, Unicode paste, text undo,
  cancellation, committing by Enter/tool change and canvas undo after commit.
  The temporary preview was closed; the user's running editor was preserved.

## Backdrop framing
- Added a non-modal side panel with solid/gradient presets (teal by default),
  padding, shadow, image radius and outer radius controls, Done and Remove.
- Document history includes backdrop settings without duplicating screenshot
  pixels. A slider gesture creates one undo entry. Crop operates on the original
  annotated image and keeps the backdrop separate.
- GPUI paints live framing with quads, gradients, rounded image sprites and
  shadows. Slider movement never recomposites the screenshot or uploads textures.
- Copy/save compose full-resolution framing on the export worker; outer rounded
  corners retain PNG transparency. Output dimensions include padding.
- Eleven functional tests pass, including padding/source preservation, gradient,
  rounded corners, shadow, PNG roundtrip, transparency and backdrop/crop history.
- A separate native preview verified presets, gradient and slider controls,
  rounded framing and one-step slider undo/redo. The native save dialog could
  not be inspected through this preview's UI automation; PNG content is verified
  by automated encode/decode tests.
- Final release formatting/lint/signature checks pass. The final zero-padding
  clipping adjustment compiled and passed tests; its additional native UI check
  was unavailable because the Mac became locked. Earlier panel checks succeeded.

## Compact canvas chrome
- Removed the practice-canvas banner and both bottom strips, including routine
  Ready/Cropped messages and always-visible tool instructions.
- Moved undo/redo, output dimensions and zoom into the top tool row. Instructions
  remain in hover tooltips. The canvas gains the former 74 px footer height.
- Operation failures use a native alert so removing the status footer does not
  hide capture/open/save/copy errors.
- Formatting, lint and signed release checks pass. A separate native preview
  confirmed the banner/footer are absent and all top-toolbar controls are visible.
  The test preview was closed without replacing the user's editor session.

## Concise header metadata
- Removed visible undo/redo and zoom +/- controls from the markup toolbar.
- Moved image dimensions and zoom percentage to the right of the main header,
  formatted as width×height and a percentage without captions or unit suffixes.
- Fit mode displays its computed percentage; clicking it fits the image.
- Tooltips contain tool names and shortcuts, with no drag instructions. Keyboard
  undo/redo and zoom bindings remain available.
- Formatting and lint checks pass; the signed app bundle was rebuilt.

## Smart resize and useful image operations
- Added Image tools: 50–400% resize presets, smart upscale, clockwise rotation
  and clipboard import. Added sequential numbered callouts (N); clipboard image
  import also supports Cmd-V outside the text editor.
- Smart upscale uses premultiplied-alpha Lanczos filtering and adaptive, bounded
  sharpening. No model, upload or additional runtime dependency is required.
  It improves edge clarity but is not learned AI super-resolution.
- Resize keeps vector annotations and scales their coordinates, stroke and font
  sizes so the compositor redraws them at the target resolution. Backdrops keep
  their physical pixel padding. Rotation flattens marks and is undoable.
- Transforms run on workers, retain undo history and reject excessive output
  dimensions before allocating. Text and number minimum sizes are enforced at
  placement rather than rendering so shrinking annotations works consistently.
- Fifteen functional tests pass, including sharpness bounds, flat areas,
  transparent edges, resize/history and rotation pixel mapping.
- OCR was excluded after the user clarified that it is low priority.
- Native preview verified Lucide controls, numbered callouts and 2× smart resize
  (1200×760 to 2400×1520). The user began editing the test preview during checks;
  further UI automation stopped and the preview was left open to preserve work.

## Editable annotations and compact native controls

Annotations have geometry hit tests, topmost selection, drag translation and
undoable deletion. V activates the picker. Crop translates marks into the new
image coordinates instead of flattening them. The 48 px toolbar uses Lucide
icons and tooltips; native File/Edit/Draw/Zoom/Help menus reuse editor commands
and preserve the text editor’s clipboard and undo behavior.

## Navigation and interaction polish

Implemented native trackpad pinch/smart zoom, anchored wheel/Z-click zoom,
two-finger and Space-drag pan, 1/10 px annotation nudges, duplication, selected
object styling, Shift constraints and crop edge snapping, and file drop to open.
References: [Shottr tips](https://shottr.cc/#tips) and
[CleanShot features](https://cleanshot.com/features). Direct Shottr inspection
was blocked by computer-use access; behavior above is our implementation based
on the published descriptions, not a verified pixel-for-pixel reproduction.

## Animated backdrop video

Implemented periodic Flow, Lava, Starfield and Painterly scenes beneath a fixed
foreground, display-synchronized GPU preview, play/pause and a 2–15 second cycle
slider. PNG captures the current phase. A Rust worker streams bounded 30 fps
BGRA frames to a small bundled Swift AVFoundation adapter, which writes H.264
MP4 using a pixel-buffer pool. Export has progress, cancellation, temporary-file
commit and no FFmpeg/runtime downloads. Composition remains in Rust/GPUI;
the Swift helper is an encoding bridge without UI or screen access.
