# Tool options map

The right sidebar follows the selected annotation, otherwise the active tool.
Options also become that tool's defaults for the next mark, retained during the
session. Selecting another tool brings back its own settings. Every change to an
existing mark is undoable. Backdrop and Image tools temporarily occupy the same
sidebar; choosing a drawing tool restores its options.

Numeric fields accept exact values (Enter or leaving the field applies; Escape
cancels) and step arrows. Color presets and custom RGB pickers preserve opacity;
**Pick from screen** opens the eyedropper. See [usage](docs/usage.md) for selection
and editing shortcuts.

| Tool | Sidebar options | Canvas behavior |
| --- | --- | --- |
| Select | Selected marks' options; duplicate, delete, nudge | Click or box-select; Shift adds/toggles; drag moves the group |
| Draw / Pen | Color, thickness, opacity, Raw / Smooth / Adaptive cleanup | Raw retains input; Smooth rounds jitter; Adaptive preserves sharp turns. Original points stay editable, so cleanup can be changed later. |
| Line / Arrow | Color, thickness, opacity, solid / dashed / dotted, independent start/end (none, arrow, dot), add point, straighten | No ends makes a plain line; both arrows makes a double arrow. Add point creates a draggable waypoint; a two-point arrow retains its bend handle. |
| Box | Color, border thickness, opacity, outline / filled, solid / dashed / dotted border, corner radius | Shift draws a square; filled boxes can be picked by their interior |
| Text | Color, font size, opacity | Native inline single-line editing |
| Highlight | Color, intensity | Transparent rectangular tint; Shift draws a square |
| Pixelate | Block size | Larger blocks obscure more detail; Shift draws a square |
| Crop | Free / 1:1 / 4:3 / 16:9 / 9:16 aspect | Drag applies crop; constrained endpoints remain within the image; undo restores it |
| Step | Color, badge size, number | Sequential by default; adjust selected number or next number |
| Spotlight | Border color, surrounding dim strength | Multiple focus regions share one mask; strongest dim setting wins |
| Magnifier | Border color, lens diameter, 2× / 3× / 4× | Separate source and lens handles |
| Backdrop | Format, padding, corners, shadow, solid / gradient / motion, colors | Frames the image |
| Image tools | Resize, smart upscale, rotation, paste | Resize preserves editable marks; rotation flattens them |
| Animation | Entrance, duration, delay, clip length, Hold / Exit, playback | Preview time scrubs; configure motion in Backdrop |
