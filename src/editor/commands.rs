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
    pub(super) fn pick_backdrop_screen_color(
        &mut self,
        stop: usize,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.pick_screen_color(Some(stop), cx)
    }
    pub(super) fn pick_screen_color(
        &mut self,
        stop: Option<usize>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.commit_text(cx);
        self.panels.sampling_color = None;
        self.panels.sampling_tool_color = false;
        let receiver = platform::sample_screen_color()?;
        let id = self
            .start_operation(OperationKind::ColorSample)
            .ok_or("Editor is busy")?;
        let revision = self.preview.revision;
        let tool = self.options_tool();
        let selected = self.interaction.selected;
        self.feedback.status = "Pick a screen color • Escape to cancel".into();
        let sender = self.sender.clone();
        cx.spawn(async move |_, _| {
            let result = receiver
                .recv()
                .await
                .unwrap_or_else(|_| Err("Screen sampler closed".into()));
            let _ = sender
                .send(Message::Operation(
                    id,
                    if let Some(stop) = stop {
                        OperationResult::ColorSample {
                            stop,
                            revision,
                            result,
                        }
                    } else {
                        OperationResult::ToolColorSample {
                            tool,
                            selected,
                            revision,
                            result,
                        }
                    },
                ))
                .await;
        })
        .detach();
        Ok(())
    }
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
        crate::platform::hide_editor(cx);
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
        let phase = self.export_phase();
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
        let phase = self.export_phase();
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
        let settings = self.interaction.defaults[tool.index()];
        self.interaction.color = settings.color;
        self.interaction.width = settings.width;
        self.panels.backdrop = false;
        self.panels.enhance = false;
        self.panels.animation = false;
        self.panels.popup = None;
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
        self.commit_text(cx);
        self.cancel_gesture();
        let tool = self.options_tool();
        let defaults = &mut self.interaction.defaults[tool.index()];
        if color {
            defaults.color = self.interaction.color;
        } else {
            defaults.width = self.interaction.width;
        }
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
            self.panels.animation = false;
        }
        if self.panels.backdrop && self.document.backdrop.is_none() {
            let mut backdrop = self.panels.backdrop_disabled.take().unwrap_or_default();
            if self.document.image_animation.enabled() {
                backdrop.seconds = self.document.animation_seconds();
            }
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
        if self.document.image_animation.enabled() {
            backdrop.seconds = self.document.animation_seconds();
        }
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
            self.panels.animation = false;
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
        let seconds = self.document.animation_seconds().max(2) as f32;
        let elapsed = if self.playback.paused {
            0.
        } else {
            self.playback.epoch.elapsed().as_secs_f32()
        };
        ((self.playback.position + elapsed) / seconds).rem_euclid(1.)
    }
    pub(super) fn clip_time(&self) -> f32 {
        let elapsed = if self.playback.paused {
            0.
        } else {
            self.playback.epoch.elapsed().as_secs_f32()
        };
        (self.playback.position + elapsed).clamp(0., self.document.animation_seconds() as f32)
    }
    pub(super) fn export_phase(&self) -> f32 {
        if self.document.image_animation.enabled() {
            if self.panels.animation {
                self.clip_time() / self.document.animation_seconds() as f32
            } else {
                let a = self.document.image_animation;
                (a.delay_ms + a.duration_ms) as f32 / 1000. / a.seconds as f32
            }
        } else {
            self.animation_phase()
        }
    }
    pub(super) fn replay_animation(&mut self, cx: &mut Context<Self>) {
        self.playback.position = 0.;
        self.playback.epoch = std::time::Instant::now();
        self.playback.paused = false;
        self.playback.seek = self.playback.seek.wrapping_add(1);
        cx.notify();
    }
    pub(super) fn toggle_animation_panel(&mut self, cx: &mut Context<Self>) {
        self.commit_text(cx);
        self.cancel_gesture();
        self.panels.popup = None;
        self.panels.animation = !self.panels.animation;
        if self.panels.animation {
            self.panels.backdrop = false;
            self.panels.enhance = false;
            self.interaction.selected = None;
            self.replay_animation(cx);
        }
        cx.notify();
    }
    pub(super) fn toggle_animation(&mut self, cx: &mut Context<Self>) {
        if self.panels.animation
            && self.document.image_animation.enabled()
            && self.clip_time() >= self.document.animation_seconds() as f32
        {
            self.replay_animation(cx);
            return;
        }
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
        if !self.document.image_animation.enabled()
            && !self
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
        if self.document.image_animation.enabled() {
            self.panels.animation = true;
            self.panels.backdrop = false;
            self.panels.enhance = false;
        } else {
            self.panels.backdrop = true;
        }
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
            let metadata = matches!(
                edit,
                DocumentAction::SetBackdrop { .. } | DocumentAction::SetImageAnimation { .. }
            );
            let previous_animation = self.document.image_animation;
            let previous_motion = self.document.backdrop.map(|b| b.motion);
            let previous_format = self.document.backdrop.map(|b| b.format);
            let previous_padding = self.document.backdrop.map_or(0, |b| b.inside_padding);
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
                if previous_animation != self.document.image_animation {
                    self.replay_animation(cx);
                }
                if metadata {
                    if previous_format != self.document.backdrop.map(|b| b.format) {
                        self.viewport.zoom = None;
                        self.viewport.pan = (0., 0.);
                    }
                    if previous_motion != self.document.backdrop.map(|b| b.motion) {
                        self.playback.position = 0.;
                        self.playback.epoch = std::time::Instant::now();
                        self.playback.paused = false;
                    }
                    // Background styling is painted separately. Rebuild the foreground
                    // only when edge padding changes; reject all old snapshots.
                    self.preview.revision += 1;
                    if previous_padding != self.document.backdrop.map_or(0, |b| b.inside_padding) {
                        self.schedule_preview();
                    }
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

impl Editor {
    pub(super) fn options_tool(&self) -> Tool {
        self.interaction
            .selected
            .and_then(|i| self.document.marks.get(i))
            .map_or(self.interaction.tool, |m| m.tool)
    }
    pub(super) fn tool_settings(&self) -> super::state::ToolSettings {
        let tool = self.options_tool();
        if let Some(mark) = self
            .interaction
            .selected
            .and_then(|i| self.document.marks.get(i))
        {
            super::state::ToolSettings {
                color: mark.color,
                width: mark.width,
                style: mark.style,
                magnification: crate::effects::zoom(mark),
            }
        } else {
            self.interaction.defaults[tool.index()]
        }
    }
    pub(super) fn set_appearance(
        &mut self,
        style: crate::style::Style,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.commit_text(cx);
        self.cancel_gesture();
        self.interaction.defaults[self.options_tool().index()].style = style;
        if let Some(index) = self.interaction.selected {
            let mut mark = self.document.marks[index].clone();
            mark.style = style;
            self.edit_document(DocumentAction::UpdateAnnotation { index, mark }, cx)?;
        }
        Ok(())
    }
    pub(super) fn set_magnifier_zoom(
        &mut self,
        zoom: f32,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.options_tool() != Tool::Magnifier {
            return Err("Choose a magnifier first".into());
        }
        self.cancel_gesture();
        self.interaction.defaults[Tool::Magnifier.index()].magnification = zoom;
        if let Some(index) = self.interaction.selected {
            let mut mark = self.document.marks[index].clone();
            mark.text = zoom.to_string();
            self.edit_document(DocumentAction::UpdateAnnotation { index, mark }, cx)?;
        }
        Ok(())
    }
    pub(super) fn counter_number(&self) -> u32 {
        if let Some(mark) = self
            .interaction
            .selected
            .and_then(|i| self.document.marks.get(i))
            .filter(|m| m.tool == Tool::Counter)
        {
            return mark.text.parse().unwrap_or(1);
        }
        self.interaction.next_counter.unwrap_or_else(|| {
            self.document
                .marks
                .iter()
                .filter(|m| m.tool == Tool::Counter)
                .filter_map(|m| m.text.parse::<u32>().ok())
                .max()
                .unwrap_or(0)
                .saturating_add(1)
                .min(999)
        })
    }
    pub(super) fn set_counter_number(
        &mut self,
        number: u32,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.options_tool() != Tool::Counter {
            return Err("Choose a step first".into());
        }
        self.cancel_gesture();
        if let Some(index) = self.interaction.selected {
            let mut mark = self.document.marks[index].clone();
            mark.text = number.to_string();
            self.edit_document(DocumentAction::UpdateAnnotation { index, mark }, cx)?;
        } else {
            self.interaction.next_counter = Some(number);
        }
        Ok(())
    }
    pub(super) fn line_point(
        &mut self,
        straighten: bool,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let index = self
            .interaction
            .selected
            .filter(|i| {
                self.document
                    .marks
                    .get(*i)
                    .is_some_and(|m| m.tool == Tool::Arrow)
            })
            .ok_or("Select a line to edit its points")?;
        self.cancel_gesture();
        let mut mark = self.document.marks[index].clone();
        if straighten {
            mark.points = vec![mark.points[0], *mark.points.last().unwrap()];
            mark.curve = None;
        } else {
            if mark.points.len() >= 32 {
                return Err("Maximum 32 line points".into());
            }
            if mark.curve.is_some() {
                mark.points = vec![
                    mark.points[0],
                    crate::arrow::at(&mark, 0.5),
                    *mark.points.last().unwrap(),
                ];
            } else {
                let i = mark
                    .points
                    .windows(2)
                    .enumerate()
                    .max_by(|(_, a), (_, b)| {
                        (a[1].0 - a[0].0)
                            .hypot(a[1].1 - a[0].1)
                            .total_cmp(&(b[1].0 - b[0].0).hypot(b[1].1 - b[0].1))
                    })
                    .unwrap()
                    .0;
                let a = mark.points[i];
                let b = mark.points[i + 1];
                mark.points
                    .insert(i + 1, ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5));
            }
            mark.curve = None;
        }
        self.edit_document(DocumentAction::UpdateAnnotation { index, mark }, cx)?;
        Ok(())
    }
}
