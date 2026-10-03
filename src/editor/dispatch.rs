//! The single entry point for application actions. Triggers only construct data.
use super::{
    Editor,
    actions::{Action, ActionReceipt, AnimationFormat, Panel},
};
use crate::{
    animation::Motion,
    backdrop::{Backdrop, Control, PRESETS},
    document::{Tool, actions::DocumentAction},
};
use gpui::{Bounds, ClipboardItem, Context, point, px, size};

impl Editor {
    /// UI adapters surface rejected actions in the status line.
    pub(super) fn dispatch_ui(&mut self, action: Action, cx: &mut Context<Self>) {
        if let Err(error) = self.dispatch(action, cx) {
            self.feedback.status = error;
            cx.notify();
        }
    }
    pub(crate) fn dispatch(
        &mut self,
        action: Action,
        cx: &mut Context<Self>,
    ) -> Result<ActionReceipt, String> {
        if matches!(action, Action::Edit { .. })
            && (self.interaction.text_edit.is_some() || self.interaction.gesture.is_active())
        {
            return Err(
                "Finish the current text edit or gesture before editing the document.".into(),
            );
        }
        if let Action::ApplyPreparedDocument { revision, .. } = &action
            && (*revision != self.preview.revision
                || self.interaction.gesture.is_active()
                || self.interaction.text_edit.is_some())
        {
            return Err("Editor changed during operation. Read state and retry.".into());
        }
        let editing_text = self.interaction.text_edit.is_some();
        let contextual_text = editing_text
            && matches!(
                action,
                Action::Copy
                    | Action::Cut
                    | Action::Paste
                    | Action::Undo
                    | Action::Redo
                    | Action::Delete
            );
        if self.is_busy()
            && !contextual_text
            && !matches!(
                action,
                Action::Show
                    | Action::CancelExport
                    | Action::RevealExport
                    | Action::Help
                    | Action::Quit
                    | Action::ClosePanel { .. }
                    | Action::Cancel
            )
        {
            return Err("Editor is busy. Wait for the current operation to finish.".into());
        }
        // Validate before committing text or canceling a gesture.
        match &action {
            Action::SetStrokeWidth { width }
                if !width.is_finite() || !(0.5..=64.).contains(width) =>
            {
                return Err("Stroke width must be 0.5..64".into());
            }
            Action::Resize { scale, .. } | Action::SetResizeScale { scale }
                if !scale.is_finite() || !(0.1..=4.).contains(scale) =>
            {
                return Err("Resize scale must be 0.1..4".into());
            }
            Action::Zoom { factor } | Action::ZoomAt { factor, .. }
                if !factor.is_finite() || *factor <= 0. =>
            {
                return Err("Zoom factor must be positive and finite".into());
            }
            Action::ZoomAt { anchor, .. } | Action::NudgeSelection { delta: anchor, .. }
                if !anchor.0.is_finite() || !anchor.1.is_finite() =>
            {
                return Err("Coordinates must be finite".into());
            }
            Action::SetBackdropPreset { preset } if *preset >= PRESETS.len() => {
                return Err("Invalid backdrop preset".into());
            }
            Action::SetBackdrop { backdrop: Some(b) }
                if b.preset >= PRESETS.len()
                    || b.padding > 512
                    || b.inner_radius > 256
                    || b.outer_radius > 256
                    || b.shadow > 128
                    || !(2..=15).contains(&b.seconds) =>
            {
                return Err("Backdrop values out of range".into());
            }
            Action::SetBackdropControl { control, value }
                if *value < control.min() || *value > control.max() =>
            {
                return Err("Backdrop control value out of range".into());
            }
            Action::BeginBackdropAdjustment {
                track, position, ..
            } if ![track.0, track.1, track.2, track.3, position.0, position.1]
                .iter()
                .all(|n| n.is_finite())
                || track.2 <= 0.
                || track.3 <= 0. =>
            {
                return Err("Invalid slider bounds".into());
            }
            _ => {}
        }
        let previous_operation = self.operations.active.as_ref().map(|op| op.id);
        match action {
            Action::Edit { edit } => self.edit_document(edit, cx)?,
            Action::ApplyPreparedDocument {
                document, replace, ..
            } => {
                self.document = *document;
                self.interaction.selected = self.document.marks.len().checked_sub(1);
                self.interaction.tool = Tool::Select;
                if replace {
                    self.viewport.zoom = None;
                    self.viewport.pan = (0., 0.);
                }
                self.playback.position = 0.;
                self.playback.epoch = std::time::Instant::now();
                self.preview.mark_count = usize::MAX;
                self.changed();
            }
            Action::Show => cx.activate(true),
            Action::Capture { area } => self.capture(area, cx),
            Action::OpenImage => self.open(cx),
            Action::OpenPath { path } => self.open_path(path, cx),
            Action::SaveImage => self.export(true, cx),
            Action::CopyImage => self.export(false, cx),
            Action::CopyRemote => self.copy_remote(cx),
            Action::PasteImage => self.paste_image(cx),
            Action::Copy
            | Action::Cut
            | Action::Paste
            | Action::Undo
            | Action::Redo
            | Action::Delete
                if contextual_text =>
            {
                let edit = self.interaction.text_edit.as_mut().unwrap();
                match action {
                    Action::Copy | Action::Cut => {
                        let range = edit.buffer.selection();
                        if !range.is_empty() {
                            cx.write_to_clipboard(ClipboardItem::new_string(
                                edit.buffer.text()[range].into(),
                            ));
                            if matches!(action, Action::Cut) {
                                edit.replace_text("");
                            }
                        }
                    }
                    Action::Paste => {
                        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                            edit.replace_text(&text);
                        }
                    }
                    Action::Undo | Action::Redo => {
                        edit.buffer.history(matches!(action, Action::Redo))
                    }
                    Action::Delete => edit.buffer.delete(false),
                    _ => unreachable!(),
                }
                edit.caret_on = true;
            }
            Action::Copy => self.export(false, cx),
            Action::Cut => return Err("Cut requires an active text edit".into()),
            Action::Paste => self.paste_image(cx),
            Action::Undo => self.history(false, cx),
            Action::Redo => self.history(true, cx),
            Action::Delete => self.delete_selected(cx),
            Action::DuplicateSelection => self.duplicate_selected(cx),
            Action::SelectTool { tool } => self.set_tool(tool, cx),
            Action::SetColor { color } => {
                self.interaction.color = color;
                self.apply_style(true, cx);
            }
            Action::SetStrokeWidth { width } => {
                self.interaction.width = width;
                self.apply_style(false, cx);
            }
            Action::CycleStrokeWidth => {
                self.interaction.width = match self.interaction.width as u32 {
                    3 => 5.,
                    5 => 9.,
                    _ => 3.,
                };
                self.apply_style(false, cx);
            }
            Action::CycleMagnifierZoom => {
                if let Some(index) = self.interaction.selected.filter(|i| {
                    self.document
                        .marks
                        .get(*i)
                        .is_some_and(|m| m.tool == Tool::Magnifier)
                }) {
                    self.cancel_gesture();
                    let next = match crate::effects::zoom(&self.document.marks[index]) as u32 {
                        2 => 3,
                        3 => 4,
                        _ => 2,
                    };
                    let mut mark = self.document.marks[index].clone();
                    mark.text = next.to_string();
                    self.edit_document(DocumentAction::UpdateAnnotation { index, mark }, cx)?;
                }
            }
            Action::NudgeSelection { delta, remember } => {
                self.cancel_gesture();
                if let Some(index) = self
                    .interaction
                    .selected
                    .filter(|i| *i < self.document.marks.len())
                {
                    self.edit_document(
                        DocumentAction::MoveAnnotation {
                            index,
                            delta,
                            remember,
                        },
                        cx,
                    )?;
                }
            }
            Action::Fit | Action::ActualSize => {
                self.viewport.zoom = matches!(action, Action::ActualSize).then_some(1.);
                self.viewport.pan = (0., 0.);
            }
            Action::Zoom { factor } => self.change_zoom(factor, cx),
            Action::ZoomAt { factor, anchor } => self.zoom_at(factor, anchor, cx),
            Action::ToggleBackdrop => self.toggle_backdrop(cx),
            Action::ToggleEnhance => self.toggle_enhance(cx),
            Action::ClosePanel { panel } => match panel {
                Panel::Backdrop => self.panels.backdrop = false,
                Panel::Enhance => self.panels.enhance = false,
            },
            Action::SetResizeScale { scale } => self.panels.resize_scale = scale,
            Action::ToggleSmartResize => self.panels.resize_smart = !self.panels.resize_smart,
            Action::ApplyResize => self.resize_image(false, cx),
            Action::Resize { scale, smart } => {
                self.panels.resize_scale = scale;
                self.panels.resize_smart = smart;
                self.resize_image(false, cx);
            }
            Action::Rotate => self.resize_image(true, cx),
            Action::SetBackdrop { backdrop } => {
                self.commit_text(cx);
                self.cancel_gesture();
                if let Some(b) = backdrop {
                    self.backdrop_style(|current| *current = b, cx);
                } else if self.document.backdrop.is_some() {
                    self.edit_document(DocumentAction::SetBackdrop { backdrop: None }, cx)?;
                    self.feedback.status = "Backdrop removed • ⌘Z to restore".into();
                    self.panels.backdrop = false;
                }
            }
            Action::SetBackdropFill { gradient } => self.backdrop_style(
                |b| {
                    b.gradient = gradient;
                    b.motion = Motion::Still;
                },
                cx,
            ),
            Action::SelectMotion { motion } => self.backdrop_style(
                |b| {
                    if b.motion != motion
                        && let Some(preset) = motion.suggested_preset()
                    {
                        b.preset = preset;
                    }
                    b.motion = motion;
                    b.gradient = true;
                },
                cx,
            ),
            Action::SetBackdropPreset { preset } => self.backdrop_style(|b| b.preset = preset, cx),
            Action::SetBackdropControl { control, value } => {
                let phase = self.animation_phase();
                let adjusting = matches!(
                    self.interaction.gesture,
                    super::state::Gesture::AdjustingBackdrop(..)
                );
                if !adjusting {
                    self.commit_text(cx);
                    self.cancel_gesture();
                    self.document.remember();
                }
                let b = self.document.backdrop.get_or_insert(Backdrop::default());
                let previous = *b;
                control.set(b, value);
                let changed = previous != *b;
                if control == Control::Duration {
                    self.playback.position = phase * b.seconds as f32;
                    self.playback.epoch = std::time::Instant::now();
                }
                if changed {
                    self.preview.revision += 1;
                }
            }
            Action::BeginBackdropAdjustment {
                control,
                track,
                position,
            } => {
                self.commit_text(cx);
                self.cancel_gesture();
                self.document.remember();
                self.document.backdrop.get_or_insert(Backdrop::default());
                self.interaction.gesture = super::state::Gesture::AdjustingBackdrop(
                    control,
                    Bounds::new(
                        point(px(track.0), px(track.1)),
                        size(px(track.2), px(track.3)),
                    ),
                );
                self.backdrop_slider_move(point(px(position.0), px(position.1)), cx);
            }
            Action::TogglePlayback => self.toggle_animation(cx),
            Action::ExportAnimation { format } => match format {
                AnimationFormat::Mp4 => self.export_video(cx),
                AnimationFormat::Gif => self.export_gif(cx),
            },
            Action::CancelExport => self.cancel_video(cx),
            Action::RevealExport => {
                if let Some(path) = &self.video_export.last_video {
                    std::process::Command::new("/usr/bin/open")
                        .arg("-R")
                        .arg(path)
                        .spawn()
                        .map_err(|e| e.to_string())?;
                }
            }
            Action::CommitText => self.commit_text(cx),
            Action::Cancel => {
                if self.video_export.cancel.is_some() {
                    self.cancel_video(cx);
                } else if self.interaction.text_edit.take().is_some() {
                    self.feedback.status = "Text canceled".into();
                } else {
                    self.cancel_gesture();
                    self.interaction.selected = None;
                }
            }
            Action::Help => cx.open_url("https://github.com/benvinegar/pachiri#workflow"),
            Action::Quit => cx.quit(),
        }
        cx.notify();
        Ok(ActionReceipt {
            revision: self.preview.revision,
            operation_id: self
                .operations
                .active
                .as_ref()
                .filter(|op| Some(op.id) != previous_operation)
                .map(|op| op.id.value()),
        })
    }
}
