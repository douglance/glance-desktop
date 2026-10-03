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
#[derive(Clone, Copy, Default)]
pub(crate) struct Layout {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) scale: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}
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
pub(crate) struct Editor {
    document: Document,
    revision: u64,
    preview_count: usize,
    rendering: bool,
    waiting_preview: bool,
    preview: Arc<RenderImage>,
    retired: Vec<Arc<RenderImage>>,
    selected: Option<usize>,
    drag_handle: Option<usize>,
    object_drag: Option<(usize, (f32, f32), Mark)>,
    tool: Tool,
    color: [u8; 4],
    width: f32,
    draft: Option<Mark>,
    pub(crate) text_edit: Option<text::Edit>,
    text_session: u64,
    animation_epoch: std::time::Instant,
    animation_paused: bool,
    animation_position: f32,
    last_video: Option<std::path::PathBuf>,
    video_progress: Option<u32>,
    video_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    backdrop_panel: bool,
    backdrop_drag: Option<(backdrop::Control, Bounds<Pixels>)>,
    enhance_panel: bool,
    resize_scale: f32,
    resize_smart: bool,

    canvas_bounds: Rc<Cell<Bounds<Pixels>>>,
    space_down: bool,
    zoom_down: bool,
    _gestures: Option<gestures::Monitor>,
    zoom: Option<f32>,
    pan: (f32, f32),
    pan_start: Option<Point<Pixels>>,
    layout: Rc<Cell<Layout>>,
    pub(crate) focus: FocusHandle,
    status: String,
    copy_feedback: Option<CopyFeedback>,
    copy_feedback_timer: Option<Task<()>>,
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
            revision: 0,
            preview_count: 0,
            rendering: false,
            waiting_preview: false,
            preview,
            retired: vec![],
            selected: None,
            drag_handle: None,
            object_drag: None,
            tool: Tool::Select,
            color: [255, 56, 100, 255],
            width: 5.,
            draft: None,
            text_edit: None,
            text_session: 0,
            animation_epoch: std::time::Instant::now(),
            animation_paused: false,
            animation_position: 0.,
            last_video: None,
            video_progress: None,
            video_cancel: None,
            backdrop_panel: false,
            backdrop_drag: None,
            enhance_panel: false,
            resize_scale: 2.,
            resize_smart: true,

            canvas_bounds,
            space_down: false,
            zoom_down: false,
            _gestures: gestures,
            zoom: None,
            pan: (0., 0.),
            pan_start: None,
            layout: Rc::new(Cell::new(Layout::default())),
            focus: cx.focus_handle(),
            status,
            copy_feedback: None,
            copy_feedback_timer: None,
            busy: false,
            sender,

            _hotkeys: hotkeys,
        }
    }
}
