use image::{ImageReader, RgbaImage};
use std::{
    borrow::Cow,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT_CAPTURE: AtomicU64 = AtomicU64::new(0);
pub fn load(path: &std::path::Path) -> Result<RgbaImage, String> {
    let reader = ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16000);
    limits.max_image_height = Some(16000);
    limits.max_alloc = Some(512 * 1024 * 1024);
    let mut reader = reader;
    reader.limits(limits);
    reader
        .decode()
        .map(|i| i.to_rgba8())
        .map_err(|e| e.to_string())
}
#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}
/// Called only when the user requests capture, before hiding the editor.
pub fn screen_capture_permission() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    unsafe {
        if CGPreflightScreenCaptureAccess() {
            return Ok(());
        }
        // Let macOS present its normal permission request for a first-time grant.
        let _ = CGRequestScreenCaptureAccess();
        if CGPreflightScreenCaptureAccess() {
            return Ok(());
        }
    }
    Err("macOS is not authorizing this Glance build to record the screen. Open System Settings → Privacy & Security → Screen & System Audio Recording. If Glance is already enabled, quit Glance, remove its entry with −, add /Applications/Glance.app with +, enable it, then reopen. Rebuilding an ad-hoc signed app can invalidate an older permission.".into())
}
fn capture_failure(area: bool, stderr: &[u8], code: Option<i32>) -> Option<String> {
    let detail = String::from_utf8_lossy(stderr);
    let detail = detail.trim();
    if area && detail.is_empty() && matches!(code, Some(0 | 1)) {
        return None;
    }
    Some(if detail.is_empty() {
        format!(
            "Screen capture produced no image (exit {}). Try again with the screen unlocked.",
            code.map_or("unknown".into(), |c| c.to_string())
        )
    } else {
        format!("Screen capture failed: {detail}")
    })
}
pub fn capture(area: bool) -> Result<Option<RgbaImage>, String> {
    // Unique paths avoid mistaking a canceled selection for a previous capture.
    let path = std::env::temp_dir().join(format!(
        "glance-{}-{}.png",
        std::process::id(),
        NEXT_CAPTURE.fetch_add(1, Ordering::Relaxed)
    ));
    std::thread::sleep(std::time::Duration::from_millis(250));
    let mut command = Command::new("/usr/sbin/screencapture");
    command.args(["-x", "-t", "png"]);
    if area {
        command.args(["-i", "-s"]);
    } else {
        command.arg("-m");
    }
    let output = command.arg(&path).output().map_err(|e| e.to_string())?;
    let result = if path.exists() {
        load(&path).map(Some)
    } else if let Some(error) = capture_failure(area, &output.stderr, output.status.code()) {
        Err(error)
    } else {
        Ok(None)
    };
    let _ = std::fs::remove_file(path);
    result
}
fn dialog(script: &str) -> Result<Option<PathBuf>, String> {
    let output = Command::new("/usr/bin/osascript")
        .args(["-e", script])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return if error.contains("-128") {
            Ok(None)
        } else {
            Err(error.trim().to_string())
        };
    }
    let path = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    Ok(Some(PathBuf::from(path.trim_end())))
}
pub fn open() -> Result<Option<RgbaImage>, String> {
    match dialog(
        "POSIX path of (choose file with prompt \"Open an image in Glance\" of type {\"public.png\", \"public.jpeg\"})",
    )? {
        Some(path) => load(&path).map(Some),
        None => Ok(None),
    }
}
pub fn save(image: RgbaImage) -> Result<Option<PathBuf>, String> {
    let Some(mut path) = dialog(
        "POSIX path of (choose file name with prompt \"Save annotated screenshot\" default name \"Glance.png\")",
    )?
    else {
        return Ok(None);
    };
    if path.extension().is_none() {
        path.set_extension("png");
    }
    image
        .save_with_format(&path, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(Some(path))
}
pub fn copy(image: RgbaImage) -> Result<(), String> {
    arboard::Clipboard::new()
        .map_err(|e| e.to_string())?
        .set_image(arboard::ImageData {
            width: image.width() as usize,
            height: image.height() as usize,
            bytes: Cow::Owned(image.into_raw()),
        })
        .map_err(|e| e.to_string())
}

pub fn clipboard_image() -> Result<RgbaImage, String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    let data = clipboard
        .get_image()
        .map_err(|_| "The clipboard doesn’t contain an image.".to_string())?;
    if data.width > 16000
        || data.height > 16000
        || data.width.saturating_mul(data.height) > 64_000_000
    {
        return Err("Clipboard image is too large.".into());
    }
    RgbaImage::from_raw(
        data.width as u32,
        data.height as u32,
        data.bytes.into_owned(),
    )
    .ok_or_else(|| "Invalid clipboard image.".into())
}

pub fn animation_destination(gif: bool) -> Result<Option<PathBuf>, String> {
    let extension = if gif { "gif" } else { "mp4" };
    let script = format!(
        "POSIX path of (choose file name with prompt \"Save animated backdrop\" default name \"Glance.{extension}\")"
    );
    let Some(mut path) = dialog(&script)? else {
        return Ok(None);
    };
    path.set_extension(extension);
    Ok(Some(path))
}
#[cfg(test)]
mod tests {
    use super::capture_failure;
    #[test]
    fn capture_errors_preserve_real_cause_and_cancellation_is_quiet() {
        assert!(capture_failure(true, b"", Some(1)).is_none());
        assert!(capture_failure(true, b" \n", Some(0)).is_none());
        assert!(
            capture_failure(false, b"", Some(1))
                .unwrap()
                .contains("produced no image")
        );
        assert!(
            capture_failure(true, b"could not create image from display", Some(1))
                .unwrap()
                .contains("could not create image from display")
        );
        assert!(capture_failure(true, b"", None).is_some());
    }
}
