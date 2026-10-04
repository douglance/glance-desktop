# Using Glance

On Omarchy, use Ctrl in place of ⌘ for editor shortcuts. Global capture uses
Hyprland bindings and Linux full-screen capture includes all displays; see the
[Omarchy guide](linux.md). The shortcut examples below describe macOS.


Capture, annotate, frame, and share from one native macOS window.

[Build and install](../README.md#build-and-install) · [Signing and troubleshooting](development.md)

## Capture and edit


- **⌘⌥2** captures an area using the native macOS selector. Escape cancels.
- **⌘⌥3** captures the main display. The app hides itself before capture.
- **V** selects annotation objects. Click a stroke, shape, label, highlight,
  numbered callout or pixelation region; drag to move it, then press Delete or
  Backspace to remove it. Clicking empty canvas or Escape clears selection.
  The topmost matching object wins. Each move/deletion is one undo step.
  Crop and resize preserve editable annotations; rotation still flattens them.
- Mark up with **P** pen, **A** arrow, **R** rectangle, **T** text, **H** highlight,
  **B** pixelation or **X** crop. Drag to crop immediately; undo restores it.
- New annotations remain selected: drag the mark to move it or press Delete.
  Click empty canvas to draw another with the same tool. Selected arrows have
  endpoint handles to reorient them and a middle handle to bend them; Shift
  snaps endpoint drags to 45°. Text editing has a transparent background.
- The right sidebar follows the selected annotation or active tool. Each tool
  has a **?** icon beside its name; hover to see its shortcut and a brief guide.
  For selected annotations, the tooltip also shows the keyboard move shortcuts.
  Each tool remembers its defaults during the session. Change color, thickness,
  opacity, and tool-specific options there; edits to existing annotations support undo.
  Compact fields pair related settings. Click a numeric value to type an exact
  size or percentage; Enter or leaving the field applies it, Escape cancels,
  and the small arrows step the value. Invalid or out-of-range text is rejected.
  Stroke, fill and arrow endpoints show visual samples. The **Points** controls
  add a waypoint or straighten the selected line; hover for each action's name.
  Magnifier diameter and Zoom sit side by side, with labels above their controls.
  Each color-capable tool has six presets and a square custom-color picker with
  wheel, brightness, hex entry, and a **Pick from screen** eyedropper button.
  Custom RGB and sampled colors preserve the current opacity.
  Lines support dashes, dots, independent ends and draggable added points. Boxes
  support outline/fill and rounded corners. Pen cleanup offers Raw, Smooth and
  Adaptive while retaining original samples. See [the full tool map](../TOOL_OPTIONS.md). **⌘D** duplicates the
  selected annotation. **Arrow keys** nudge it 1 pixel; **Shift + arrows** move
  10 pixels. Held key repeats are grouped into one undo step.
- **Shift-drag:** arrows snap to 45° angles; boxes, highlights, pixelation and
  crops become squares; moving an object locks to the dominant axis. Crop
  endpoints snap to image edges within 8 screen pixels.
- **Text:** click the canvas and type directly into the outlined box. Enter or
  clicking outside finishes the label; Escape cancels. Selection, copy/paste,
  text undo/redo and native macOS input methods work while editing. Labels are
  single-line; pasted line breaks become spaces.
- **Backdrop:** open the toolbar panel to frame the image with a solid color or
  gradient. **Format** offers Auto, 1:1, 4:3, 3:2, 16:9, 4:5, 9:16 and named
  YouTube/Shorts/Pinterest presets. Fixed formats expand the background and center
  the full screenshot without cropping or stretching it. **Outside padding** is
  the minimum backdrop space on each side. **Inside padding** adds space within
  the screenshot by repeating its nearest edge pixels (and corner pixels),
  preserving their transparency. Image corners and shadow follow the expanded
  screenshot; the capture and editable annotations keep their original size and
  coordinates. Outside padding, inside padding, image corners and shadow are
  always visible in a 2×2 grid above **Solid / Gradient / Motion**. **Done** closes
  the panel; **Enable backdrop** toggles framing while keeping the current style.
  The eight palette presets stay on one row. Custom colors appear as compact
  swatches; hex values are shown inside the picker. Play/Pause is outlined beside
  the duration slider. Click a color swatch to open a picker with a color wheel, brightness
  slider and a hex field (**#RGB** or **#RRGGBB**, then **Apply** or Enter). Solid
  uses one color; Gradient and Motion use two. The **Pick from screen** button
  opens the native macOS eyedropper (Omarchy requires
  `hyprpicker`). Colors are opaque sRGB. Custom colors survive mode/effect changes;
  choosing a palette preset resets them. Each wheel/brightness gesture commits on
  release as one undo step; uncommitted hex input is discarded when closed.
  Each slider gesture is one undo step. Copy and
  PNG save include the backdrop and inside padding at full resolution. The output
  canvas has square corners.
- **Image entrances:** open **Animation** (play icon) and choose **Diagonal reveal**,
  **Spring pop**, or **3D settle**. The toolbar panel icons are ordered **Backdrop →
  Animation → Image tools**. Animation contains foreground effects and playback;
  configure the background in **Backdrop** and export through the toolbar **Export**
  menu. Set entrance duration, delay, and a 2–15 second
  clip length. **Replay** starts the clip again; **Play/Pause** and the **Preview
  time** slider let you inspect any frame. Under **After entrance**, choose
  **Hold**, or **Exit** for a repeating clip that returns to the empty backdrop. Backdrop
  motion is independent: choose an effect in **Backdrop → Motion**, or keep it still.
  On macOS, foreground reveal and transform sampling runs on Metal in the preview
  and export workers, with a CPU fallback. Effect/timing edits reuse the prepared
  card, and preview quality changes keep the previous frame visible while rendering.
  Entrances also work without a backdrop. The image, annotations, rounded image
  corners, and shadow animate together. Close the panel or click the canvas to
  return to annotation editing. Slider drags are undoable as one step; seeking
  and replay do not change history. MP4/GIF always start at the entrance's
  beginning; PNG/copy capture the inspected frame while this panel is open,
  and the fully revealed image during normal editing.
- **Animated backdrops:** choose **Backdrop → Motion**, then Flow, Nebula,
  Aurora, Contours, Painterly, Prism, Liquid or Lava. Each effect has a small line
  icon beside its name. Screenshot and annotations stay fixed while the
  background moves. Click **Randomize** in Motion to try a new looping variation.
  Each click changes the motion seed and is undoable with **⌘Z**. Colors, framing
  and duration stay as set; the same variation appears in preview and exports.
  Set a **2–15 second** duration (5 seconds by default), pause/play the preview,
  and use the toolbar **Export** menu to save PNG,
  MP4 or GIF. Effect and duration controls appear only in Motion mode. Export streams 30 fps
  H.264 video through macOS AVFoundation, preserving aspect ratio with a maximum
  1920-pixel edge. The bundled encoder needs no FFmpeg installation. Each video
  is one seamless backdrop cycle; duration also controls the preview's cycle speed.
  A progress bar and percentage stay visible at the bottom of the window throughout
  MP4/GIF export, even when the inspector is closed or scrolled. Use its **Cancel export**
  button, **Export → Cancel export**, or Escape to stop it. Transparent pixels use an ivory matte
  in MP4; PNG retains transparency and captures the current animation phase.
  Liquid has flowing ribbons and fine grain; Lava has molten streams that stretch,
  curl, merge and shed small globules; Aurora has rippling light curtains; Contours has terrain lines
  that ripple, swell and curl; Prism has irregular crystal facets with depth,
  sweeping highlights and palette-tinted bands of refracted color;
  Painterly has brush strokes that sweep, bloom and fade. These
  six effects use distinct procedural Metal shaders. Flow keeps a gentle cloud
  gradient, and Nebula has prominent drifting stars over a subtle evolving
  nebula with wispy colored gas, soft filaments and dark dust lanes. Its nebula
  also uses the shader worker and CPU fallback. Each effect starts with a suggested
  palette, and all eight palettes remain available. Shader preview is capped at
  960 pixels and 30 fps, with the same renderer used for full-resolution PNG and
  MP4 export. A CPU fallback preserves the effect when Metal is unavailable.
- **⌘Z / ⌘⇧Z** undo/redo; **⌘C** copies the composed image; **⌘S** saves PNG.
  Copy shows a brief **Copied!** confirmation and a checkmark on its button.
  Save dialogs suggest `Screenshot YYYY-MM-DD at HH.MM.SS.png` in **Pictures**.
  Animated exports suggest `Animated Screenshot YYYY-MM-DD at HH.MM.SS` with
  `.gif` in **Pictures** or `.mp4` in **Movies**. The timestamp is the local
  export time; you can change the name and folder before saving.
- **Copy (remote) · ⌘⇧C** (cloud-upload icon) uploads the composed PNG (including annotations and
  backdrop) to [Glance](https://glance.sh) and copies `Screenshot: <url>`.
  An **Uploading…** indicator stays visible until completion, then **Link copied!**
  confirms success with the link lifetime. The confirmation dismisses after three seconds.
  Paste it into a remote agent’s chat; the agent can fetch the image directly.
  Links expire after about 30 minutes. Uploads use Glance’s client encryption
  and private Blob storage, require internet, and are limited to 15 MB and
  30 uploads/hour per IP. Failed uploads preserve your clipboard. This shares
  a link; it does not automatically push into an agent’s live session.
  Glance API requests identify the app with `X-Glance-Client: glance-desktop` and
  `X-Glance-Client-Version: <app version>`, alongside `User-Agent: Glance/<app version>`.
- **⌘O** opens PNG/JPEG. **⌘1** fits, **⌘0** uses 100%, **⌘+ / ⌘−** zoom.
  Pinch zooms around the pointer (1–800%); two-finger scrolling pans. **⌘ +
  scroll** zooms; **Shift + wheel** pans horizontally. Hold **Space** and drag
  to pan, or right-drag. Hold **Z** and click to zoom in at that spot; Shift-click
  zooms out. Trackpad smart zoom (two-finger double-tap) toggles 100%/fit.
  Escape cancels an unfinished mark. **⌘Q** quits.
- Drag an image file from Finder onto the canvas to open it. This replaces the
  current image, like Open; save or copy your work first.
- The starter image is a practice canvas. Capture or open replaces it.
- **N** places sequential numbered callouts. Undo removes the last step.
- **⌘V** loads an image from the clipboard when you are not editing text.
- **Image tools** opens resize and rotation controls. Choose 50–400% and apply.
  Smart upscale combines Lanczos resampling with bounded adaptive sharpening;
  it is local image processing, not AI super-resolution, and cannot reconstruct
  missing detail. Vector annotations redraw at the target resolution. Resize
  and 90° clockwise rotation support undo; rotation flattens existing marks.
  Backdrop padding stays in physical pixels. Large resizes are limited to
  16,000 pixels per side and 64 megapixels.

Annotations and export use physical image pixels, including Retina captures.
The preview is capped at 1600×1200 for responsiveness; saved/copied images retain
full resolution. Live pen/arrow/box/highlight strokes use GPU overlays;
compositing, crop and export run on workers. Pixelation shows its selected area
while dragging and applies on release. Text uses the macOS Arial font and an
on-canvas editor. Typing and selection render directly through GPUI.
History retains up to 30 states and shares immutable image pixels between
annotations. Cropped images retain separate pixels so crop can be undone. Closing the window hides the app and keeps global shortcuts active.
Click the Dock icon to reopen; ⌘Q quits. Save/copy before replacing the current capture.

## Spotlight, magnifier, and GIF loops


- **S — Spotlight:** drag a focus rectangle. The surrounding image dims; multiple focus windows share one dimming mask in exports. Drag the object to move it, or drag either corner handle to resize it. Undo/Delete work as with other annotations.
- **M — Magnifier:** drag from a detail to where its enlarged lens should appear. The source and lens have separate handles. The sidebar's **2× / 3× / 4×** buttons and **Lens diameter** control change the selected lens or the next lens. It samples the original annotated foreground, so the enlarged detail stays bright even with a spotlight.
- **Export → GIF…** or **File → Export Looping GIF…** exports an infinitely repeating GIF. MP4 export is available in the same menu. GIF uses 20 fps and a maximum edge of 960 pixels; MP4 uses 30 fps and 1920 pixels. Both render one complete cycle, excluding a duplicate endpoint frame. GIF's fixed palette keeps foreground colors stable across frames; rounded corners use the same ivory matte as MP4.
- Spotlight and magnifier remain editable objects and appear in PNG, clipboard, GIF, and MP4 output. Animated backdrops loop while the foreground stays fixed unless an image entrance is selected. MP4 repeats when the player is configured to loop; GIF includes infinite-repeat metadata. Use an exit when you want the entrance to repeat without abruptly resetting a visible image.

## Limits

Glance edits one image at a time. Save or copy before capturing, opening, or
pasting a replacement image; annotations are not persisted across launches.
Crop out sensitive information before sharing. Pixelation is a visual effect,
not a guarantee that information has been removed.

### Export filenames

Save with the matching `.png`, `.gif`, or `.mp4` extension. Glance preserves
that filename so the native dialog's overwrite confirmation applies to the
actual destination. If you omit the extension, Glance appends it only when the
resulting filename does not already exist; otherwise choose the full filename
in the dialog to confirm replacement.

## macOS accessibility

Toolbar buttons, inspector choices, numeric fields, animation/backdrop sliders,
Format/Export menus and color hex values are exposed to macOS accessibility.
Numeric values support exact edits and increment/decrement; color fields accept
`#RGB` or `#RRGGBB` and preserve annotation opacity. These edits use the same
validation and undo behavior as mouse input. The canvas reports its source size
and annotation count; drawing and on-canvas text still require ordinary input.
