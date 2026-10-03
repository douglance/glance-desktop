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
