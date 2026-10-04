//! GPUI editor entity: state ownership and application integration.
#[cfg(test)]
mod action_tests;
pub(crate) mod actions;
mod automation;
mod canvas;
mod commands;
mod dispatch;
mod feedback;
mod input;
mod jobs;
mod lens;
mod panels;
mod state;
#[cfg(test)]
mod tests;
mod text_input;
mod view;

use crate::{
    document::{self, Document, Tool},
    gestures,
};
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use gpui::*;
pub(crate) use jobs::Message;
use jobs::OperationState;
pub(crate) use state::Layout;
use state::{
    FeedbackState, Gesture, InteractionState, PanelState, PlaybackState, PreviewState,
    VideoExportState, ViewportState,
};
use std::{cell::Cell, rc::Rc, sync::Arc};
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
    operations: OperationState,
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
        let motion_sender = sender.clone();
        let motion_preview = Rc::new(std::cell::RefCell::new(crate::animation::Preview::new(
            move || {
                let _ = motion_sender.try_send(Message::MotionPreviewReady);
            },
        )));
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
        if native
            && std::env::args().any(|arg| arg == "--automation")
            && let Err(error) = crate::automation::listen(sender.clone())
        {
            eprintln!("Glance automation: {error}");
        }
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
                lens: None,
                lens_wanted: None,
                lens_rendering: false,
                revision: 0,
                mark_count: 0,
                rendering: false,
                waiting: false,
                image: preview,
                retired: vec![],
            },
            playback: PlaybackState {
                motion_preview,
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
                backdrop_disabled: None,
                popup: None,
                popup_index: 0,
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
            operations: OperationState::default(),
            sender,
            _hotkeys: hotkeys,
        }
    }
}
