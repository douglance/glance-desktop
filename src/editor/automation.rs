use super::Editor;
use crate::{
    automation::{Request, Snapshot, state},
    document::Tool,
};
use gpui::Context;
use serde_json::json;
impl Editor {
    pub fn automation(&mut self, request: Request, cx: &mut Context<Self>) {
        match request {
            Request::Show(reply) => {
                cx.activate(true);
                let _ = reply.send(Ok(json!({"native_window":true})));
            }
            Request::Snapshot(reply) => {
                if self.is_busy()
                    || self.interaction.gesture.is_active()
                    || self.interaction.text_edit.is_some()
                {
                    let _ = reply.send(Err("Editor is busy or has an unfinished gesture/text edit. Finish it and retry.".into()));
                    return;
                }
                let _ = reply.send(Ok(Snapshot {
                    document: self.document.clone(),
                    revision: self.preview.revision,
                    phase: self.animation_phase(),
                }));
            }
            Request::Apply {
                document,
                revision,
                replace,
                reply,
            } => {
                if self.preview.revision != revision
                    || self.is_busy()
                    || self.interaction.gesture.is_active()
                    || self.interaction.text_edit.is_some()
                {
                    let _ = reply.send(Err(
                        "Editor changed during operation. Read state and retry.".into(),
                    ));
                    return;
                }
                self.document = document;
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
                cx.notify();
                let _ = reply.send(Ok(state(&Snapshot {
                    document: self.document.render_snapshot(),
                    revision: self.preview.revision,
                    phase: 0.,
                })));
            }
        }
    }
}
