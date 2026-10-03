use super::*;
impl Editor {
    pub(super) fn commit_text(&mut self, cx: &mut Context<Self>) {
        if let Some(edit) = self.text_edit.take() {
            if !edit.buffer.text().trim().is_empty() {
                let mut mark = edit.mark;
                mark.text = edit.buffer.text().into();
                self.document.commit(mark);
                self.selected = self.document.marks.len().checked_sub(1);
                self.changed();
                self.status = "Text label added".into();
            } else {
                self.status = "Empty label discarded".into();
            }
            cx.notify();
        }
    }
    pub(super) fn capture(&mut self, area: bool, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if let Err(error) = platform::screen_capture_permission() {
            self.receive(Message::Image(Err(error)), cx);
            return;
        }
        self.commit_text(cx);
        self.draft = None;
        self.busy = true;
        self.status = "Capturing… Escape cancels area selection".into();
        cx.hide();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let _ = sender.send_blocking(Message::Image(platform::capture(area)));
        });
        cx.notify();
    }
    pub(super) fn open(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.commit_text(cx);
        self.busy = true;
        self.status = "Choose a PNG or JPEG…".into();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let _ = sender.send_blocking(Message::Image(platform::open()));
        });
        cx.notify();
    }
    pub(super) fn export(&mut self, save: bool, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.commit_text(cx);
        self.busy = true;
        let document = self.document.render_snapshot();
        let phase = self.animation_phase();
        let sender = self.sender.clone();
        self.status = if save {
            "Choose where to save…"
        } else {
            "Copying…"
        }
        .into();
        if !save {
            self.set_copy_feedback(Some(CopyFeedback::Copying), cx);
        }
        std::thread::spawn(move || {
            let image = document.export_at(phase);
            let message = if save {
                Message::Saved(platform::save(image))
            } else {
                Message::Copied(platform::copy(image))
            };
            let _ = sender.send_blocking(message);
        });
        cx.notify();
    }
    pub(super) fn copy_remote(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.commit_text(cx);
        self.busy = true;
        self.status = "Uploading screenshot to Glance…".into();
        self.set_copy_feedback(Some(CopyFeedback::Uploading), cx);
        let document = self.document.render_snapshot();
        let phase = self.animation_phase();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = glance::upload(document.export_at(phase));
            let _ = sender.send_blocking(Message::RemoteCopied(result));
        });
        cx.notify();
    }
    pub(super) fn history(&mut self, redo: bool, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.commit_text(cx);
        self.draft = None;
        self.selected = None;
        self.object_drag = None;
        if redo {
            self.document.redo();
        } else {
            self.document.undo();
        }
        self.preview_count = usize::MAX;
        self.waiting_preview = true;
        self.busy = true;
        self.status = "Updating image…".into();
        self.changed();
        cx.notify();
    }
    pub(super) fn set_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        self.commit_text(cx);
        self.cancel_move();
        self.selected = None;
        self.tool = tool;
        self.draft = None;
        cx.notify();
    }
    pub(super) fn cancel_move(&mut self) {
        if self.object_drag.take().is_some() {
            self.changed();
        }
    }
    pub(super) fn duplicate_selected(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.cancel_move();
        if let Some(index) = self.selected {
            let mut mark = self.document.marks[index].clone();
            mark.translate(10., 10.);
            self.document.commit(mark);
            self.selected = Some(self.document.marks.len() - 1);
            self.changed();
            cx.notify();
        }
    }
    pub(super) fn apply_style(&mut self, color: bool, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.cancel_move();
        if let Some(index) = self.selected {
            let m = &self.document.marks[index];
            if (color && m.color != self.color) || (!color && m.width != self.width) {
                self.document.remember();
                let m = &mut self.document.marks[index];
                if color {
                    m.color = self.color;
                } else {
                    m.width = self.width;
                }
                self.changed();
            }
        }
        cx.notify();
    }
    pub(super) fn delete_selected(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.cancel_move();
        if let Some(index) = self.selected.take() {
            self.document.delete_mark(index);
            self.preview_count = usize::MAX;
            self.waiting_preview = true;
            self.busy = true;
            self.changed();
            cx.notify();
        }
    }
    pub(super) fn toggle_backdrop(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.commit_text(cx);
        self.draft = None;
        self.backdrop_panel = !self.backdrop_panel;
        if self.backdrop_panel {
            self.enhance_panel = false;
        }
        if self.backdrop_panel && self.document.backdrop.is_none() {
            self.document.remember();
            self.document.backdrop = Some(Backdrop::default());
            self.status = "Backdrop added • style it in the panel • ⌘Z to undo".into();
        }
        cx.notify();
    }
    pub(super) fn backdrop_style(
        &mut self,
        change: impl FnOnce(&mut Backdrop),
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        if let Some(mut b) = self.document.backdrop {
            change(&mut b);
            if self
                .document
                .backdrop
                .is_some_and(|old| old.motion != b.motion)
            {
                self.animation_position = 0.;
                self.animation_epoch = std::time::Instant::now();
                self.animation_paused = false;
            }
            if self.document.backdrop != Some(b) {
                self.document.remember();
                self.document.backdrop = Some(b);
            }
        } else {
            let mut b = Backdrop::default();
            change(&mut b);
            self.document.remember();
            self.document.backdrop = Some(b);
        }
        cx.notify();
    }
    pub(super) fn toggle_enhance(&mut self, cx: &mut Context<Self>) {
        self.commit_text(cx);
        self.enhance_panel = !self.enhance_panel;
        if self.enhance_panel {
            self.backdrop_panel = false;
        }
        cx.notify();
    }
    pub(super) fn resize_image(&mut self, rotate: bool, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.commit_text(cx);
        self.draft = None;
        self.busy = true;
        let mut document = self.document.clone();
        let scale = self.resize_scale;
        let smart = self.resize_smart;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = if rotate {
                document.rotate();
                Ok(())
            } else {
                document.resize(scale, smart)
            };
            let result = result.map(|()| {
                let count = document.marks.len();
                let preview = render_image(preview_base(&document));
                (document, count, preview)
            });
            let _ = sender.send_blocking(Message::Transformed(result));
        });
        cx.notify();
    }
    pub(super) fn paste_image(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.commit_text(cx);
        self.busy = true;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let _ =
                sender.send_blocking(Message::Image(crate::platform::clipboard_image().map(Some)));
        });
        cx.notify();
    }
    pub(super) fn animation_phase(&self) -> f32 {
        let seconds = self.document.backdrop.map_or(5, |b| b.seconds).max(2) as f32;
        let elapsed = if self.animation_paused {
            0.
        } else {
            self.animation_epoch.elapsed().as_secs_f32()
        };
        ((self.animation_position + elapsed) / seconds).rem_euclid(1.)
    }
    pub(super) fn toggle_animation(&mut self, cx: &mut Context<Self>) {
        if self.animation_paused {
            self.animation_epoch = std::time::Instant::now();
            self.animation_paused = false;
        } else {
            self.animation_position += self.animation_epoch.elapsed().as_secs_f32();
            self.animation_paused = true;
        }
        cx.notify();
    }
    pub(super) fn cancel_video(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel) = &self.video_cancel {
            cancel.store(true, Ordering::Relaxed);
            self.status = "Canceling video export…".into();
            cx.notify();
        }
    }
    pub(super) fn export_video(&mut self, cx: &mut Context<Self>) {
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
                video::encode(&document, &path, phase, &cancel, |percent| {
                    let _ = sender.try_send(Message::VideoProgress(percent));
                })
                .map(|finished| finished.then_some(path))
            });
            let _ = sender.send_blocking(Message::VideoSaved(result));
        });
    }
}
