use super::*;
impl Editor {
    pub(super) fn outside_text(&mut self, e: &MouseDownEvent, cx: &mut Context<Self>) {
        let layout = self.viewport.layout.get();
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
        let layout = self.viewport.layout.get();
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
        if self.viewport.space_down {
            self.begin_pan(e.position, cx);
            return;
        }
        if self.viewport.zoom_down {
            self.zoom_at(
                if e.modifiers.shift { 0.5 } else { 2. },
                (f32::from(e.position.x), f32::from(e.position.y)),
                cx,
            );
            return;
        }
        if let Some(edit) = &mut self.interaction.text_edit
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
        if self.interaction.tool == Tool::Crop {
            p = navigation::endpoint(Tool::Crop, p, p, false, self.viewport.layout.get());
        }
        if let Some(index) = self
            .interaction
            .selected
            .filter(|i| *i < self.document.marks.len())
        {
            let mark = &self.document.marks[index];
            let handle = arrow::handle_at(mark, p, 7. / self.viewport.layout.get().scale);
            if handle.is_some() || mark.hit(p, 5. / self.viewport.layout.get().scale) {
                self.start_annotation_drag(index, p, handle);
                cx.notify();
                return;
            }
        }
        self.interaction.selected = None;
        if self.interaction.tool == Tool::Select {
            self.interaction.selected =
                self.document.pick(p, 5. / self.viewport.layout.get().scale);
            if let Some(index) = self.interaction.selected {
                self.interaction.color = self.document.marks[index].color;
                self.interaction.width = self.document.marks[index].width;
                self.start_annotation_drag(index, p, None);
            }
            cx.notify();
            return;
        }
        let mut mark = Mark {
            tool: self.interaction.tool,
            curve: None,
            points: vec![p],
            color: self.interaction.color,
            width: match self.interaction.tool {
                Tool::Text => self.interaction.width.max(20. / 7.),
                Tool::Counter => self.interaction.width.max(20. / 4.4),
                _ => self.interaction.width,
            },
            text: String::new(),
        };
        if self.interaction.tool == Tool::Counter {
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
            self.interaction.selected = self.document.marks.len().checked_sub(1);
            self.changed();
            cx.notify();
            return;
        }
        if self.interaction.tool == Tool::Text {
            self.interaction.text_edit = Some(text::Edit::new(mark));
            self.interaction.text_session += 1;
            let session = self.interaction.text_session;
            self.feedback.status = "Type your label • Enter to finish • Escape to cancel".into();
            cx.spawn(async move |view, cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(500))
                        .await;
                    let keep = view
                        .update(cx, |editor, cx| {
                            if editor.interaction.text_session != session {
                                return false;
                            }
                            if let Some(edit) = &mut editor.interaction.text_edit {
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
            self.interaction.gesture = Gesture::Drawing(mark);
        }
        cx.notify();
    }
    pub(super) fn motion(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.backdrop_slider_move(e.position, cx) {
            return;
        }
        if let Gesture::Panning(previous) = &mut self.interaction.gesture {
            self.viewport.pan.0 += f32::from(e.position.x - previous.x);
            self.viewport.pan.1 += f32::from(e.position.y - previous.y);
            *previous = e.position;
            cx.notify();
            return;
        }
        if let Some(edit) = &mut self.interaction.text_edit
            && edit.selecting
        {
            edit.buffer.move_to(edit.index(e.position), true);
            edit.caret_on = true;
            cx.notify();
            return;
        }
        let Some(p) = self.coordinate(e.position, true) else {
            return;
        };
        match &mut self.interaction.gesture {
            Gesture::MovingAnnotation(drag) => drag.update(p, e.modifiers.shift, None),
            Gesture::EditingArrow { drag, handle } => {
                drag.update(p, e.modifiers.shift, Some(*handle))
            }
            Gesture::Drawing(mark) => {
                if mark.tool == Tool::Pen {
                    if let Some(last) = mark.points.last()
                        && (p.0 - last.0).hypot(p.1 - last.1) * self.viewport.layout.get().scale
                            < 0.35
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
                        self.viewport.layout.get(),
                    ));
                }
            }
            _ => return,
        }
        cx.notify();
    }
    pub(super) fn finish(&mut self, e: &MouseUpEvent, cx: &mut Context<Self>) {
        if let Some(edit) = &mut self.interaction.text_edit {
            edit.selecting = false;
        }
        let gesture = std::mem::take(&mut self.interaction.gesture);
        let (mut drag, handle) = match gesture {
            Gesture::MovingAnnotation(drag) => (drag, None),
            Gesture::EditingArrow { drag, handle } => (drag, Some(handle)),
            Gesture::Drawing(mut mark) => {
                if let Some(p) = self.coordinate(e.position, true) {
                    mark.points.push(navigation::endpoint(
                        mark.tool,
                        mark.points[0],
                        p,
                        e.modifiers.shift,
                        self.viewport.layout.get(),
                    ));
                }
                if mark.tool == Tool::Arrow && mark.points.len() > 2 {
                    let end = *mark.points.last().unwrap();
                    mark.points.truncate(1);
                    mark.points.push(end);
                }
                if mark.tool == Tool::Crop {
                    self.busy = true;
                    self.feedback.status = "Cropping…".into();
                    let mut document = self.document.clone();
                    let sender = self.sender.clone();
                    std::thread::spawn(move || {
                        document.commit(mark);
                        let image = render_image(preview_base(&document));
                        let _ = sender.send_blocking(Message::Cropped(document, image));
                    });
                } else {
                    self.document.commit(mark);
                    self.interaction.selected = self.document.marks.len().checked_sub(1);
                    self.changed();
                }
                cx.notify();
                return;
            }
            Gesture::Idle => return,
            Gesture::Panning(_) | Gesture::AdjustingBackdrop(..) => {
                cx.notify();
                return;
            }
        };
        if let Some(p) = self.coordinate(e.position, true) {
            drag.update(p, e.modifiers.shift, handle);
        }
        if let Some(original) = self.document.marks.get(drag.index)
            && (drag.moved.points != original.points || drag.moved.curve != original.curve)
        {
            self.document.remember();
            self.document.marks[drag.index] = drag.moved;
        }
        self.changed();
        cx.notify();
    }
    pub(super) fn start_annotation_drag(
        &mut self,
        index: usize,
        origin: (f32, f32),
        handle: Option<usize>,
    ) {
        let original = self.document.marks[index].clone();
        let drag = AnnotationDrag {
            index,
            origin,
            moved: original.clone(),
            original,
        };
        self.interaction.gesture = match handle {
            Some(handle) => Gesture::EditingArrow { drag, handle },
            None => Gesture::MovingAnnotation(drag),
        };
        self.changed();
    }
    pub(super) fn begin_pan(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.cancel_gesture();
        self.interaction.gesture = Gesture::Panning(position);
        cx.notify();
    }
    pub(super) fn end_pan(&mut self) {
        if matches!(self.interaction.gesture, Gesture::Panning(_)) {
            self.interaction.gesture = Gesture::Idle;
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
        if key == "escape" && self.video_export.cancel.is_some() {
            self.cancel_video(cx);
            cx.stop_propagation();
            return;
        }
        let m = e.keystroke.modifiers;
        if self.interaction.text_edit.is_some() {
            if matches!(key, "enter" | "escape")
                && self
                    .interaction
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
                    self.interaction.text_edit = None;
                    self.feedback.status = "Text canceled".into();
                    cx.notify();
                    cx.stop_propagation();
                    return;
                }
                _ => {}
            }
            if m.platform && (matches!(key, "s" | "o" | "q") || (m.shift && key == "c")) {
                self.commit_text(cx);
            } else {
                let edit = self.interaction.text_edit.as_mut().unwrap();
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
                self.viewport.space_down = true;
                cx.notify();
                cx.stop_propagation();
                return;
            }
            if key == "z" {
                self.viewport.zoom_down = true;
                cx.notify();
                cx.stop_propagation();
                return;
            }
            if matches!(key, "left" | "right" | "up" | "down")
                && self.interaction.selected.is_some()
                && !self.busy
            {
                self.cancel_gesture();
                if let Some(index) = self.interaction.selected {
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
                    self.viewport.zoom = None;
                    self.viewport.pan = (0., 0.);
                    cx.notify();
                }
                "0" => {
                    self.viewport.zoom = Some(1.);
                    self.viewport.pan = (0., 0.);
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
                    self.cancel_gesture();
                    self.interaction.selected = None;
                    self.interaction.gesture = Gesture::Idle;
                    cx.notify();
                }
                _ => {}
            }
        }
        cx.stop_propagation();
    }
    pub(super) fn zoom_at(&mut self, factor: f32, anchor: (f32, f32), cx: &mut Context<Self>) {
        if self.interaction.gesture.is_active() {
            return;
        }
        let bounds = self.viewport.canvas_bounds.get();
        let center = (
            f32::from(bounds.origin.x + bounds.size.width * 0.5),
            f32::from(bounds.origin.y + bounds.size.height * 0.5),
        );
        if let Some((zoom, pan)) = navigation::anchored_zoom(
            self.viewport
                .zoom
                .unwrap_or(self.viewport.layout.get().scale),
            self.viewport.pan,
            center,
            anchor,
            factor,
        ) {
            self.viewport.zoom = Some(zoom);
            self.viewport.pan = pan;
            cx.notify();
        }
    }
    pub(super) fn change_zoom(&mut self, factor: f32, cx: &mut Context<Self>) {
        let b = self.viewport.canvas_bounds.get();
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
        if self.interaction.gesture.is_active() {
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
                self.viewport.pan.0 += f32::from(delta.y);
            } else {
                self.viewport.pan.0 += f32::from(delta.x);
                self.viewport.pan.1 += f32::from(delta.y);
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
        let Gesture::AdjustingBackdrop(control, bounds) = self.interaction.gesture else {
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
                self.playback.position = phase * b.seconds as f32;
                self.playback.epoch = std::time::Instant::now();
            }
            cx.notify();
        }
        true
    }
}
