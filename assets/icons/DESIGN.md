# Pachiri app icon

`pachiri.png` is the generated master artwork with transparent margins. The
bundle's `examples/icon.rs` creates all ten standard macOS iconset PNGs (16–1024
physical pixels), filtering premultiplied colors to preserve clean alpha edges.
`scripts/bundle.sh` packages them into `Pachiri.icns` for Dock, Finder, Spotlight
and the app switcher.

Generated using the built-in image_gen tool on 2026-10-03.

## Generation prompt

Use case: logo-brand. Asset type: production macOS application icon for Pachiri, a fast screenshot capture and annotation app. Generate one single finished square icon, straight-on, centered, on a genuinely transparent canvas. Design: a warm coral-red macOS rounded-square tile with generous continuous rounded corners, subtle crafted depth, a softly luminous upper edge and very restrained shadow. Within it, a bold ivory capture-frame symbol formed by four thick rounded viewfinder corner brackets, combined with one beautiful simple diagonal ivory annotation pencil, integrated into the frame as a single memorable emblem. Keep the silhouette bold, balanced and instantly legible at 32px in the Dock and app switcher. Premium friendly native Mac utility aesthetic; smooth enamel-like surface, mostly flat with modest tactile shading, clean geometry and ample internal breathing room. Tile occupies about 82% of square canvas width and height, standard macOS icon optical sizing, centered with transparent margins on all four sides. Coral palette echoes Pachiri's existing warm red/orange accent. No text, no letters, no watermark, no mockup, no device, no scene, no extra decorative sparkles, no multiple variants. Output the actual icon artwork with real alpha transparency outside the rounded tile, not a checkerboard painted into the image.
