use super::*;
pub(super) struct HoverLabel(pub(super) SharedString);
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
pub(super) fn icon(name: &'static str, color: u32) -> impl IntoElement {
    svg()
        .path(format!("icons/{name}.svg"))
        .size(px(18.))
        .flex_shrink_0()
        .text_color(rgb(color))
}
impl Editor {
    pub(super) fn tool_button(&self, tool: Tool, cx: &Context<Self>) -> impl IntoElement {
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
        let active = self.interaction.tool == tool;
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
    pub(super) fn compact_button(
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
    pub(super) fn button(
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
            self.viewport.space_down = false;
            self.viewport.zoom_down = false;
            self.end_pan();
        }
        for image in self.preview.retired.drain(..) {
            let _ = window.drop_image(image);
        }
        let dimensions = self.document.base.dimensions();
        let output_dimensions = self
            .document
            .backdrop
            .map_or(dimensions, |b| b.dimensions(dimensions));
        let viewport = window.viewport_size();
        let fit_zoom = ((f32::from(viewport.width)
            - if self.panels.backdrop || self.panels.enhance {
                260.
            } else {
                0.
            }
            - 80.)
            / output_dimensions.0 as f32)
            .min((f32::from(viewport.height) - 48. - 70.) / output_dimensions.1 as f32)
            .clamp(0.01, 1.);
        let zoom_label = format!("{:.0}%", self.viewport.zoom.unwrap_or(fit_zoom) * 100.);
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
            .relative()
            .flex()
            .flex_col()
            .bg(rgb(0xfcfcfd))
            .text_color(rgb(0x272831))
            .font_family(".AppleSystemUIFont")
            .track_focus(&self.focus)
            .key_context(if self.interaction.text_edit.is_some() {
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
                        this.viewport.space_down = false;
                        this.end_pan();
                    }
                    "z" => this.viewport.zoom_down = false,
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
                cx.listener(|this, _, _, _| this.end_pan()),
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
                        self.panels.backdrop,
                        cx,
                        |this, cx| this.toggle_backdrop(cx),
                    ))
                    .child(self.compact_button(
                        "Image tools",
                        "sparkles",
                        self.panels.enhance,
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
                                    .border_color(rgb(if self.interaction.color == color {
                                        0xf35d45
                                    } else {
                                        0xd7d7df
                                    }))
                                    .bg(rgb(hex))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.interaction.color = color;
                                        this.apply_style(true, cx);
                                    }))
                            }))
                            .child(self.button(
                                &format!("{} px", self.interaction.width as u32),
                                false,
                                cx,
                                |this, cx| {
                                    this.interaction.width = match this.interaction.width as u32 {
                                        3 => 5.,
                                        5 => 9.,
                                        _ => 3.,
                                    };
                                    this.apply_style(false, cx);
                                },
                            )),
                    )
                    .child(div().flex_1())
                    .child(self.compact_button(
                        "Copy · ⌘C",
                        if self.feedback.copy == Some(CopyFeedback::Copied) {
                            "check"
                        } else {
                            "copy"
                        },
                        false,
                        cx,
                        |this, cx| this.export(false, cx),
                    ))
                    .child(
                        div()
                            .id("copy-remote")
                            .debug_selector(|| "copy-remote".into())
                            .flex_shrink_0()
                            .child(self.compact_button(
                                "Copy (remote) · Upload to Glance · ⌘⇧C",
                                if matches!(self.feedback.copy, Some(CopyFeedback::LinkCopied(_))) {
                                    "check"
                                } else {
                                    "cloud-upload"
                                },
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
                                        this.viewport.zoom = None;
                                        this.viewport.pan = (0., 0.);
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(self.canvas(window, cx))
            .when_some(self.feedback.copy, |el, feedback| {
                el.child(self.copy_confirmation(feedback))
            })
    }
}
