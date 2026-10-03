//! Bounded, streaming video export using the bundled AVFoundation encoder.
use crate::{
    Editor, Message,
    animation::{Motion, Renderer},
    document::Document,
};
use gpui::Context;
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
impl Editor {
    pub fn animation_phase(&self) -> f32 {
        let seconds = self.document.backdrop.map_or(5, |b| b.seconds).max(2) as f32;
        let elapsed = if self.animation_paused {
            0.
        } else {
            self.animation_epoch.elapsed().as_secs_f32()
        };
        ((self.animation_position + elapsed) / seconds).rem_euclid(1.)
    }
    pub fn toggle_animation(&mut self, cx: &mut Context<Self>) {
        if self.animation_paused {
            self.animation_epoch = std::time::Instant::now();
            self.animation_paused = false;
        } else {
            self.animation_position += self.animation_epoch.elapsed().as_secs_f32();
            self.animation_paused = true;
        }
        cx.notify();
    }
    pub fn cancel_video(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel) = &self.video_cancel {
            cancel.store(true, Ordering::Relaxed);
            self.status = "Canceling video export…".into();
            cx.notify();
        }
    }
    pub fn export_video(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.commit_text(cx);
        self.cancel_move();
        self.draft = None;
        if !self
            .document
            .backdrop
            .is_some_and(|b| b.motion != Motion::Still)
        {
            self.toggle_backdrop(cx);
            self.backdrop_style(
                |b| {
                    b.motion = Motion::Flow;
                    b.gradient = true;
                },
                cx,
            );
            self.backdrop_panel = true;
            cx.notify();
            return;
        }
        let document = self.document.render_snapshot();
        let phase = self.animation_phase();
        let cancel = Arc::new(AtomicBool::new(false));
        self.video_cancel = Some(cancel.clone());
        self.video_progress = Some(0);
        self.busy = true;
        self.backdrop_panel = true;
        let sender = self.sender.clone();
        cx.notify();
        std::thread::spawn(move || {
            let result = crate::platform::video_destination().and_then(|path| {
                let Some(path) = path else {
                    return Ok(None);
                };
                encode(&document, &path, phase, &cancel, |percent| {
                    let _ = sender.try_send(Message::VideoProgress(percent));
                })
                .map(|finished| finished.then_some(path))
            });
            let _ = sender.send_blocking(Message::VideoSaved(result));
        });
    }
}
struct Encoder(Child);
impl Drop for Encoder {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn encoder_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("App directory unavailable")?;
    for path in [
        dir.join("pachiri-video-encoder"),
        dir.join("../pachiri-video-encoder"),
        dir.join("../../pachiri-video-encoder"),
    ] {
        if path.is_file() {
            return Ok(path);
        }
    }
    Err("Video encoder unavailable. Build the app with scripts/bundle.sh.".into())
}
pub fn encode(
    document: &Document,
    path: &Path,
    start_phase: f32,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u32),
) -> Result<bool, String> {
    let b = document
        .backdrop
        .ok_or("Add a backdrop before exporting video")?;
    if !(2..=15).contains(&b.seconds) {
        return Err("Video duration must be 2–15 seconds".into());
    }
    if cancel.load(Ordering::Relaxed) {
        return Ok(false);
    }
    let native = encoder_path()?;
    let source = document.render(None);
    let renderer = Renderer::new(&source, b, Some(1920));
    let temp = Temporary(path.with_file_name(format!(
            ".pachiri-{}-{}.mp4",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos()
        )));
    let count = b.seconds * 30;
    let mut encoder = Encoder(
        Command::new(native)
            .arg(renderer.width.to_string())
            .arg(renderer.height.to_string())
            .arg("30")
            .arg(count.to_string())
            .arg(&temp.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?,
    );
    let mut input = encoder.0.stdin.take().ok_or("Video input unavailable")?;
    let mut reported = 0;
    for frame in 0..count {
        if cancel.load(Ordering::Relaxed) {
            return Ok(false);
        }
        let mut data = renderer
            .frame(start_phase + frame as f32 / count as f32)
            .into_raw();
        // MP4 has no alpha. Composite rounded output corners onto a soft ivory matte.
        for p in data.as_chunks_mut::<4>().0 {
            let a = p[3] as u16;
            for channel in p.iter_mut().take(3) {
                *channel = ((*channel as u16 * a + 246 * (255 - a) + 127) / 255) as u8;
            }
            p.swap(0, 2);
            p[3] = 255;
        }
        if let Err(error) = input.write_all(&data) {
            drop(input);
            encoder.0.wait().map_err(|e| e.to_string())?;
            let output = read_error(&mut encoder.0);
            return Err(if output.is_empty() {
                error.to_string()
            } else {
                output
            });
        }
        let percent = (frame + 1) * 100 / count;
        if percent != reported {
            progress(percent.min(99));
            reported = percent;
        }
    }
    drop(input);
    let status = encoder.0.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(read_error(&mut encoder.0));
    }
    if cancel.load(Ordering::Relaxed) {
        return Ok(false);
    }
    std::fs::rename(&temp.0, path).map_err(|e| e.to_string())?;
    progress(100);
    Ok(true)
}
fn read_error(child: &mut Child) -> String {
    use std::io::Read;
    let mut error = String::new();
    if let Some(mut stderr) = child.stderr.take() {
        let _ = stderr.read_to_string(&mut error);
    }
    if error.trim().is_empty() {
        "Video encoder failed".into()
    } else {
        error.trim().into()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_and_invalid_duration_leave_destination_untouched() {
        let mut d = Document::new(image::RgbaImage::new(10, 10));
        d.backdrop = Some(crate::backdrop::Backdrop::default());
        let path = std::env::temp_dir().join(format!("pachiri-cancel-{}.mp4", std::process::id()));
        std::fs::write(&path, b"existing movie").unwrap();
        assert!(!encode(&d, &path, 0., &AtomicBool::new(true), |_| {}).unwrap());
        d.backdrop.as_mut().unwrap().seconds = 100;
        assert!(encode(&d, &path, 0., &AtomicBool::new(false), |_| {}).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"existing movie");
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    #[ignore = "requires the built native encoder; writes real QA videos"]
    fn native_motion_export_qa() {
        let directory = std::env::var_os("PACHIRI_VIDEO_QA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("pachiri-motion-qa"));
        std::fs::create_dir_all(&directory).unwrap();
        let source = image::imageops::resize(
            &crate::document::demo(),
            480,
            270,
            image::imageops::FilterType::Lanczos3,
        );
        for motion in Motion::EFFECTS {
            let mut d = Document::new(source.clone());
            d.backdrop = Some(crate::backdrop::Backdrop {
                motion,
                padding: 100,
                seconds: if motion == Motion::Stars { 10 } else { 5 },
                preset: if matches!(motion, Motion::Paint | Motion::Lava) {
                    3
                } else {
                    1
                },
                ..Default::default()
            });
            let path = directory.join(format!("{}.mp4", motion.label()));
            let timer = std::time::Instant::now();
            let mut progress = vec![];
            assert!(encode(&d, &path, 0., &AtomicBool::new(false), |p| progress.push(p)).unwrap());
            assert_eq!(progress.last(), Some(&100));
            assert!(progress.windows(2).all(|p| p[0] <= p[1]));
            assert!(std::fs::metadata(&path).unwrap().len() > 1000);
            d.export_at(0.37)
                .save(directory.join(format!("{}.png", motion.label())))
                .unwrap();
            // Cancel after actual frames have streamed; preserve the finished destination.
            let original = std::fs::read(&path).unwrap();
            let cancel = AtomicBool::new(false);
            assert!(
                !encode(&d, &path, 0., &cancel, |p| {
                    if p >= 5 {
                        cancel.store(true, Ordering::Relaxed);
                    }
                })
                .unwrap()
            );
            assert_eq!(std::fs::read(&path).unwrap(), original);
            println!(
                "{}: {} sec video encoded in {:.2}s → {}",
                motion.label(),
                d.backdrop.unwrap().seconds,
                timer.elapsed().as_secs_f32(),
                path.display()
            );
        }
    }
}
