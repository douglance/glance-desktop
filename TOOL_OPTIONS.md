# Tool options map

The right sidebar follows the selected annotation, otherwise the active tool.
Options also become that tool's defaults for the next mark, retained during the
session. Selecting another tool brings back its own settings. Every change to an
existing mark is undoable. Backdrop and Image tools temporarily occupy the same
sidebar; choosing a drawing tool restores its options.

| Tool | Sidebar options | Canvas behavior |
| --- | --- | --- |
| Select | Selected object's options; duplicate, delete, nudge | Pick topmost object, drag to move |
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
| Backdrop | Existing format, padding, shadow, corners, fill / gradient / motion | Existing sidebar preserved |
| Image tools | Existing resize, smart upscale, rotation, paste | Existing sidebar preserved |

For a later pass: separate fill/border colors, ellipse shapes, text font/weight,
blur redaction, object layering and persistent defaults. These are not inactive
controls in this sidebar.

Common shape controls include fill, stroke weight/style and transparency. See
[Shottr's styling requests](https://www.shottr.cc/request/) and
[Savvyshot's fill styles](https://www.savvyshot.app/docs/editing/annotations).
