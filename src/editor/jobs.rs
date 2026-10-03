use super::*;
pub(crate) enum Message {
    Magnify(f32, (f32, f32), bool),
    Hotkey(bool),
    Preview(u64, usize, Arc<RenderImage>),
    Cropped(Document, Arc<RenderImage>),
    Image(Result<Option<image::RgbaImage>, String>),
    VideoProgress(u32),
    VideoSaved(Result<Option<std::path::PathBuf>, String>),
    Saved(Result<Option<std::path::PathBuf>, String>),
    Copied(Result<(), String>),
    RemoteCopied(Result<glance::Share, String>),
    Transformed(Result<(Document, usize, Arc<RenderImage>), String>),
}
impl Editor {
    pub(super) fn schedule_preview(&mut self) {
        if self.preview.rendering {
            return;
        }
        self.preview.rendering = true;
        let mut document = self.document.render_snapshot();
        if let Some((index, _, _)) = &self.interaction.object_drag {
            document.marks.truncate(*index);
        }
        let revision = self.preview.revision;
        let count = document.marks.len();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let image = render_image(preview_base(&document));
            let _ = sender.send_blocking(Message::Preview(revision, count, image));
        });
    }
    pub(super) fn changed(&mut self) {
        self.preview.revision += 1;
        self.schedule_preview();
    }
    pub(super) fn receive(&mut self, message: Message, cx: &mut Context<Self>) {
        let failed = matches!(
            &message,
            Message::Image(Err(_))
                | Message::VideoSaved(Err(_))
                | Message::Saved(Err(_))
                | Message::Copied(Err(_))
                | Message::RemoteCopied(Err(_))
                | Message::Transformed(Err(_))
        );
        match message {
            Message::Magnify(delta, position, smart) => {
                if self.busy
                    || self.interaction.draft.is_some()
                    || self.interaction.object_drag.is_some()
                    || self.viewport.pan_start.is_some()
                {
                    return;
                }
                if smart {
                    if self.viewport.zoom.is_some_and(|z| z >= 1.) {
                        self.viewport.zoom = None;
                        self.viewport.pan = (0., 0.);
                    } else {
                        self.zoom_at(
                            1. / self
                                .viewport
                                .zoom
                                .unwrap_or(self.viewport.layout.get().scale)
                                .max(0.01),
                            position,
                            cx,
                        );
                    }
                } else {
                    self.zoom_at(1. + delta, position, cx);
                }
                cx.notify();
                return;
            }
            Message::Transformed(Ok((document, count, image))) => {
                self.interaction.selected = None;
                self.interaction.object_drag = None;
                self.document = document;
                self.preview.revision += 1;
                self.preview
                    .retired
                    .push(std::mem::replace(&mut self.preview.image, image));
                self.preview.mark_count = count;
                self.viewport.zoom = None;
                self.viewport.pan = (0., 0.);
                self.busy = false;
            }
            Message::Transformed(Err(e)) => {
                self.busy = false;
                self.feedback.status = e;
            }
            Message::Hotkey(area) => {
                self.capture(area, cx);
                return;
            }
            Message::Preview(revision, count, image) => {
                self.preview.rendering = false;
                if revision == self.preview.revision {
                    self.preview
                        .retired
                        .push(std::mem::replace(&mut self.preview.image, image));
                    self.preview.mark_count = count;
                    if self.preview.waiting {
                        self.preview.waiting = false;
                        self.busy = false;
                        self.feedback.status = "Ready".into();
                    }
                } else {
                    self.schedule_preview();
                }
                cx.notify();
                return;
            }
            Message::Cropped(document, image) => {
                let count = document.marks.len();
                self.interaction.selected = None;
                self.interaction.object_drag = None;
                self.document = document;
                self.preview.revision += 1;
                self.preview
                    .retired
                    .push(std::mem::replace(&mut self.preview.image, image));
                self.preview.mark_count = count;
                self.viewport.zoom = None;
                self.viewport.pan = (0., 0.);
                self.busy = false;
                self.feedback.status = "Cropped • ⌘Z to restore".into();
            }
            Message::Image(Ok(Some(image))) => {
                self.interaction.selected = None;
                self.interaction.object_drag = None;
                self.document = Document::new(image);
                self.viewport.zoom = None;
                self.viewport.pan = (0., 0.);
                self.interaction.draft = None;
                self.preview.mark_count = usize::MAX;
                self.preview.waiting = true;
                self.changed();
                self.feedback.status = "Preparing image…".into();
            }
            Message::Image(Ok(None)) => {
                self.busy = false;
                self.feedback.status = "Selection canceled".into();
            }
            Message::Image(Err(e)) => {
                self.busy = false;
                self.feedback.status = e;
            }
            Message::VideoProgress(percent) => {
                self.video_export.progress = Some(percent);
                cx.notify();
                return;
            }
            Message::VideoSaved(result) => {
                self.busy = false;
                self.video_export.progress = None;
                self.video_export.cancel = None;
                self.feedback.status = match result {
                    Ok(Some(path)) => {
                        self.video_export.last_video = Some(path.clone());
                        format!("Video saved to {}", path.display())
                    }
                    Ok(None) => "Video export canceled".into(),
                    Err(e) => e,
                };
            }
            Message::Saved(result) => {
                self.busy = false;
                self.feedback.status = match result {
                    Ok(Some(path)) => format!("Saved {}", path.display()),
                    Ok(None) => "Save canceled".into(),
                    Err(e) => e,
                };
            }
            Message::Copied(result) => {
                self.busy = false;
                self.set_copy_feedback(result.is_ok().then_some(CopyFeedback::Copied), cx);
                self.feedback.status = match result {
                    Ok(()) => "Copied image to clipboard".into(),
                    Err(e) => e,
                };
            }
            Message::RemoteCopied(result) => {
                self.busy = false;
                self.feedback.status = match result {
                    Ok(share) => {
                        cx.write_to_clipboard(ClipboardItem::new_string(format!(
                            "Screenshot: {}",
                            share.url
                        )));
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        let minutes = share.expires_at.saturating_sub(now).div_ceil(60_000);
                        self.set_copy_feedback(Some(CopyFeedback::LinkCopied(minutes)), cx);
                        format!(
                            "Glance link copied • expires in {minutes} min • Paste into your agent’s chat"
                        )
                    }
                    Err(e) => {
                        self.set_copy_feedback(None, cx);
                        e
                    }
                };
            }
        }
        cx.activate(true);
        if failed && let Some(window) = cx.windows().first().copied() {
            let detail = self.feedback.status.clone();
            if let Ok(answer) = window.update(cx, |_, window, cx| {
                window.prompt(
                    PromptLevel::Critical,
                    "Pachiri couldn’t complete the operation",
                    Some(&detail),
                    &["OK"],
                    cx,
                )
            }) {
                cx.spawn(async move |_, _| {
                    let _ = answer.await;
                })
                .detach();
            }
        }
        cx.notify();
    }
}
