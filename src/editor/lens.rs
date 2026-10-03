use super::jobs::Message;
use super::{Editor, render_image};
use crate::{
    document::{Mark, Tool},
    effects,
};
impl Editor {
    pub fn prepare_lens(&mut self, overlays: &[Mark]) {
        let Some(mark) = overlays.iter().find(|m| m.tool == Tool::Magnifier) else {
            self.preview.lens_wanted = None;
            return;
        };
        let key = effects::LensKey::new(self.preview.revision, mark);
        self.preview.lens_wanted = Some(key);
        if self.preview.lens_rendering
            || self
                .preview
                .lens
                .as_ref()
                .is_some_and(|(cached, _)| *cached == key)
        {
            return;
        }
        self.preview.lens_rendering = true;
        let document = self.document.render_snapshot();
        let mark = mark.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let source = document.plain(None);
            let image = render_image(effects::tile(&source, &mark));
            let _ = sender.send_blocking(Message::Lens(key, image));
        });
    }
}
