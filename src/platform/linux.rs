//! Wayland integration for Omarchy/Hyprland.
use super::*;
use std::{
    io::{Read, Write},
    process::Stdio,
};
pub fn permission() -> Result<(), String> {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return Err(
            "Screen capture requires a Wayland session (Omarchy/Hyprland) and grim/slurp.".into(),
        );
    }
    Ok(())
}
pub fn capture(area: bool) -> Result<Option<RgbaImage>, String> {
    let geometry = if area {
        let selection = Command::new("slurp")
            .output()
            .map_err(|e| format!("Cannot run slurp: {e}. Install grim and slurp."))?;
        if !selection.status.success() {
            return match capture_failure(true, &selection.stderr, selection.status.code()) {
                Some(e) => Err(e),
                None => Ok(None),
            };
        }
        let geometry = String::from_utf8(selection.stdout).map_err(|e| e.to_string())?;
        if geometry.trim().is_empty() {
            return Ok(None);
        }
        Some(geometry)
    } else {
        None
    };
    let path = std::env::temp_dir().join(format!(
        "glance-{}-{}.png",
        std::process::id(),
        NEXT_CAPTURE.fetch_add(1, Ordering::Relaxed)
    ));
    // Allow the compositor to process the editor's minimize request.
    std::thread::sleep(std::time::Duration::from_millis(250));
    let mut command = Command::new("grim");
    if let Some(geometry) = geometry {
        command.args(["-g", geometry.trim()]);
    }
    let output = command
        .arg(&path)
        .output()
        .map_err(|e| format!("Cannot run grim: {e}. Install grim and slurp."));
    let result = match output {
        Ok(output) if output.status.success() && path.exists() => load(&path).map(Some),
        Ok(output) => Err(capture_failure(false, &output.stderr, output.status.code()).unwrap()),
        Err(error) => Err(error),
    };
    let _ = std::fs::remove_file(path);
    result
}
fn dialog(args: &[&str]) -> Result<Option<PathBuf>, String> {
    let output = Command::new("zenity")
        .arg("--file-selection")
        .args(args)
        .output()
        .map_err(|e| format!("Cannot open the file dialog: {e}. Install zenity."))?;
    if output.status.code() == Some(1) {
        return Ok(None);
    }
    if !output.status.success() {
        return Err(format!(
            "File dialog failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let path = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    if path.trim_end().is_empty() {
        return Ok(None);
    }
    Ok(Some(PathBuf::from(path.trim_end())))
}
pub fn open() -> Result<Option<RgbaImage>, String> {
    dialog(&[
        "--title=Open an image in Glance",
        "--file-filter=Images | *.png *.jpg *.jpeg *.webp",
    ])?
    .map(|path| load(&path))
    .transpose()
}
pub fn destination(format: ExportFormat) -> Result<Option<PathBuf>, String> {
    let extension = match format {
        ExportFormat::Png => "png",
        ExportFormat::Gif => "gif",
        ExportFormat::Mp4 => "mp4",
    };
    let stamp = Command::new("date")
        .arg("+%Y-%m-%d at %H.%M.%S")
        .output()
        .map_err(|e| e.to_string())?;
    let filename = format!(
        "--filename=Screenshot {}.{extension}",
        String::from_utf8_lossy(&stamp.stdout).trim()
    );
    let Some(mut path) = dialog(&[
        "--save",
        "--confirm-overwrite",
        "--title=Export from Glance",
        &filename,
    ])?
    else {
        return Ok(None);
    };
    // Preserve the chosen path: changing an extension after the dialog
    // would bypass Zenity's overwrite confirmation for the actual target.
    if path.extension().is_none() {
        path.set_extension(extension);
        if path.exists() {
            return Err("That filename already exists; choose the full filename in the save dialog to confirm replacement.".into());
        }
    } else if path.extension().is_none_or(|ext| ext != extension) {
        return Err(format!("Choose a filename ending in .{extension}"));
    }
    Ok(Some(path))
}
pub fn copy(image: RgbaImage) -> Result<(), String> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    // wl-copy forks to keep owning the selection after this worker exits.
    let mut child = Command::new("wl-copy")
        .args(["--type", "image/png"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Cannot copy an image: {e}. Install wl-clipboard."))?;
    let written = child
        .stdin
        .take()
        .ok_or("Missing clipboard input")?
        .write_all(bytes.get_ref());
    let status = child.wait().map_err(|e| e.to_string())?;
    written.map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("wl-copy failed; check your Wayland session.".into())
    }
}
pub fn clipboard_image() -> Result<RgbaImage, String> {
    let mut child = Command::new("wl-paste")
        .args(["--no-newline", "--type", "image/png"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Cannot paste an image: {e}. Install wl-clipboard."))?;
    let mut data = Vec::new();
    let result = child
        .stdout
        .take()
        .ok_or("Missing clipboard output")?
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut data);
    if result.is_err() || data.len() > 64 * 1024 * 1024 {
        let _ = child.kill();
        let _ = child.wait();
        return Err("Clipboard image could not be read or is too large.".into());
    }
    if !child.wait().map_err(|e| e.to_string())?.success() {
        return Err("The clipboard doesn’t contain a PNG image.".into());
    }
    let mut reader = ImageReader::new(std::io::Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16000);
    limits.max_image_height = Some(16000);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map(|i| i.to_rgba8())
        .map_err(|e| e.to_string())
}
