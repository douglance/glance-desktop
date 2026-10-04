# Changelog

Notable user-visible changes are recorded here. Glance has no published releases
yet; the current development version is 0.1.0.

## [Unreleased]

### Added

- Image Animation sidebar with diagonal reveal, spring pop, and 3D settle;
  independent backdrop motion, replay/scrubbing, and optional exits in MP4/GIF.

- Contextual sidebar options for all annotation tools, with separate defaults
  remembered during the session and undoable edits to selected annotations.
- Dashed and dotted lines, independent arrow/dot ends, editable waypoints,
  filled and rounded boxes, and reversible Raw/Smooth/Adaptive pen cleanup.
- Text sizing and opacity, highlight intensity, pixelation block size, crop
  aspect ratios, step numbers, spotlight dimming, and magnifier defaults.

- Native macOS area and main-display capture, editable annotations, crop,
  resize, rotation, clipboard import/export, and PNG save.
- Spotlight and magnifier tools, solid and gradient framing, aspect-ratio
  presets, and eight animated backdrops with MP4 and looping GIF export.
- Temporary image sharing through glance.sh and an opt-in local MCP companion.
- MIT license, contributor and security guides, and macOS build validation.

### Changed

- Replaced backdrop canvas corners with inside padding that extends screenshot
  edge pixels, with matching image corners, shadows, preview and PNG/GIF/MP4 output.

- Reorganized the README around trying the app and sharing visual context, with
  separate usage, development, and architecture guides.

### Fixed

- Resized annotation widths and corners stay editable and round-trip through MCP;
  invalid transformed geometry is rejected before changing the image or undo history.
- Zero-length arrows retain their live stroke in exports, and incompatible backdrop
  durations return an action error before changing an image entrance.
- Text exports preserve glyph overhangs such as the left edge of “j”.

- MCP action discovery includes backdrop format and enable/disable controls;
  animation export descriptions reflect image entrances over still or absent backdrops.

- Translucent annotation exports preserve opaque image alpha and blend text
  and stroke joints consistently.
