use crate::{Editor, HoverLabel, Message, icon, preview_base, render_image};
use gpui::{prelude::*, *};

impl Editor {
    pub fn toggle_enhance(&mut self, cx: &mut Context<Self>) {
        self.commit_text(cx);
        self.enhance_panel = !self.enhance_panel;
        if self.enhance_panel {
            self.backdrop_panel = false;
        }
        cx.notify();
    }
    pub fn resize_image(&mut self, rotate: bool, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.commit_text(cx);
        self.draft = None;
        self.busy = true;
        let mut document = self.document.clone();
        let scale = self.resize_scale;
        let smart = self.resize_smart;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = if rotate {
                document.rotate();
                Ok(())
            } else {
                document.resize(scale, smart)
            };
            let result = result.map(|()| {
                let count = document.marks.len();
                let preview = render_image(preview_base(&document));
                (document, count, preview)
            });
            let _ = sender.send_blocking(Message::Transformed(result));
        });
        cx.notify();
    }
    pub fn paste_image(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.commit_text(cx);
        self.busy = true;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let _ =
                sender.send_blocking(Message::Image(crate::platform::clipboard_image().map(Some)));
        });
        cx.notify();
    }
    pub fn enhance_controls(&self, cx: &Context<Self>) -> impl IntoElement {
        let target = crate::enhance::dimensions(self.document.base.dimensions(), self.resize_scale);
        div()
            .id("image-panel")
            .w(px(260.))
            .h_full()
            .flex_shrink_0()
            .p_5()
            .flex()
            .flex_col()
            .gap_4()
            .overflow_y_scroll()
            .bg(rgb(0xfcfcfd))
            .border_l_1()
            .border_color(rgb(0xe5e5ec))
            .cursor(CursorStyle::Arrow)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.commit_text(cx);
                    cx.stop_propagation();
                }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(icon("sparkles", 0xf35d45))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Image tools"),
                            ),
                    )
                    .child(self.button("Done", false, cx, |this, cx| {
                        this.enhance_panel = false;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Resize"),
            )
            .child(div().flex().flex_wrap().gap_2().children(
                [0.5_f32, 1., 1.5, 2., 3., 4.].into_iter().map(|scale| {
                    div()
                        .id(("resize-scale", (scale * 100.) as u32))
                        .w(px(66.))
                        .h(px(32.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .text_xs()
                        .cursor_pointer()
                        .bg(rgb(if self.resize_scale == scale {
                            0xffe9e4
                        } else {
                            0xf0f1f5
                        }))
                        .text_color(rgb(if self.resize_scale == scale {
                            0xd94d38
                        } else {
                            0x555966
                        }))
                        .child(format!("{}%", (scale * 100.) as u32))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.resize_scale = scale;
                            cx.notify();
                        }))
                }),
            ))
            .child(
                div()
                    .id("smart-upscale")
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .text_xs()
                    .child(
                        div()
                            .size(px(16.))
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(0xd5d8e0))
                            .bg(rgb(if self.resize_smart {
                                0xf35d45
                            } else {
                                0xffffff
                            }))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(0xffffff))
                            .child(if self.resize_smart { "✓" } else { "" }),
                    )
                    .child("Smart upscale")
                    .tooltip(|_, cx| {
                        cx.new(|_| {
                            HoverLabel("Local adaptive sharpening; no AI model or uploads".into())
                        })
                        .into()
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.resize_smart = !this.resize_smart;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x878b98))
                    .child("Sharper edges when enlarging. Annotations redraw at full resolution."),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x555966))
                    .child(match target {
                        Ok((w, h)) => format!(
                            "{}×{} → {w}×{h}",
                            self.document.base.width(),
                            self.document.base.height()
                        ),
                        Err(_) => "Choose a smaller scale".into(),
                    }),
            )
            .child(self.button(
                if self.busy {
                    "Working…"
                } else {
                    "Apply resize"
                },
                true,
                cx,
                |this, cx| this.resize_image(false, cx),
            ))
            .child(div().h(px(1.)).bg(rgb(0xe5e5ec)))
            .child(self.button("Rotate 90°", false, cx, |this, cx| {
                this.resize_image(true, cx)
            }))
            .child(self.button("Paste image  ⌘V", false, cx, |this, cx| {
                this.paste_image(cx)
            }))
    }
}
