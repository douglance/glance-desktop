# Drawing performance

## Changes

Pointer events append image-coordinate points and notify GPUI directly. The live
stroke is an antialiased GPU path with round caps and joins. Pointer motion does
not clone screenshot pixels, rasterize annotations, create RenderImages or upload
textures. Nearby samples within 0.35 screen pixels are coalesced; long paths are
split into overlapping chunks to avoid GPUI's u16 vertex limit.

Completed marks remain visible as GPU overlays until a worker has produced the
composed preview. Only one preview worker runs at a time; newer changes coalesce
into its next job. Revision checks reject stale results after undo/new captures.
Crop and full-resolution export also run on workers. Idle hotkey/completion
polling was replaced by an async event channel, removing the former 24 ms delay
and periodic wakeups.

Undo states and export snapshots share immutable source image pixels through
Arc. Adding an annotation no longer duplicates the screenshot. Crop creates a
new image and retains the old shared source for undo.

## Reproduce

```sh
cargo test --release --locked drawing_preparation_benchmark -- --ignored --nocapture
```

Measured locally on this Mac, optimized build, 100 samples after 10 warmups.
The legacy comparison reproduces the former 1600×900 preview copy/composition,
BGRA conversion and RenderImage allocation. GPU measurement includes mark
cloning and GPUI path tessellation, with a 4K image-coordinate stroke at 40% scale.

| Stroke points | GPU path p50 | GPU path p95 | Former raster p50 | Former raster p95 |
| --- | ---: | ---: | ---: | ---: |
| 500 | 0.082 ms | 0.093 ms | 0.655 ms | 0.791 ms |
| 2,000 | 0.194 ms | 0.200 ms | 0.625 ms | 0.647 ms |
| 10,000 | 0.986 ms | 1.037 ms | 0.839 ms | 0.869 ms |

4K history commit (two-point marks, capped at 30 undo states): p50 **0.001 ms**, p95 **0.002 ms**.

These are CPU preparation measurements, not input-to-display latency or measured
frame rate. They exclude GPUI layout/painting, Metal execution, texture uploads,
OS input scheduling and display refresh. The old approach also waited up to
24 ms for its polling timer and uploaded a new preview texture during drawing;
the new live path does neither. At extremely long stroke lengths tessellation
can cost more CPU than the bounded raster comparison, and was approximately 1 ms in
this run. No fixed FPS or display latency guarantee is claimed.

## Validation

Compositor history/crop, pixelation bounds, non-destructive drafts, shared 4K
history pixels, and 10,000-point path chunking are covered by automated tests.
The updated native visual/input check was attempted, but computer use reported
that the Mac was locked. That check must be completed after unlock; the prior
build's native drawing/clipboard check does not verify this new rendering path.

Backdrop controls paint their live preview through GPUI. Changing padding,
colors, gradients, corners or shadow only updates framing metadata; it does not
clone screenshot pixels or invoke the raster compositor. Full-resolution
backdrop composition runs on the existing export worker. Slider undo snapshots
share the immutable screenshot Arc, and each drag produces one history entry.

Resize, adaptive sharpening and rotation run on the existing worker channel.
Image edits swap in completed previews; normal canvas pointer events retain the
GPU overlay path. Upscale redraws vector annotations instead of enlarging their
rasterized pixels. Output limits are checked before allocating resized buffers.

Object dragging retains the original document until mouse-up. A worker prepares
the raster prefix below the selected object once per gesture; the selected mark
and subsequent layers render as GPU overlays during pointer movement. Release
commits one history snapshot and schedules the normal composed preview. Pixelation
regions above that prefix use their selection outline during dragging and are
fully recomposited on release. No screenshot rasterization runs in mouse-move.

Pinch, wheel zoom and pan update only the viewport transform and request a GPUI
redraw. AppKit magnify/smart-magnify events are bridged into the editor channel
because GPUI 0.2.2 does not expose these events. The monitor is local to the app,
filters to canvas bounds, and is removed when the editor drops. No timer polling,
screenshot recomposition or texture upload is used by zoom or pan handlers.

Arrow endpoint and curvature drags update vector geometry and paint a quadratic
GPU path with a filled triangular head. The midpoint lies on the visible curve;
export uses the same geometry with a supersampled coverage mask to avoid colored
alpha fringes. The mask is allocated only on the compositor worker. Curved-arrow
path preparation measured p50 0.005 ms / p95 0.006 ms over 1,000 release samples.

## Animated backdrops

Display-frame requests run only for an active window with a playing motion
backdrop. Flow and Starfield paint GPUI Metal gradient, Gaussian shadow and star primitives;
the screenshot texture stays constant. Animation ticks allocate no screenshot
pixels, upload no image textures, launch no compositor workers and create no
undo snapshots. Pause, window deactivation and static backgrounds stop frame
requests. Drawing and normal object edits retain the existing GPU overlay path.

Video export composites the screenshot/annotations once, caches the foreground
and shadow coverage, then streams one bounded frame at a time to an AVFoundation
helper. Soft-blob export shading uses a cached lookup of GPUI's Gaussian
integration. Opaque foreground pixels bypass animated shading. H.264 output is
30 fps with a maximum 1920 px edge; no frame sequence is retained in memory.
Encoding speed varies with output size and effect; native desktop preview frame
times/input latency have not been measured because app control remains denied.

MCP operations use a serial IPC worker. Image import, document transforms, PNG rendering, and MP4 encode/decode do not run on the GPUI thread. The worker snapshots shared document pixels/history, then applies edits only if the editor revision is unchanged. Preview rendering uses the existing asynchronous cache. Video frames stream to AVFoundation rather than accumulating a clip in memory; inline PNG previews are bounded and do not upscale small source images.

Focus effects: spotlight drafts use GPU quads; magnifier drafts use a bounded circular texture with asynchronous source sampling. A lens texture key includes source position, zoom, radius and document revision, but excludes bubble position, so dragging only the lens reuses its texture. Source-handle movement coalesces sampling jobs. Pointer handlers never rasterize a focus effect. Final focus composition is performed by the existing preview/export workers.

GIF encoding renders the fixed foreground once and streams 20 fps frames at ≤960px. A fixed 256-color palette sampled across the whole animation prevents per-frame palette shimmer. All eight background effects pass a periodic endpoint check and a last-to-first step-size check.

Liquid, Lava, Aurora, Contours, Prism and Painterly use a runtime-compiled Metal compute pipeline and a reused shared
output buffer. GPUI 0.2.2 does not expose custom RGBA shader painting, so this
prototype reads GPU output into an image and uploads it to GPUI's atlas. Preview
is capped at a 960 px edge and samples 30 frames per second; unchanged/paused
frames reuse the existing image. Old atlas entries are removed on replacement
or when leaving shader effects. Effect selection is included in the preview
cache key, so switching effects at the same phase/palette refreshes the image.
Screenshot/annotation textures remain unchanged.
Export uses the same shader at the requested output size, then composites the
cached foreground and shadow on the CPU. A matching CPU evaluator is used only
if Metal initialization or execution fails.

The shader QA measured approximately 0.42 ms/frame for compute plus readback at
960×540 and 1920×1080 on this Mac. This excludes BGRA conversion, GPUI atlas
upload, layout, foreground composition and display latency; it is not an
end-to-end frame-rate measurement. Reproduce with:

```sh
cargo test --locked liquid_visual_qa -- --ignored --nocapture
```

`motion_gallery_qa` generates portrait/landscape samples of all six shader
effects and measures their compute-plus-readback cost. Painterly evaluates
fourteen textured brush marks per pixel; Lava evaluates six merging metaballs.
The geometric, contour and aurora modes use independent formulas rather than
recoloring Liquid or Flow. All periodic motion is driven by sine/cosine of the
loop phase, so frame zero and frame one match exactly.
