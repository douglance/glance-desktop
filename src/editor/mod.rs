mod state;
mod text_input;
pub(crate) use jobs::Message;
pub(crate) use state::Layout;
use state::*;
mod canvas;
mod commands;
mod feedback;
mod input;
mod jobs;
mod panels;
#[cfg(test)]
mod tests;
mod view;
use crate::animation::Motion;
use crate::backdrop::{Backdrop, Control, PRESETS};
use crate::document::{Document, Mark, Tool};
use crate::{
    animation, arrow, backdrop, document, drawing, gestures, glance, menus, navigation, platform,
    text, video,
};
use feedback::CopyFeedback;
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use gpui::{prelude::*, *};
use std::{
    cell::Cell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use view::{HoverLabel, icon};
pub(crate) struct Editor {
    document: Document,
    interaction: InteractionState,
    viewport: ViewportState,
    preview: PreviewState,
    playback: PlaybackState,
    video_export: VideoExportState,
    panels: PanelState,
    feedback: FeedbackState,
    _gestures: Option<gestures::Monitor>,
    pub(crate) focus: FocusHandle,
    busy: bool,
    sender: async_channel::Sender<Message>,
    _hotkeys: Option<GlobalHotKeyManager>,
}
pub(crate) fn render_image(mut image: image::RgbaImage) -> Arc<RenderImage> {
    for p in image.pixels_mut() {
        p.0.swap(0, 2);
    }
    Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
        image
    )]))
}
fn preview_base(doc: &Document) -> image::RgbaImage {
    image::DynamicImage::ImageRgba8(doc.render(None))
        .thumbnail(1600, 1200)
        .to_rgba8()
}
impl Editor {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        Self::with_native(cx, true)
    }
    pub(super) fn with_native(cx: &mut Context<Self>, native: bool) -> Self {
        let document = Document::new(document::demo());
        let base = preview_base(&document);
        let preview = render_image(base.clone());
        let (sender, receiver) = async_channel::unbounded();
        let area = HotKey::new(Some(Modifiers::SUPER | Modifiers::ALT), Code::Digit2);
        let full = HotKey::new(Some(Modifiers::SUPER | Modifiers::ALT), Code::Digit3);
        let mut status = "Practice on this canvas, or capture your screen with ⌘⌥2".to_string();
        let hotkeys = if !native {
            None
        } else {
            match GlobalHotKeyManager::new() {
                Ok(manager) => {
                    for key in [area, full] {
                        if let Err(e) = manager.register(key) {
                            status = format!("Shortcut unavailable: {e}. Use the capture buttons.");
                        }
                    }
                    Some(manager)
                }
                Err(e) => {
                    status = format!("Global shortcuts unavailable: {e}");
                    None
                }
            }
        };
        let hotkey_sender = sender.clone();
        if native {
            GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
                if event.state == HotKeyState::Pressed {
                    if event.id == area.id() {
                        let _ = hotkey_sender.try_send(Message::Hotkey(true));
                    }
                    if event.id == full.id() {
                        let _ = hotkey_sender.try_send(Message::Hotkey(false));
                    }
                }
            }));
        }
        cx.spawn(async move |view, cx| {
            while let Ok(message) = receiver.recv().await {
                if view
                    .update(cx, |editor, cx| editor.receive(message, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let canvas_bounds = Rc::new(Cell::new(Bounds::default()));
        let gestures =
            native.then(|| gestures::Monitor::new(sender.clone(), canvas_bounds.clone()));
        Self {
            document,
            interaction: InteractionState {
                gesture: Gesture::Idle,
                selected: None,

                tool: Tool::Select,
                color: [255, 56, 100, 255],
                width: 5.,

                text_edit: None,
                text_session: 0,
            },
            viewport: ViewportState {
                canvas_bounds,
                space_down: false,
                zoom_down: false,
                zoom: None,
                pan: (0., 0.),

                layout: Rc::new(Cell::new(Layout::default())),
            },
            preview: PreviewState {
                revision: 0,
                mark_count: 0,
                rendering: false,
                waiting: false,
                image: preview,
                retired: vec![],
            },
            playback: PlaybackState {
                epoch: std::time::Instant::now(),
                paused: false,
                position: 0.,
            },
            video_export: VideoExportState {
                last_video: None,
                progress: None,
                cancel: None,
            },
            panels: PanelState {
                backdrop: false,
                enhance: false,
                resize_scale: 2.,
                resize_smart: true,
            },
            feedback: FeedbackState {
                status,
                copy: None,
                timer: None,
            },
            _gestures: gestures,
            focus: cx.focus_handle(),
            busy: false,
            sender,
            _hotkeys: hotkeys,
        }
    }
}
