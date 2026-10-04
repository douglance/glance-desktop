# Changelog

Notable user-visible changes are recorded here.

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

### Changed

- Replaced backdrop canvas corners with inside padding that extends screenshot
  edge pixels, with matching image corners, shadows, preview and PNG/GIF/MP4 output.

- Linux release builds reuse optimized dependency libraries cached on `main`
  across release tags, alongside the existing CI test cache.

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

## [0.1.0] - 2026-10-03

Initial release of Glance, a native screenshot editor for annotation, animated
backdrops, and sharing visual context with coding agents.

### Added

- Native macOS area and main-display capture, editable annotations, crop,
  resize, rotation, clipboard import/export, and PNG save.
- Spotlight and magnifier tools, solid and gradient framing, aspect-ratio
  presets, and eight animated backdrops with MP4 and looping GIF export.
- Temporary image sharing through glance.sh and an opt-in local MCP companion.
- MIT license, contributor and security guides, and macOS build validation.
- Experimental Omarchy/Hyprland support with Wayland capture and clipboard,
  Linux dialogs/fonts, Ctrl shortcuts, and FFmpeg video helpers.
- Release-triggered macOS ARM64 ZIP and Omarchy x86_64 Arch package builds with
  checksums attached to GitHub releases; regular CI includes video encode/decode
  checks.
- `--open`, `--capture-area`, and `--capture-screen` startup options.

### Changed

- Reorganized the README around trying the app and sharing visual context, with
  separate usage, development, and architecture guides.

### Known limitations

- The macOS download supports Apple Silicon on macOS 12+. It is ad-hoc signed
  and not notarized; macOS may block opening it. Source builds use a persistent
  local development signing identity. Developer ID signing and Intel/universal
  downloads are not configured.
- Omarchy/Hyprland support is experimental and still needs a real desktop
  acceptance pass. Animated Linux backdrops render on the CPU.
- Save or copy before replacing an image or quitting; editable sessions are
  not persisted. OCR, scrolling capture, and automatic updates are not available.
