# Changelog

Notable user-visible changes are recorded here. Glance has no published releases
yet; the current development version is 0.1.0.

## [Unreleased]

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

### Fixed
