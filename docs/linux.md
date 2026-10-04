# Glance on Omarchy

Glance has experimental support for Omarchy’s Arch Linux / Hyprland desktop.
The native GPUI window uses Vulkan. Capture uses `grim` and `slurp`, PNG
clipboard operations use `wl-clipboard`, file dialogs use `zenity`, and video
exports/frame extraction use your system FFmpeg. Linux annotation and UI fonts
use DejaVu Sans. Remote sharing remains optional, as on macOS.

## Install the CI package

Open a successful [CI run](https://github.com/modem-dev/glance-desktop/actions/workflows/ci.yml)
and download `glance-omarchy-x86_64` from its Artifacts section. Extract the
Actions ZIP. Verify the package checksum and install it:

```sh
sha256sum -c arch-package.sha256
sudo pacman -U ./glance-desktop-*.pkg.tar.zst
glance
```

Pacman installs the declared dependencies. A working Vulkan GPU driver is also
required; keep the driver appropriate to your hardware. CI artifacts are
short-lived development snapshots; they are not published releases or an AUR
package. The package places Glance and its helpers in `/usr/lib/glance/bin`,
links `/usr/bin/glance`, and installs the launcher/icon/license notices.

## Capture from Hyprland

Launch with `glance --capture-area` to select a region, or
`glance --capture-screen` to capture all displays. Escape cancels a region
selection without opening a new editor. These commands capture before creating
the window. Each launch opens a separate editor; save or copy before closing.

Optional bindings for `~/.config/hypr/bindings.conf`:

```ini
bindd = SUPER ALT, 2, Capture area in Glance, exec, glance --capture-area
bindd = SUPER ALT, 3, Capture displays in Glance, exec, glance --capture-screen
```

Reload with `hyprctl reload`. Check for existing bindings before adding these;
Glance’s installer does not change Omarchy’s defaults. To open an existing
image, use `glance --open /path/to/image.png` or the editor’s Open button.

Use **Ctrl** in place of **⌘** for editing: Ctrl+C copies an image locally,
Ctrl+Shift+C uploads a temporary remote link, Ctrl+S saves PNG, and Ctrl+Z undoes.
Ctrl+Alt+2/3 captures while the editor is focused. The toolbar requests that the
compositor minimize the editor before capture; Hyprland may ignore this request.
Use the startup capture commands when the editor must stay out of the shot.

## Build from source

Install stable Rust through rustup, or install Arch’s Rust packages alongside
the build/runtime dependencies:

```sh
sudo pacman -S --needed base-devel rust clang cmake \
  fontconfig freetype2 libx11 libxcb libxkbcommon libxkbcommon-x11 \
  wayland vulkan-icd-loader xdg-utils ttf-dejavu grim slurp wl-clipboard zenity ffmpeg
git clone https://github.com/modem-dev/glance-desktop.git
cd glance-desktop
./scripts/package-linux.sh
cd target/dist
makepkg
sudo pacman -U ./glance-desktop-*.pkg.tar.zst
```

`makepkg` runs as your normal user. The generated PKGBUILD installs the archive
just built, verifies its SHA-256, and preserves both video helpers next to the
executable. Local packaging follows the host architecture; CI targets x86_64.

For iteration, `cargo run --locked` opens the practice canvas. Copy
`native/linux/glance-video-*` to `target/` once to enable video in Cargo builds.
Run the same formatting, Clippy, and test commands as macOS contributors.
GPUI 0.2.2’s legacy xattr dependency currently requires a pinned libc version;
remove the compatibility pin when upgrading that upstream dependency.

## Acceptance checks and limits

CI checks the Linux build, virtual editor interactions, text export, actual
FFmpeg encode/decode, and the installed Arch package. It cannot verify a real
Hyprland desktop. Before calling this port stable, check area cancellation,
multi-monitor/scaled capture, clipboard ownership after Glance closes, open/save
and overwrite dialogs, text/IME, GPU startup, window close, and the optional
bindings on an Omarchy machine. Check PNG/GIF/MP4 output visually too.

Animated backdrops use the CPU fallback on Linux, so preview/export can be slower
than Metal on macOS. Pinch gestures are macOS-only. Capture requires a Wayland
compositor supporting grim’s capture protocol; X11, GNOME, and other Linux
sessions have not been validated. The editor does not persist editable sessions.
