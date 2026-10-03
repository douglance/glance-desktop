mod animation;
mod arrow;
mod backdrop;
mod backdrop_panel;
mod document;
mod drawing;
mod enhance;
mod enhance_panel;
mod gestures;
mod glance;
mod icons;
#[cfg(test)]
mod interaction_tests;
mod menus;
mod navigation;
#[cfg(test)]
mod performance;
mod platform;
mod selection;
#[cfg(test)]
mod stress_tests;
mod text;
mod video;
actions!(pachiri, [Quit]);
use document::{Document, Mark, Tool};
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc, sync::Arc};

#[derive(Clone, Copy, Default)]
struct Layout {
    x: f32,
    y: f32,
    scale: f32,
    width: f32,
    height: f32,
}
enum Message {
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
struct Editor {
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
    text_edit: Option<text::Edit>,
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
    focus: FocusHandle,
    status: String,
    busy: bool,
    sender: async_channel::Sender<Message>,
    _hotkeys: Option<GlobalHotKeyManager>,
}
fn render_image(mut image: image::RgbaImage) -> Arc<RenderImage> {
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
struct HoverLabel(SharedString);
impl Render for HoverLabel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_2()
            .rounded_md()
            .bg(rgb(0x282b34))
            .text_color(rgb(0xffffff))
            .text_xs()
            .shadow_md()
            .child(self.0.clone())
    }
}
fn icon(name: &'static str, color: u32) -> impl IntoElement {
    svg()
        .path(format!("icons/{name}.svg"))
        .size(px(18.))
        .flex_shrink_0()
        .text_color(rgb(color))
}
impl Editor {
    fn new(cx: &mut Context<Self>) -> Self {
        Self::with_native(cx, true)
    }
    fn with_native(cx: &mut Context<Self>, native: bool) -> Self {
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
            busy: false,
            sender,

            _hotkeys: hotkeys,
        }
    }
    fn schedule_preview(&mut self) {
        if self.rendering {
            return;
        }
        self.rendering = true;
        let mut document = self.document.render_snapshot();
        if let Some((index, _, _)) = &self.object_drag {
            document.marks.truncate(*index);
        }
        let revision = self.revision;
        let count = document.marks.len();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let image = render_image(preview_base(&document));
            let _ = sender.send_blocking(Message::Preview(revision, count, image));
        });
    }
    fn changed(&mut self) {
        self.revision += 1;
        self.schedule_preview();
    }
    fn receive(&mut self, message: Message, cx: &mut Context<Self>) {
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
                    || self.draft.is_some()
                    || self.object_drag.is_some()
                    || self.pan_start.is_some()
                {
                    return;
                }
                if smart {
                    if self.zoom.is_some_and(|z| z >= 1.) {
                        self.zoom = None;
                        self.pan = (0., 0.);
                    } else {
                        self.zoom_at(
                            1. / self.zoom.unwrap_or(self.layout.get().scale).max(0.01),
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
                self.selected = None;
                self.object_drag = None;
                self.document = document;
                self.revision += 1;
                self.retired
                    .push(std::mem::replace(&mut self.preview, image));
                self.preview_count = count;
                self.zoom = None;
                self.pan = (0., 0.);
                self.busy = false;
            }
            Message::Transformed(Err(e)) => {
                self.busy = false;
                self.status = e;
            }
            Message::Hotkey(area) => {
                self.capture(area, cx);
                return;
            }
            Message::Preview(revision, count, image) => {
                self.rendering = false;
                if revision == self.revision {
                    self.retired
                        .push(std::mem::replace(&mut self.preview, image));
                    self.preview_count = count;
                    if self.waiting_preview {
                        self.waiting_preview = false;
                        self.busy = false;
                        self.status = "Ready".into();
                    }
                } else {
                    self.schedule_preview();
                }
                cx.notify();
                return;
            }
            Message::Cropped(document, image) => {
                let count = document.marks.len();
                self.selected = None;
                self.object_drag = None;
                self.document = document;
                self.revision += 1;
                self.retired
                    .push(std::mem::replace(&mut self.preview, image));
                self.preview_count = count;
                self.zoom = None;
                self.pan = (0., 0.);
                self.busy = false;
                self.status = "Cropped • ⌘Z to restore".into();
            }
            Message::Image(Ok(Some(image))) => {
                self.selected = None;
                self.object_drag = None;
                self.document = Document::new(image);
                self.zoom = None;
                self.pan = (0., 0.);
                self.draft = None;
                self.preview_count = usize::MAX;
                self.waiting_preview = true;
                self.changed();
                self.status = "Preparing image…".into();
            }
            Message::Image(Ok(None)) => {
                self.busy = false;
                self.status = "Selection canceled".into();
            }
            Message::Image(Err(e)) => {
                self.busy = false;
                self.status = e;
            }
            Message::VideoProgress(percent) => {
                self.video_progress = Some(percent);
                cx.notify();
                return;
            }
            Message::VideoSaved(result) => {
                self.busy = false;
                self.video_progress = None;
                self.video_cancel = None;
                self.status = match result {
                    Ok(Some(path)) => {
                        self.last_video = Some(path.clone());
                        format!("Video saved to {}", path.display())
                    }
                    Ok(None) => "Video export canceled".into(),
                    Err(e) => e,
                };
            }
            Message::Saved(result) => {
                self.busy = false;
                self.status = match result {
                    Ok(Some(path)) => format!("Saved {}", path.display()),
                    Ok(None) => "Save canceled".into(),
                    Err(e) => e,
                };
            }
            Message::Copied(result) => {
                self.busy = false;
                self.status = match result {
                    Ok(()) => "Copied image to clipboard".into(),
                    Err(e) => e,
                };
            }
            Message::RemoteCopied(result) => {
                self.busy = false;
                self.status = match result {
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
                        format!(
                            "Glance link copied • expires in {minutes} min • Paste into your agent’s chat"
                        )
                    }
                    Err(e) => e,
                };
            }
        }
        cx.activate(true);
        if failed && let Some(window) = cx.windows().first().copied() {
            let detail = self.status.clone();
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
    fn commit_text(&mut self, cx: &mut Context<Self>) {
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
    fn outside_text(&mut self, e: &MouseDownEvent, cx: &mut Context<Self>) {
        let layout = self.layout.get();
        let image = Bounds::new(
            point(px(layout.x), px(layout.y)),
            size(
                px(layout.width * layout.scale),
                px(layout.height * layout.scale),
            ),
        );
        if !image.contains(&e.position) {
            self.commit_text(cx);
        }
    }
    fn capture(&mut self, area: bool, cx: &mut Context<Self>) {
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
    fn open(&mut self, cx: &mut Context<Self>) {
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
    fn export(&mut self, save: bool, cx: &mut Context<Self>) {
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
    fn copy_remote(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.commit_text(cx);
        self.busy = true;
        self.status = "Uploading screenshot to Glance…".into();
        let document = self.document.render_snapshot();
        let phase = self.animation_phase();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = glance::upload(document.export_at(phase));
            let _ = sender.send_blocking(Message::RemoteCopied(result));
        });
        cx.notify();
    }
    fn history(&mut self, redo: bool, cx: &mut Context<Self>) {
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
    fn set_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        self.commit_text(cx);
        self.cancel_move();
        self.selected = None;
        self.tool = tool;
        self.draft = None;
        cx.notify();
    }
    fn coordinate(&self, p: Point<Pixels>, clamp: bool) -> Option<(f32, f32)> {
        let layout = self.layout.get();
        if layout.scale <= 0. {
            return None;
        }
        let x = (f32::from(p.x) - layout.x) / layout.scale;
        let y = (f32::from(p.y) - layout.y) / layout.scale;
        if !clamp && (x < 0. || y < 0. || x >= layout.width || y >= layout.height) {
            return None;
        }
        Some((
            x.clamp(0., layout.width - 1.),
            y.clamp(0., layout.height - 1.),
        ))
    }
    fn begin(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window);
        if self.busy {
            return;
        }
        if self.space_down {
            self.pan_start = Some(e.position);
            cx.notify();
            return;
        }
        if self.zoom_down {
            self.zoom_at(
                if e.modifiers.shift { 0.5 } else { 2. },
                (f32::from(e.position.x), f32::from(e.position.y)),
                cx,
            );
            return;
        }
        if let Some(edit) = &mut self.text_edit
            && edit
                .bounds
                .is_some_and(|bounds| bounds.dilate(px(5.)).contains(&e.position))
        {
            edit.buffer
                .move_to(edit.index(e.position), e.modifiers.shift);
            edit.selecting = true;
            edit.caret_on = true;
            cx.notify();
            return;
        }
        self.commit_text(cx);
        let Some(mut p) = self.coordinate(e.position, false) else {
            return;
        };
        if self.tool == Tool::Crop {
            p = navigation::endpoint(Tool::Crop, p, p, false, self.layout.get());
        }
        self.drag_handle = None;
        if let Some(index) = self.selected.filter(|i| *i < self.document.marks.len()) {
            let mark = &self.document.marks[index];
            self.drag_handle = arrow::handle_at(mark, p, 7. / self.layout.get().scale);
            if self.drag_handle.is_some() || mark.hit(p, 5. / self.layout.get().scale) {
                self.object_drag = Some((index, p, mark.clone()));
                self.changed();
                cx.notify();
                return;
            }
        }
        self.selected = None;
        if self.tool == Tool::Select {
            self.selected = self.document.pick(p, 5. / self.layout.get().scale);
            if let Some(index) = self.selected {
                self.color = self.document.marks[index].color;
                self.width = self.document.marks[index].width;
                self.object_drag = Some((index, p, self.document.marks[index].clone()));
                self.changed();
            }
            cx.notify();
            return;
        }
        let mut mark = Mark {
            tool: self.tool,
            curve: None,
            points: vec![p],
            color: self.color,
            width: match self.tool {
                Tool::Text => self.width.max(20. / 7.),
                Tool::Counter => self.width.max(20. / 4.4),
                _ => self.width,
            },
            text: String::new(),
        };
        if self.tool == Tool::Counter {
            mark.text = (self
                .document
                .marks
                .iter()
                .filter(|m| m.tool == Tool::Counter)
                .filter_map(|m| m.text.parse::<usize>().ok())
                .max()
                .unwrap_or(0)
                .saturating_add(1))
            .to_string();
            self.document.commit(mark);
            self.selected = self.document.marks.len().checked_sub(1);
            self.changed();
            cx.notify();
            return;
        }
        if self.tool == Tool::Text {
            self.text_edit = Some(text::Edit::new(mark));
            self.text_session += 1;
            let session = self.text_session;
            self.status = "Type your label • Enter to finish • Escape to cancel".into();
            cx.spawn(async move |view, cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(500))
                        .await;
                    let keep = view
                        .update(cx, |editor, cx| {
                            if editor.text_session != session {
                                return false;
                            }
                            if let Some(edit) = &mut editor.text_edit {
                                edit.caret_on = !edit.caret_on;
                                cx.notify();
                                true
                            } else {
                                false
                            }
                        })
                        .unwrap_or(false);
                    if !keep {
                        break;
                    }
                }
            })
            .detach();
        } else {
            self.draft = Some(mark);
        }
        cx.notify();
    }
    fn motion(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.backdrop_slider_move(e.position, cx) {
            return;
        }
        if let Some(previous) = self.pan_start {
            self.pan.0 += f32::from(e.position.x - previous.x);
            self.pan.1 += f32::from(e.position.y - previous.y);
            self.pan_start = Some(e.position);
            cx.notify();
            return;
        }
        if let Some(edit) = &mut self.text_edit
            && edit.selecting
        {
            edit.buffer.move_to(edit.index(e.position), true);
            edit.caret_on = true;
            cx.notify();
            return;
        }
        if let Some((index, start, _)) = self.object_drag.as_ref() {
            if let Some(p) = self.coordinate(e.position, true) {
                let mut moved = self.document.marks[*index].clone();
                let d = navigation::translation(
                    *start,
                    p,
                    e.modifiers.shift && self.drag_handle.is_none(),
                );
                arrow::drag(&mut moved, self.drag_handle, d, e.modifiers.shift);
                self.object_drag.as_mut().unwrap().2 = moved;
                cx.notify();
            }
            return;
        }
        if self.draft.is_none() {
            return;
        }
        let Some(p) = self.coordinate(e.position, true) else {
            return;
        };
        if let Some(mark) = &mut self.draft {
            if mark.tool == Tool::Pen {
                if let Some(last) = mark.points.last()
                    && (p.0 - last.0).hypot(p.1 - last.1) * self.layout.get().scale < 0.35
                {
                    return;
                }
                mark.points.push(p);
            } else {
                mark.points.truncate(1);
                mark.points.push(navigation::endpoint(
                    mark.tool,
                    mark.points[0],
                    p,
                    e.modifiers.shift,
                    self.layout.get(),
                ));
            }
            cx.notify();
        }
    }
    fn finish(&mut self, e: &MouseUpEvent, cx: &mut Context<Self>) {
        if self.pan_start.take().is_some() {
            cx.notify();
            return;
        }
        if let Some((index, start, mut moved)) = self.object_drag.take() {
            if let Some(p) = self.coordinate(e.position, true) {
                moved = self.document.marks[index].clone();
                let d = navigation::translation(
                    start,
                    p,
                    e.modifiers.shift && self.drag_handle.is_none(),
                );
                arrow::drag(&mut moved, self.drag_handle, d, e.modifiers.shift);
            }
            if moved.points != self.document.marks[index].points
                || moved.curve != self.document.marks[index].curve
            {
                self.document.remember();
                self.document.marks[index] = moved;
            }
            self.changed();
            cx.notify();
            return;
        }
        if self.backdrop_drag.take().is_some() {
            cx.notify();
            return;
        }
        if let Some(edit) = &mut self.text_edit {
            edit.selecting = false;
        }
        if let Some(p) = self.coordinate(e.position, true)
            && let Some(mark) = &mut self.draft
        {
            mark.points.push(navigation::endpoint(
                mark.tool,
                mark.points[0],
                p,
                e.modifiers.shift,
                self.layout.get(),
            ));
        }
        if let Some(mut mark) = self.draft.take() {
            if mark.tool == Tool::Arrow && mark.points.len() > 2 {
                let end = *mark.points.last().unwrap();
                mark.points.truncate(1);
                mark.points.push(end);
            }
            if mark.tool == Tool::Crop {
                self.busy = true;
                self.status = "Cropping…".into();
                let mut document = self.document.clone();
                let sender = self.sender.clone();
                std::thread::spawn(move || {
                    document.commit(mark);
                    let image = render_image(preview_base(&document));
                    let _ = sender.send_blocking(Message::Cropped(document, image));
                });
            } else {
                self.document.commit(mark);
                self.selected = self.document.marks.len().checked_sub(1);
                self.changed();
            }
            cx.notify();
        }
    }
    fn menu_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let e = KeyDownEvent {
            keystroke: Keystroke::parse(key).expect("valid menu shortcut"),
            is_held: false,
        };
        self.key(&e, window, cx);
    }
    fn key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = e.keystroke.key.as_str();
        if key == "escape" && self.video_cancel.is_some() {
            self.cancel_video(cx);
            cx.stop_propagation();
            return;
        }
        let m = e.keystroke.modifiers;
        if self.text_edit.is_some() {
            if matches!(key, "enter" | "escape")
                && self
                    .text_edit
                    .as_ref()
                    .is_some_and(|e| e.buffer.marked.is_some())
            {
                return;
            }
            match key {
                "enter" => {
                    self.commit_text(cx);
                    cx.stop_propagation();
                    return;
                }
                "escape" => {
                    self.text_edit = None;
                    self.status = "Text canceled".into();
                    cx.notify();
                    cx.stop_propagation();
                    return;
                }
                _ => {}
            }
            if m.platform && (matches!(key, "s" | "o" | "q") || (m.shift && key == "c")) {
                self.commit_text(cx);
            } else {
                let edit = self.text_edit.as_mut().unwrap();
                let mut handled = true;
                if m.platform {
                    match key {
                        "a" => edit.buffer.select_all(),
                        "v" => {
                            if let Some(text) = cx.read_from_clipboard().and_then(|i| i.text()) {
                                edit.replace_text(&text);
                            }
                        }
                        "c" | "x" => {
                            let range = edit.buffer.selection();
                            if !range.is_empty() {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    edit.buffer.text()[range].into(),
                                ));
                                if key == "x" {
                                    edit.replace_text("");
                                }
                            }
                        }
                        "z" => edit.buffer.history(m.shift),
                        "left" => edit.buffer.move_to(0, m.shift),
                        "right" => edit.buffer.move_to(edit.buffer.text().len(), m.shift),
                        _ => handled = false,
                    }
                } else {
                    match key {
                        "backspace" => edit.buffer.delete(false),
                        "delete" => edit.buffer.delete(true),
                        "left" => edit.buffer.move_cursor(false, m.shift),
                        "right" => edit.buffer.move_cursor(true, m.shift),
                        "home" => edit.buffer.move_to(0, m.shift),
                        "end" => edit.buffer.move_to(edit.buffer.text().len(), m.shift),
                        _ => handled = false,
                    }
                }
                if handled {
                    edit.caret_on = true;
                    cx.stop_propagation();
                    cx.notify();
                }
                let _ = window;
                return;
            }
        }
        if !m.platform && !m.alt && !m.control {
            if key == "space" {
                self.space_down = true;
                cx.notify();
                cx.stop_propagation();
                return;
            }
            if key == "z" {
                self.zoom_down = true;
                cx.notify();
                cx.stop_propagation();
                return;
            }
            if matches!(key, "left" | "right" | "up" | "down")
                && self.selected.is_some()
                && !self.busy
            {
                self.cancel_move();
                if let Some(index) = self.selected {
                    if !e.is_held {
                        self.document.remember();
                    }
                    let step = if m.shift { 10. } else { 1. };
                    let d = match key {
                        "left" => (-step, 0.),
                        "right" => (step, 0.),
                        "up" => (0., -step),
                        _ => (0., step),
                    };
                    self.document.marks[index].translate(d.0, d.1);
                    self.changed();
                    cx.notify();
                }
                cx.stop_propagation();
                return;
            }
        }
        if m.platform && m.alt && matches!(key, "2" | "3") {
            self.capture(key == "2", cx);
        } else if m.platform {
            match key {
                "q" => cx.quit(),
                "c" if m.shift => self.copy_remote(cx),
                "c" => self.export(false, cx),
                "s" => self.export(true, cx),
                "o" => self.open(cx),
                "v" => self.paste_image(cx),
                "d" => self.duplicate_selected(cx),
                "z" => self.history(m.shift, cx),
                "1" => {
                    self.zoom = None;
                    self.pan = (0., 0.);
                    cx.notify();
                }
                "0" => {
                    self.zoom = Some(1.);
                    self.pan = (0., 0.);
                    cx.notify();
                }
                "+" | "=" => self.change_zoom(1.25, cx),
                "-" => self.change_zoom(0.8, cx),
                _ => {}
            }
        } else if !m.alt && !m.control {
            match key {
                "v" => self.set_tool(Tool::Select, cx),
                "backspace" | "delete" => self.delete_selected(cx),
                "p" => self.set_tool(Tool::Pen, cx),
                "a" => self.set_tool(Tool::Arrow, cx),
                "r" => self.set_tool(Tool::Rectangle, cx),
                "h" => self.set_tool(Tool::Highlight, cx),
                "b" => self.set_tool(Tool::Pixelate, cx),
                "x" => self.set_tool(Tool::Crop, cx),
                "t" => self.set_tool(Tool::Text, cx),
                "n" => self.set_tool(Tool::Counter, cx),
                "escape" => {
                    self.cancel_move();
                    self.selected = None;
                    self.draft = None;
                    cx.notify();
                }
                _ => {}
            }
        }
        cx.stop_propagation();
    }
    fn cancel_move(&mut self) {
        if self.object_drag.take().is_some() {
            self.changed();
        }
    }
    fn duplicate_selected(&mut self, cx: &mut Context<Self>) {
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
    fn apply_style(&mut self, color: bool, cx: &mut Context<Self>) {
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
    fn delete_selected(&mut self, cx: &mut Context<Self>) {
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
    fn zoom_at(&mut self, factor: f32, anchor: (f32, f32), cx: &mut Context<Self>) {
        if self.draft.is_some() || self.object_drag.is_some() || self.pan_start.is_some() {
            return;
        }
        let bounds = self.canvas_bounds.get();
        let center = (
            f32::from(bounds.origin.x + bounds.size.width * 0.5),
            f32::from(bounds.origin.y + bounds.size.height * 0.5),
        );
        if let Some((zoom, pan)) = navigation::anchored_zoom(
            self.zoom.unwrap_or(self.layout.get().scale),
            self.pan,
            center,
            anchor,
            factor,
        ) {
            self.zoom = Some(zoom);
            self.pan = pan;
            cx.notify();
        }
    }
    fn change_zoom(&mut self, factor: f32, cx: &mut Context<Self>) {
        let b = self.canvas_bounds.get();
        self.zoom_at(
            factor,
            (
                f32::from(b.origin.x + b.size.width * 0.5),
                f32::from(b.origin.y + b.size.height * 0.5),
            ),
            cx,
        );
    }
    fn scroll(&mut self, e: &ScrollWheelEvent, cx: &mut Context<Self>) {
        if self.draft.is_some() || self.object_drag.is_some() || self.pan_start.is_some() {
            return;
        }
        let delta = e.delta.pixel_delta(px(24.));
        if e.modifiers.platform {
            self.zoom_at(
                (f32::from(delta.y) * 0.008).exp(),
                (f32::from(e.position.x), f32::from(e.position.y)),
                cx,
            );
        } else {
            if e.modifiers.shift && f32::from(delta.x).abs() < 0.01 {
                self.pan.0 += f32::from(delta.y);
            } else {
                self.pan.0 += f32::from(delta.x);
                self.pan.1 += f32::from(delta.y);
            }
            cx.notify();
        }
        cx.stop_propagation();
    }
    fn tool_button(&self, tool: Tool, cx: &Context<Self>) -> impl IntoElement {
        let (name, key) = match tool {
            Tool::Select => ("mouse-pointer-2", "V"),
            Tool::Pen => ("pen-line", "P"),
            Tool::Arrow => ("arrow-up-right", "A"),
            Tool::Rectangle => ("square", "R"),
            Tool::Text => ("type", "T"),
            Tool::Highlight => ("highlighter", "H"),
            Tool::Pixelate => ("grid-2x2", "B"),
            Tool::Crop => ("crop", "X"),
            Tool::Counter => ("list-ordered", "N"),
        };
        let active = self.tool == tool;
        let label: SharedString = format!("{} · {}", tool.label(), key).into();
        div()
            .id(SharedString::from(format!("tool-{name}")))
            .size(px(30.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .cursor_pointer()
            .bg(rgb(if active { 0xffe9e4 } else { 0xfcfcfd }))
            .hover(|s| s.bg(rgb(0xf0f1f5)))
            .active(|s| s.bg(rgb(0xe5e7ed)))
            .child(icon(name, if active { 0xd94d38 } else { 0x555966 }))
            .tooltip(move |_, cx| cx.new(|_| HoverLabel(label.clone())).into())
            .on_click(cx.listener(move |this, _, _, cx| this.set_tool(tool, cx)))
    }
    fn compact_button(
        &self,
        label: &'static str,
        name: &'static str,
        active: bool,
        cx: &Context<Self>,
        action: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(label)
            .size(px(30.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .cursor_pointer()
            .bg(rgb(if active { 0xffe9e4 } else { 0xfcfcfd }))
            .hover(|s| s.bg(rgb(0xf0f1f5)))
            .child(icon(name, if active { 0xd94d38 } else { 0x555966 }))
            .tooltip(move |_, cx| cx.new(|_| HoverLabel(label.into())).into())
            .on_click(cx.listener(move |this, _, _, cx| action(this, cx)))
    }
    fn button(
        &self,
        label: &str,
        active: bool,
        cx: &Context<Self>,
        action: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        let icon_name = if label.starts_with("Area") {
            Some("scan")
        } else if label.starts_with("Screen") {
            Some("monitor")
        } else if label == "Open" {
            Some("folder-open")
        } else if label == "Backdrop" {
            Some("square")
        } else if label == "Image tools" {
            Some("sparkles")
        } else if label.starts_with("Paste") {
            Some("clipboard-paste")
        } else if label.starts_with("Rotate") {
            Some("rotate-cw")
        } else if label.starts_with("Copy") {
            Some("copy")
        } else if label.starts_with("Export MP4") {
            Some("video")
        } else if label == "Cancel export" {
            Some("square")
        } else if label.starts_with("Save") {
            Some("save")
        } else {
            None
        };
        div()
            .id(SharedString::from(label.to_string()))
            .px_3()
            .py_1()
            .h(px(32.))
            .flex()
            .items_center()
            .rounded_md()
            .cursor_pointer()
            .text_sm()
            .bg(rgb(if active { 0xffe9e4 } else { 0xffffff }))
            .text_color(rgb(if active { 0xd94d38 } else { 0x44454f }))
            .hover(|s| s.bg(rgb(0xf0f1f5)))
            .active(|s| s.bg(rgb(0xe5e7ed)))
            .gap_2()
            .when_some(icon_name, |el, name| {
                el.child(icon(name, if active { 0xd94d38 } else { 0x555966 }))
            })
            .child(label.to_string())
            .on_click(cx.listener(move |this, _, _, cx| action(this, cx)))
    }
}
impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !window.is_window_active() {
            self.space_down = false;
            self.zoom_down = false;
            self.pan_start = None;
        }
        for image in self.retired.drain(..) {
            let _ = window.drop_image(image);
        }
        let image = self.preview.clone();
        let text_entity = cx.entity();
        let overlays: Vec<Mark> = self
            .document
            .marks
            .iter()
            .skip(self.preview_count)
            .chain(self.draft.iter())
            .map(|mark| {
                if let Some((index, _, moved)) = &self.object_drag
                    && std::ptr::eq(mark, &self.document.marks[*index])
                {
                    return moved.clone();
                }
                mark.clone()
            })
            .collect();
        let selected_mark = self.selected.and_then(|index| {
            self.object_drag
                .as_ref()
                .map(|(_, _, m)| m)
                .or_else(|| self.document.marks.get(index))
                .cloned()
        });
        let selection_bounds = self.selected.and_then(|index| {
            self.object_drag
                .as_ref()
                .map(|(_, _, m)| m)
                .or_else(|| self.document.marks.get(index))
                .map(Mark::bounds)
        });
        let canvas_bounds = self.canvas_bounds.clone();
        let layout = self.layout.clone();
        let dimensions = self.document.base.dimensions();
        let backdrop = self.document.backdrop;
        let animation_phase = self.animation_phase();
        if backdrop.is_some_and(|b| b.motion != animation::Motion::Still)
            && !self.animation_paused
            && !self.busy
            && window.is_window_active()
        {
            window.request_animation_frame();
        }
        let output_dimensions = backdrop.map_or(dimensions, |b| b.dimensions(dimensions));
        let viewport = window.viewport_size();
        let fit_zoom = ((f32::from(viewport.width)
            - if self.backdrop_panel || self.enhance_panel {
                260.
            } else {
                0.
            }
            - 80.)
            / output_dimensions.0 as f32)
            .min((f32::from(viewport.height) - 48. - 70.) / output_dimensions.1 as f32)
            .clamp(0.01, 1.);
        let zoom_label = format!("{:.0}%", self.zoom.unwrap_or(fit_zoom) * 100.);
        let zoom = self.zoom;
        let pan = self.pan;
        let tools = [
            Tool::Select,
            Tool::Pen,
            Tool::Arrow,
            Tool::Rectangle,
            Tool::Text,
            Tool::Highlight,
            Tool::Pixelate,
            Tool::Crop,
            Tool::Counter,
        ];
        let colors = [
            (0xff3864, [255, 56, 100, 255]),
            (0xffb82e, [255, 184, 46, 255]),
            (0x26b690, [38, 182, 144, 255]),
            (0x4c8dff, [76, 141, 255, 255]),
            (0xffffff, [255, 255, 255, 255]),
            (0x20222a, [32, 34, 42, 255]),
        ];
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xfcfcfd))
            .text_color(rgb(0x272831))
            .font_family(".AppleSystemUIFont")
            .track_focus(&self.focus)
            .key_context(if self.text_edit.is_some() {
                "PachiriText"
            } else {
                "PachiriCanvas"
            })
            .on_action(
                cx.listener(|this, _: &menus::Open, window, cx| this.menu_key("cmd-o", window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &menus::Save, window, cx| this.menu_key("cmd-s", window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &menus::Copy, window, cx| this.menu_key("cmd-c", window, cx)),
            )
            .on_action(cx.listener(|this, _: &menus::CopyRemote, window, cx| {
                this.menu_key("cmd-shift-c", window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &menus::Paste, window, cx| {
                    this.menu_key("cmd-v", window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &menus::Undo, window, cx| this.menu_key("cmd-z", window, cx)),
            )
            .on_action(cx.listener(|this, _: &menus::Redo, window, cx| {
                this.menu_key("cmd-shift-z", window, cx)
            }))
            .on_action(cx.listener(|this, _: &menus::Delete, window, cx| {
                this.menu_key("backspace", window, cx)
            }))
            .on_action(cx.listener(|this, _: &menus::CaptureArea, window, cx| {
                this.menu_key("cmd-alt-2", window, cx)
            }))
            .on_action(cx.listener(|this, _: &menus::CaptureScreen, window, cx| {
                this.menu_key("cmd-alt-3", window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &menus::Fit, window, cx| this.menu_key("cmd-1", window, cx)),
            )
            .on_action(cx.listener(|this, _: &menus::ActualSize, window, cx| {
                this.menu_key("cmd-0", window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &menus::ZoomIn, window, cx| {
                    this.menu_key("cmd-=", window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &menus::ZoomOut, window, cx| {
                this.menu_key("cmd--", window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &menus::Select, _, cx| this.set_tool(Tool::Select, cx)),
            )
            .on_action(cx.listener(|this, _: &menus::Pen, _, cx| this.set_tool(Tool::Pen, cx)))
            .on_action(cx.listener(|this, _: &menus::Arrow, _, cx| this.set_tool(Tool::Arrow, cx)))
            .on_action(
                cx.listener(|this, _: &menus::Rectangle, _, cx| this.set_tool(Tool::Rectangle, cx)),
            )
            .on_action(cx.listener(|this, _: &menus::Text, _, cx| this.set_tool(Tool::Text, cx)))
            .on_action(
                cx.listener(|this, _: &menus::Highlight, _, cx| this.set_tool(Tool::Highlight, cx)),
            )
            .on_action(
                cx.listener(|this, _: &menus::Pixelate, _, cx| this.set_tool(Tool::Pixelate, cx)),
            )
            .on_action(cx.listener(|this, _: &menus::Crop, _, cx| this.set_tool(Tool::Crop, cx)))
            .on_action(
                cx.listener(|this, _: &menus::Counter, _, cx| this.set_tool(Tool::Counter, cx)),
            )
            .on_action(cx.listener(|this, _: &menus::ExportVideo, _, cx| this.export_video(cx)))
            .on_action(cx.listener(|this, _: &menus::Backdrop, _, cx| this.toggle_backdrop(cx)))
            .on_action(cx.listener(|this, _: &menus::ImageTools, _, cx| this.toggle_enhance(cx)))
            .on_action(|_: &menus::Help, _, cx| {
                cx.open_url("https://github.com/benvinegar/pachiri#workflow")
            })
            .on_action(cx.listener(|this, _: &menus::Duplicate, _, cx| this.duplicate_selected(cx)))
            .on_key_down(cx.listener(Self::key))
            .on_key_up(cx.listener(|this, e: &KeyUpEvent, _, cx| {
                match e.keystroke.key.as_str() {
                    "space" => {
                        this.space_down = false;
                        this.pan_start = None;
                    }
                    "z" => this.zoom_down = false,
                    _ => {}
                }
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e, _, cx| this.outside_text(e, cx)),
            )
            .on_mouse_move(cx.listener(|this, e, _, cx| this.motion(e, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, e, _, cx| this.finish(e, cx)),
            )
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(|this, _, _, _| this.pan_start = None),
            )
            .child(
                div()
                    .h(px(48.))
                    .flex_shrink_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap(px(2.))
                    .border_b_1()
                    .border_color(rgb(0xe9e9ee))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xf35d45))
                            .mr_2()
                            .child("pachiri"),
                    )
                    .child(
                        self.compact_button("Area · ⌘⌥2", "scan", false, cx, |this, cx| {
                            this.capture(true, cx)
                        }),
                    )
                    .child(self.compact_button(
                        "Screen · ⌘⌥3",
                        "monitor",
                        false,
                        cx,
                        |this, cx| this.capture(false, cx),
                    ))
                    .child(self.compact_button(
                        "Open · ⌘O",
                        "folder-open",
                        false,
                        cx,
                        |this, cx| this.open(cx),
                    ))
                    .child(div().w(px(1.)).h(px(18.)).mx_1().bg(rgb(0xe5e5ec)))
                    .children(tools.into_iter().map(|tool| self.tool_button(tool, cx)))
                    .child(div().w(px(1.)).h(px(18.)).mx_1().bg(rgb(0xe5e5ec)))
                    .child(self.compact_button(
                        "Backdrop",
                        "square",
                        self.backdrop_panel,
                        cx,
                        |this, cx| this.toggle_backdrop(cx),
                    ))
                    .child(self.compact_button(
                        "Image tools",
                        "sparkles",
                        self.enhance_panel,
                        cx,
                        |this, cx| this.toggle_enhance(cx),
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .children(colors.into_iter().map(|(hex, color)| {
                                div()
                                    .id(("color", hex))
                                    .size(px(16.))
                                    .rounded_full()
                                    .border_2()
                                    .border_color(rgb(if self.color == color {
                                        0xf35d45
                                    } else {
                                        0xd7d7df
                                    }))
                                    .bg(rgb(hex))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.color = color;
                                        this.apply_style(true, cx);
                                    }))
                            }))
                            .child(self.button(
                                &format!("{} px", self.width as u32),
                                false,
                                cx,
                                |this, cx| {
                                    this.width = match this.width as u32 {
                                        3 => 5.,
                                        5 => 9.,
                                        _ => 3.,
                                    };
                                    this.apply_style(false, cx);
                                },
                            )),
                    )
                    .child(div().flex_1())
                    .child(
                        self.compact_button("Copy · ⌘C", "copy", false, cx, |this, cx| {
                            this.export(false, cx)
                        }),
                    )
                    .child(
                        div()
                            .id("copy-remote")
                            .debug_selector(|| "copy-remote".into())
                            .flex_shrink_0()
                            .child(self.compact_button(
                                "Copy (remote) · Upload to Glance · ⌘⇧C",
                                "cloud-upload",
                                false,
                                cx,
                                |this, cx| this.copy_remote(cx),
                            )),
                    )
                    .child(
                        self.compact_button("Save · ⌘S", "save", false, cx, |this, cx| {
                            this.export(true, cx)
                        }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .text_xs()
                            .text_color(rgb(0x555966))
                            .child(
                                div()
                                    .id("image-size")
                                    .child(format!(
                                        "{}×{}",
                                        output_dimensions.0, output_dimensions.1
                                    ))
                                    .tooltip(|_, cx| {
                                        cx.new(|_| HoverLabel("Image size in pixels".into())).into()
                                    }),
                            )
                            .child(div().h(px(20.)).w(px(1.)).bg(rgb(0xe5e5ec)))
                            .child(
                                div()
                                    .id("header-zoom")
                                    .debug_selector(|| "header-zoom".into())
                                    .min_w(px(36.))
                                    .cursor_pointer()
                                    .child(zoom_label)
                                    .tooltip(|_, cx| {
                                        cx.new(|_| HoverLabel("Zoom · ⌘+/⌘− · Click to fit".into()))
                                            .into()
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.zoom = None;
                                        this.pan = (0., 0.);
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .relative()
                    .on_scroll_wheel(cx.listener(|this, e, _, cx| this.scroll(e, cx)))
                    .on_drop(cx.listener(|this, files: &ExternalPaths, _, cx| {
                        if this.busy {
                            return;
                        }
                        if let Some(path) = files.paths().first() {
                            let path = path.clone();
                            this.commit_text(cx);
                            this.cancel_move();
                            this.busy = true;
                            let sender = this.sender.clone();
                            std::thread::spawn(move || {
                                let _ = sender
                                    .send_blocking(Message::Image(platform::load(&path).map(Some)));
                            });
                            cx.notify();
                        }
                    }))
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .cursor(if self.space_down {
                        CursorStyle::OpenHand
                    } else if self.tool == Tool::Select {
                        CursorStyle::Arrow
                    } else {
                        CursorStyle::Crosshair
                    })
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::begin))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, e: &MouseDownEvent, _, _| {
                            this.pan_start = Some(e.position)
                        }),
                    )
                    .child(
                        canvas(
                            move |bounds, _, _| bounds,
                            move |bounds, _, window, cx| {
                                canvas_bounds.set(bounds);
                                window.paint_quad(fill(bounds, rgb(0xeff0f4)));
                                let fit = ((f32::from(bounds.size.width) - 80.)
                                    / output_dimensions.0 as f32)
                                    .min(
                                        (f32::from(bounds.size.height) - 70.)
                                            / output_dimensions.1 as f32,
                                    )
                                    .clamp(0.01, 1.);
                                let scale = zoom.unwrap_or(fit);
                                let w = dimensions.0 as f32 * scale;
                                let h = dimensions.1 as f32 * scale;
                                let padding = backdrop.map_or(0., |b| b.padding as f32 * scale);
                                let x = f32::from(bounds.origin.x)
                                    + (f32::from(bounds.size.width) - w) / 2.
                                    + pan.0;
                                let y = f32::from(bounds.origin.y)
                                    + (f32::from(bounds.size.height) - h) / 2.
                                    + pan.1;
                                layout.set(Layout {
                                    x,
                                    y,
                                    scale,
                                    width: dimensions.0 as f32,
                                    height: dimensions.1 as f32,
                                });
                                let image_bounds =
                                    Bounds::new(point(px(x), px(y)), size(px(w), px(h)));
                                let frame_bounds = image_bounds.dilate(px(padding));
                                if let Some(b) = backdrop {
                                    window.paint_quad(quad(
                                        frame_bounds,
                                        px(b.outer_radius as f32 * scale),
                                        b.background(),
                                        px(0.),
                                        rgb(0xffffff),
                                        Default::default(),
                                    ));
                                    if b.motion != animation::Motion::Still {
                                        window.with_content_mask(
                                            Some(ContentMask {
                                                bounds: frame_bounds.intersect(&bounds),
                                            }),
                                            |window| {
                                                animation::paint(
                                                    b,
                                                    animation_phase,
                                                    frame_bounds,
                                                    window,
                                                )
                                            },
                                        );
                                    }
                                    if b.shadow > 0 {
                                        window.with_content_mask(
                                            Some(ContentMask {
                                                bounds: frame_bounds.intersect(&bounds),
                                            }),
                                            |window| {
                                                window.paint_shadows(
                                                    image_bounds,
                                                    px(b.inner_radius as f32 * scale).into(),
                                                    &[BoxShadow {
                                                        color: rgba(0x00000038).into(),
                                                        offset: point(
                                                            px(0.),
                                                            px(b.shadow as f32 * scale * 0.25),
                                                        ),
                                                        blur_radius: px(b.shadow as f32 * scale),
                                                        spread_radius: px(0.),
                                                    }],
                                                );
                                            },
                                        );
                                    }
                                } else {
                                    window.paint_shadows(
                                        image_bounds,
                                        Default::default(),
                                        &[
                                            BoxShadow {
                                                color: rgba(0x17203320).into(),
                                                offset: point(px(0.), px(12.)),
                                                blur_radius: px(32.),
                                                spread_radius: px(0.),
                                            },
                                            BoxShadow {
                                                color: rgba(0x17203310).into(),
                                                offset: point(px(0.), px(2.)),
                                                blur_radius: px(6.),
                                                spread_radius: px(0.),
                                            },
                                        ],
                                    );
                                }
                                if backdrop.is_none() {
                                    window.paint_quad(quad(
                                        image_bounds,
                                        px(backdrop.map_or(0., |b| b.inner_radius as f32 * scale)),
                                        rgb(0xffffff),
                                        px(1.),
                                        rgb(0xd8d8e1),
                                        Default::default(),
                                    ));
                                }
                                let _ = window.paint_image(
                                    image_bounds,
                                    px(backdrop.map_or(0., |b| b.inner_radius as f32 * scale))
                                        .into(),
                                    image,
                                    0,
                                    false,
                                );
                                window.with_content_mask(
                                    Some(ContentMask {
                                        bounds: image_bounds.intersect(&bounds),
                                    }),
                                    |window| {
                                        for mark in &overlays {
                                            drawing::paint(mark, layout.get(), window, cx);
                                        }
                                        if let Some(mark) = &selected_mark
                                            && mark.tool == Tool::Arrow
                                        {
                                            arrow::paint_handles(mark, layout.get(), window);
                                        }
                                        if let Some((left, top, right, bottom)) = selection_bounds
                                            .filter(|_| {
                                                selected_mark
                                                    .as_ref()
                                                    .is_none_or(|m| m.tool != Tool::Arrow)
                                            })
                                        {
                                            let l = layout.get();
                                            let b = Bounds::new(
                                                point(
                                                    px(l.x + left * l.scale),
                                                    px(l.y + top * l.scale),
                                                ),
                                                size(
                                                    px((right - left) * l.scale),
                                                    px((bottom - top) * l.scale),
                                                ),
                                            )
                                            .dilate(px(4.));
                                            window.paint_quad(quad(
                                                b,
                                                px(3.),
                                                gpui::transparent_black(),
                                                px(1.),
                                                rgb(0x4c8dff),
                                                Default::default(),
                                            ));
                                        }
                                        text::paint(
                                            &text_entity,
                                            layout.get(),
                                            image_bounds,
                                            window,
                                            cx,
                                        );
                                    },
                                );
                                if let Some(b) = backdrop {
                                    backdrop::clip_output_corners(
                                        frame_bounds,
                                        b.outer_radius as f32 * scale,
                                        window,
                                    );
                                }
                            },
                        )
                        .h_full()
                        .flex_1()
                        .min_w_0(),
                    )
                    .when(self.backdrop_panel, |el| {
                        el.child(self.backdrop_controls(cx))
                    })
                    .when(self.enhance_panel, |el| el.child(self.enhance_controls(cx))),
            )
    }
}
fn main() {
    let application = Application::new().with_assets(icons::Icons);
    application.on_reopen(|cx| cx.activate(true));
    application.run(|cx: &mut App| {
        cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        menus::install(cx);
        let bounds = Bounds::centered(None, size(px(1220.), px(860.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(1050.), px(600.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Pachiri".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    window.on_window_should_close(cx, |_, cx| {
                        cx.hide();
                        false
                    });
                    let editor = Editor::new(cx);
                    editor.focus.focus(window);
                    editor
                })
            },
        )
        .expect("Unable to open the editor");
        cx.activate(true);
    });
}
