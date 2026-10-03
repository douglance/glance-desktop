use super::*;
impl Editor {
    pub(super) fn outside_text(&mut self, e: &MouseDownEvent, cx: &mut Context<Self>) {
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
    pub(super) fn coordinate(&self, p: Point<Pixels>, clamp: bool) -> Option<(f32, f32)> {
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
    pub(super) fn begin(
        &mut self,
        e: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
    pub(super) fn motion(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
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
    pub(super) fn finish(&mut self, e: &MouseUpEvent, cx: &mut Context<Self>) {
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
    pub(super) fn menu_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let e = KeyDownEvent {
            keystroke: Keystroke::parse(key).expect("valid menu shortcut"),
            is_held: false,
        };
        self.key(&e, window, cx);
    }
    pub(super) fn key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
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
    pub(super) fn zoom_at(&mut self, factor: f32, anchor: (f32, f32), cx: &mut Context<Self>) {
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
    pub(super) fn change_zoom(&mut self, factor: f32, cx: &mut Context<Self>) {
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
    pub(super) fn scroll(&mut self, e: &ScrollWheelEvent, cx: &mut Context<Self>) {
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
    pub(super) fn backdrop_slider_move(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((control, bounds)) = self.backdrop_drag else {
            return false;
        };
        let phase = self.animation_phase();
        if let Some(b) = &mut self.document.backdrop {
            let ratio = f32::from(position.x - bounds.left()) / f32::from(bounds.size.width);
            control.set(
                b,
                control.min()
                    + (ratio.clamp(0., 1.) * (control.max() - control.min()) as f32).round() as u32,
            );
            if control == Control::Duration {
                self.animation_position = phase * b.seconds as f32;
                self.animation_epoch = std::time::Instant::now();
            }
            cx.notify();
        }
        true
    }
}
