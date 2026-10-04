use super::{Editor, preview_base, render_image};
use super::{
    feedback::CopyFeedback,
    jobs::{Message, OperationKind, OperationResult},
};
use crate::{
    animation::Motion,
    backdrop::Backdrop,
    document::{
        Tool,
        actions::{DocumentAction, Selection},
    },
    glance, platform, video,
};
use gpui::Context;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
impl Editor {
    pub(super) fn commit_text(&mut self, cx: &mut Context<Self>) {
        if let Some(edit) = self.interaction.text_edit.take() {
            if !edit.buffer.text().trim().is_empty() {
                let mut mark = edit.mark.clone();
                mark.text = edit.buffer.text().into();
                match self.edit_document(DocumentAction::AddAnnotation { mark }, cx) {
                    Ok(()) => self.feedback.status = "Text label added".into(),
                    Err(error) => {
                        self.interaction.text_edit = Some(edit);
                        self.feedback.status = error;
                    }
                }
            } else {
                self.feedback.status = "Empty label discarded".into();
            }
            cx.notify();
        }
    }
    pub(super) fn capture(&mut self, area: bool, cx: &mut Context<Self>) -> Result<(), String> {
        if self.is_busy() {
            return Err("Editor is busy".into());
        }
        let Some(id) = self.start_operation(OperationKind::Capture) else {
            return Err("Editor is busy".into());
        };
        if let Err(error) = platform::screen_capture_permission() {
            self.receive(
                Message::Operation(id, OperationResult::Image(Err(error.clone()))),
                cx,
            );
            return Err(error);
        }
        self.commit_text(cx);
        self.cancel_gesture();
        self.feedback.status = "Capturing… Escape cancels area selection".into();
        cx.hide();
        self.spawn_operation(id, move || OperationResult::Image(platform::capture(area)));
        cx.notify();
        Ok(())
    }
    pub(super) fn open(&mut self, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        self.commit_text(cx);
        let Some(id) = self.start_operation(OperationKind::Open) else {
            return;
        };
        self.feedback.status = "Choose a PNG or JPEG…".into();
        self.spawn_operation(id, move || OperationResult::Image(platform::open()));
        cx.notify();
    }
    pub(super) fn export(&mut self, save: bool, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        self.commit_text(cx);
        let Some(id) = self.start_operation(if save {
            OperationKind::Save
        } else {
            OperationKind::Copy
        }) else {
            return;
        };
        let document = self.document.render_snapshot();
        let phase = self.animation_phase();
        self.feedback.status = if save {
            "Choose where to save…"
        } else {
            "Copying…"
        }
        .into();
        if !save {
            self.set_copy_feedback(Some(CopyFeedback::Copying), cx);
        }
        self.spawn_operation(id, move || {
            let image = document.export_at(phase);
            if save {
                OperationResult::Saved(platform::save(image))
            } else {
                OperationResult::Copied(platform::copy(image))
            }
        });
        cx.notify();
    }
    pub(super) fn copy_remote(&mut self, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        self.commit_text(cx);
        let Some(id) = self.start_operation(OperationKind::Upload) else {
            return;
        };
        self.feedback.status = "Uploading screenshot to Glance…".into();
        self.set_copy_feedback(Some(CopyFeedback::Uploading), cx);
        let document = self.document.render_snapshot();
        let phase = self.animation_phase();
        self.spawn_operation(id, move || {
            let result = glance::upload(document.export_at(phase));
            OperationResult::RemoteCopied(result)
        });
        cx.notify();
    }
    pub(super) fn history(&mut self, redo: bool, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        self.commit_text(cx);
        self.cancel_gesture();
        let edit = if redo {
            DocumentAction::Redo
        } else {
            DocumentAction::Undo
        };
        if let Err(error) = self.edit_document(edit, cx) {
            self.feedback.status = error;
            cx.notify();
        }
    }
    pub(super) fn set_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        self.commit_text(cx);
        self.cancel_gesture();
        self.interaction.selected = None;
        self.interaction.tool = tool;
        cx.notify();
    }
    pub(super) fn cancel_gesture(&mut self) {
        let gesture = std::mem::take(&mut self.interaction.gesture);
        if gesture.drag().is_some() {
            self.changed();
        }
    }
    pub(super) fn duplicate_selected(&mut self, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        self.cancel_gesture();
        if let Some(index) = self.interaction.selected {
            let mut mark = self.document.marks[index].clone();
            mark.translate(10., 10.);
            if let Err(error) = self.edit_document(DocumentAction::AddAnnotation { mark }, cx) {
                self.feedback.status = error;
            }
            cx.notify();
        }
    }
    pub(super) fn apply_style(&mut self, color: bool, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        self.cancel_gesture();
        if let Some(index) = self.interaction.selected {
            let mut mark = self.document.marks[index].clone();
            if color {
                mark.color = self.interaction.color;
            } else {
                mark.width = self.interaction.width;
            }
            if let Err(error) =
                self.edit_document(DocumentAction::UpdateAnnotation { index, mark }, cx)
            {
                self.feedback.status = error;
            }
        }
        cx.notify();
    }
    pub(super) fn delete_selected(&mut self, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        self.cancel_gesture();
        if let Some(index) = self.interaction.selected {
            if let Err(error) = self.edit_document(DocumentAction::DeleteAnnotation { index }, cx) {
                self.feedback.status = error;
            }
            cx.notify();
        }
    }
    pub(super) fn toggle_backdrop(&mut self, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        self.commit_text(cx);
        self.cancel_gesture();
        self.panels.popup = None;
        self.panels.backdrop = !self.panels.backdrop;
        if self.panels.backdrop {
            self.panels.enhance = false;
        }
        if self.panels.backdrop && self.document.backdrop.is_none() {
            let backdrop = self.panels.backdrop_disabled.take().unwrap_or_default();
            if let Err(error) = self.edit_document(
                DocumentAction::SetBackdrop {
                    backdrop: Some(backdrop),
                },
                cx,
            ) {
                self.feedback.status = error;
                return;
            }
            self.feedback.status = "Backdrop added • style it in the panel • ⌘Z to undo".into();
        }
        cx.notify();
    }
    pub(super) fn backdrop_style(
        &mut self,
        change: impl FnOnce(&mut Backdrop),
        cx: &mut Context<Self>,
    ) {
        if self.is_busy() {
            return;
        }
        self.commit_text(cx);
        self.cancel_gesture();
        let mut backdrop = self
            .document
            .backdrop
            .or(self.panels.backdrop_disabled)
            .unwrap_or_default();
        change(&mut backdrop);
        if let Err(error) = self.edit_document(
            DocumentAction::SetBackdrop {
                backdrop: Some(backdrop),
            },
            cx,
        ) {
            self.feedback.status = error;
        }
        cx.notify();
    }
    pub(super) fn toggle_enhance(&mut self, cx: &mut Context<Self>) {
        self.commit_text(cx);
        self.panels.enhance = !self.panels.enhance;
        if self.panels.enhance {
            self.panels.backdrop = false;
        }
        cx.notify();
    }
    pub(super) fn resize_image(&mut self, rotate: bool, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        self.commit_text(cx);
        self.cancel_gesture();
        let edit = if rotate {
            DocumentAction::Rotate
        } else {
            DocumentAction::Resize {
                scale: self.panels.resize_scale,
                smart: self.panels.resize_smart,
            }
        };
        if let Err(error) = self.edit_document(edit, cx) {
            self.feedback.status = error;
            cx.notify();
        }
    }
    pub(super) fn paste_image(&mut self, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        self.commit_text(cx);
        let Some(id) = self.start_operation(OperationKind::Paste) else {
            return;
        };
        self.spawn_operation(id, move || {
            OperationResult::Image(crate::platform::clipboard_image().map(Some))
        });
        cx.notify();
    }
    pub(super) fn animation_phase(&self) -> f32 {
        let seconds = self.document.backdrop.map_or(5, |b| b.seconds).max(2) as f32;
        let elapsed = if self.playback.paused {
            0.
        } else {
            self.playback.epoch.elapsed().as_secs_f32()
        };
        ((self.playback.position + elapsed) / seconds).rem_euclid(1.)
    }
    pub(super) fn toggle_animation(&mut self, cx: &mut Context<Self>) {
        if self.playback.paused {
            self.playback.epoch = std::time::Instant::now();
            self.playback.paused = false;
        } else {
            self.playback.position += self.playback.epoch.elapsed().as_secs_f32();
            self.playback.paused = true;
        }
        cx.notify();
    }
    pub(super) fn cancel_video(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel) = &self.video_export.cancel {
            cancel.store(true, Ordering::Relaxed);
            self.feedback.status = "Canceling video export…".into();
            cx.notify();
        }
    }
    pub(super) fn export_video(&mut self, cx: &mut Context<Self>) {
        self.export_animation(false, cx);
    }
    pub(super) fn export_gif(&mut self, cx: &mut Context<Self>) {
        self.export_animation(true, cx);
    }
    fn export_animation(&mut self, gif: bool, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        self.commit_text(cx);
        self.cancel_gesture();
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
            self.panels.backdrop = true;
            cx.notify();
            return;
        }
        let document = self.document.render_snapshot();
        let phase = self.animation_phase();
        let cancel = Arc::new(AtomicBool::new(false));
        self.video_export.cancel = Some(cancel.clone());
        self.video_export.progress = Some(0);
        let Some(id) = self.start_operation(OperationKind::Video) else {
            return;
        };
        self.panels.backdrop = true;
        let sender = self.sender.clone();
        cx.notify();
        self.spawn_operation(id, move || {
            let result = crate::platform::animation_destination(gif).and_then(|path| {
                let Some(path) = path else {
                    return Ok(None);
                };
                let progress = |percent| {
                    let _ = sender.try_send(Message::VideoProgress(id, percent));
                };
                let result = if gif {
                    crate::gif_export::encode(&document, &path, phase, &cancel, progress)
                } else {
                    video::encode(&document, &path, phase, &cancel, progress)
                };
                result.map(|finished| finished.then_some(path))
            });
            OperationResult::VideoSaved(result)
        });
    }
    pub(super) fn open_path(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        self.commit_text(cx);
        let Some(id) = self.start_operation(OperationKind::Open) else {
            return;
        };
        self.spawn_operation(id, move || {
            OperationResult::Image(platform::load(&path).map(Some))
        });
        cx.notify();
    }

    pub(super) fn edit_document(
        &mut self,
        edit: DocumentAction,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if matches!(
            edit,
            DocumentAction::Resize { .. } | DocumentAction::Rotate | DocumentAction::Crop { .. }
        ) {
            let crop = matches!(edit, DocumentAction::Crop { .. });
            let id = self
                .start_operation(if crop {
                    OperationKind::Crop
                } else {
                    OperationKind::Transform
                })
                .ok_or("Editor is busy")?;
            if crop {
                self.feedback.status = "Cropping…".into();
            }
            let mut document = self.document.clone();
            self.spawn_operation(id, move || match edit.apply(&mut document) {
                Ok(_) => {
                    let count = document.marks.len();
                    let image = render_image(preview_base(&document));
                    if crop {
                        OperationResult::Cropped(document, image)
                    } else {
                        OperationResult::Transformed(Ok((document, count, image)))
                    }
                }
                Err(error) => OperationResult::Failed(error),
            });
        } else {
            let wait_for_preview = matches!(
                edit,
                DocumentAction::DeleteAnnotation { .. }
                    | DocumentAction::Undo
                    | DocumentAction::Redo
            );
            let backdrop = matches!(edit, DocumentAction::SetBackdrop { .. });
            let previous_motion = self.document.backdrop.map(|b| b.motion);
            let previous_format = self.document.backdrop.map(|b| b.format);
            let outcome = edit.apply(&mut self.document)?;
            match outcome.selection {
                Selection::Keep => {}
                Selection::Clear => self.interaction.selected = None,
                Selection::Select(index) => self.interaction.selected = Some(index),
            }
            if outcome.reset_view {
                self.viewport.zoom = None;
                self.viewport.pan = (0., 0.);
            }
            if outcome.changed {
                if backdrop {
                    if previous_format != self.document.backdrop.map(|b| b.format) {
                        self.viewport.zoom = None;
                        self.viewport.pan = (0., 0.);
                    }
                    if previous_motion != self.document.backdrop.map(|b| b.motion) {
                        self.playback.position = 0.;
                        self.playback.epoch = std::time::Instant::now();
                        self.playback.paused = false;
                    }
                    // Framing is painted separately; invalidate in-flight snapshots
                    // without re-rasterizing the unchanged foreground on each slider tick.
                    self.preview.revision += 1;
                } else {
                    if wait_for_preview {
                        self.preview.mark_count = usize::MAX;
                        self.preview.waiting = true;
                    }
                    self.changed();
                }
            }
        }
        cx.notify();
        Ok(())
    }
}
