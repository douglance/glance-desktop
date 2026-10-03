use super::super::{
    Editor,
    state::Gesture,
    view::{HoverLabel, icon},
};
use crate::{
    animation::Motion,
    backdrop::{Backdrop, Control, Format, PRESETS},
};
use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy, PartialEq)]
pub(in crate::editor) enum Popup {
    Format,
    Export,
}

impl Editor {
    fn popup_count(&self, popup: Popup) -> usize {
        match popup {
            Popup::Format => Format::ALL.len(),
            Popup::Export if self.video_export.progress.is_some() => 1,
            Popup::Export => 3 + usize::from(self.video_export.last_video.is_some()),
        }
    }

    pub(in crate::editor) fn backdrop_slider(
        &self,
        control: Control,
        b: Backdrop,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
        let painted_bounds = bounds.clone();
        let value = control.value(b);
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .text_xs()
                    .child(
                        div()
                            .whitespace_nowrap()
                            .child(if control == Control::OuterRadius {
                                "Canvas corners"
                            } else {
                                control.label()
                            }),
                    )
                    .child(div().text_color(rgb(0x8a8d99)).child(format!(
                        "{value} {}",
                        if control == Control::Duration {
                            "s"
                        } else {
                            "px"
                        }
                    ))),
            )
            .child(
                div()
                    .id(SharedString::from(format!("backdrop-{}", control.label())))
                    .debug_selector(move || format!("backdrop-{}", control.label()))
                    .h(px(24.))
                    .w_full()
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            if this.is_busy() {
                                return;
                            }
                            this.commit_text(cx);
                            this.document.remember();
                            if this.document.backdrop.is_none() {
                                this.document.backdrop = Some(b);
                            }
                            this.cancel_gesture();
                            this.interaction.gesture =
                                Gesture::AdjustingBackdrop(control, bounds.get());
                            this.backdrop_slider_move(e.position, cx);
                        }),
                    )
                    .child(
                        canvas(
                            move |bounds, _, _| {
                                painted_bounds.set(bounds);
                                bounds
                            },
                            move |bounds, _, window, _| {
                                let y = bounds.center().y;
                                let x = bounds.left()
                                    + bounds.size.width
                                        * ((value - control.min()) as f32
                                            / (control.max() - control.min()) as f32);
                                let track = Bounds::new(
                                    point(bounds.left(), y - px(2.)),
                                    size(bounds.size.width, px(4.)),
                                );
                                window.paint_quad(quad(
                                    track,
                                    px(2.),
                                    rgb(0xe7e8ee),
                                    px(0.),
                                    rgb(0xe7e8ee),
                                    Default::default(),
                                ));
                                window.paint_quad(quad(
                                    Bounds::new(track.origin, size(x - bounds.left(), px(4.))),
                                    px(2.),
                                    rgb(0x32b49b),
                                    px(0.),
                                    rgb(0x32b49b),
                                    Default::default(),
                                ));
                                window.paint_quad(quad(
                                    Bounds::new(
                                        point(x - px(7.), y - px(7.)),
                                        size(px(14.), px(14.)),
                                    ),
                                    px(7.),
                                    rgb(0xffffff),
                                    px(1.),
                                    rgb(0xd3d6df),
                                    Default::default(),
                                ));
                            },
                        )
                        .size_full(),
                    ),
            )
    }
    fn open_popup(&mut self, popup: Popup, cx: &mut Context<Self>) {
        if self.is_busy() && self.video_export.progress.is_none() {
            return;
        }
        self.commit_text(cx);
        self.panels.popup = if self.panels.popup == Some(popup) {
            None
        } else {
            Some(popup)
        };
        self.panels.popup_index = match popup {
            Popup::Format => Format::ALL
                .iter()
                .position(|f| {
                    *f == self
                        .document
                        .backdrop
                        .or(self.panels.backdrop_disabled)
                        .unwrap_or_default()
                        .format
                })
                .unwrap_or(0),
            Popup::Export => 0,
        };
        cx.notify();
    }
    fn choose_popup(&mut self, popup: Popup, index: usize, cx: &mut Context<Self>) {
        self.panels.popup = None;
        match popup {
            Popup::Format => self.backdrop_style(|b| b.format = Format::ALL[index], cx),
            Popup::Export => match index {
                0 if self.video_export.progress.is_some() => self.cancel_video(cx),
                0 => self.export(true, cx),
                1 => self.export_video(cx),
                2 => self.export_gif(cx),
                _ => {
                    if let Some(path) = &self.video_export.last_video {
                        let _ = std::process::Command::new("/usr/bin/open")
                            .arg("-R")
                            .arg(path)
                            .spawn();
                    }
                }
            },
        }
        cx.notify();
    }
    pub(in crate::editor) fn popup_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Some(popup) = self.panels.popup else {
            return false;
        };
        match key {
            "escape" => self.panels.popup = None,
            "up" => {
                self.panels.popup_index = (self.panels.popup_index + self.popup_count(popup) - 1)
                    % self.popup_count(popup)
            }
            "down" => {
                self.panels.popup_index = (self.panels.popup_index + 1) % self.popup_count(popup)
            }
            "enter" => self.choose_popup(popup, self.panels.popup_index, cx),
            _ => {
                self.panels.popup = None;
                cx.notify();
                return false;
            }
        }
        cx.notify();
        true
    }
    fn dropdown(&self, popup: Popup, trigger: AnyElement, cx: &Context<Self>) -> impl IntoElement {
        let trigger_bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
        let painted_bounds = trigger_bounds.clone();
        let b = self
            .document
            .backdrop
            .or(self.panels.backdrop_disabled)
            .unwrap_or_default();
        div()
            .relative()
            .flex_1()
            .flex_shrink_0()
            .child(trigger)
            .child(
                canvas(
                    move |bounds, _, _| {
                        painted_bounds.set(bounds);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .when(self.panels.popup == Some(popup), |el| {
                el.child(
                    deferred(
                        anchored()
                            .position_mode(AnchoredPositionMode::Local)
                            .position(point(px(0.), px(34.)))
                            .snap_to_window()
                            .child(
                                div()
                                    .id("backdrop-popup")
                                    .debug_selector(|| "backdrop-popup".into())
                                    .occlude()
                                    .w(px(230.))
                                    .p_1()
                                    .flex()
                                    .flex_col()
                                    .rounded_lg()
                                    .shadow_md()
                                    .bg(rgb(0xffffff))
                                    .border_1()
                                    .border_color(rgb(0xdfe1e7))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|_, _, _, cx| cx.stop_propagation()),
                                    )
                                    .on_mouse_down_out(cx.listener(
                                        move |this, e: &MouseDownEvent, _, cx| {
                                            if !trigger_bounds.get().contains(&e.position)
                                                && this.panels.popup == Some(popup)
                                            {
                                                this.panels.popup = None;
                                                cx.notify();
                                            }
                                        },
                                    ))
                                    .children((0..self.popup_count(popup)).map(|index| {
                                        let label = match popup {
                                            Popup::Format => Format::ALL[index].label(),
                                            Popup::Export => match index {
                                                0 if self.video_export.progress.is_some() => {
                                                    "Cancel export"
                                                }
                                                0 => "Save PNG…",
                                                1 => "Export MP4…",
                                                2 => "Export GIF…",
                                                _ => "Show exported animation",
                                            },
                                        };
                                        div()
                                            .child(
                                                div()
                                                    .id(("popup-option", index))
                                                    .debug_selector(move || {
                                                        format!("popup-option-{index}")
                                                    })
                                                    .h(px(28.))
                                                    .px_2()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .rounded_md()
                                                    .text_xs()
                                                    .cursor_pointer()
                                                    .bg(rgb(if self.panels.popup_index == index {
                                                        0xe5f4f0
                                                    } else {
                                                        0xffffff
                                                    }))
                                                    .hover(|s| s.bg(rgb(0xe5f4f0)))
                                                    .child(div().w(px(12.)).child(
                                                        if popup == Popup::Format
                                                            && b.format == Format::ALL[index]
                                                        {
                                                            "✓"
                                                        } else {
                                                            ""
                                                        },
                                                    ))
                                                    .child(label)
                                                    .on_click(cx.listener(
                                                        move |this, _, _, cx| {
                                                            this.choose_popup(popup, index, cx)
                                                        },
                                                    )),
                                            )
                                            .when(popup == Popup::Format && index == 6, |el| {
                                                el.child(div().h(px(1.)).my_1().bg(rgb(0xe5e5ec)))
                                            })
                                    })),
                            ),
                    )
                    .with_priority(2),
                )
            })
    }
    pub(in crate::editor) fn export_menu(&self, cx: &Context<Self>) -> impl IntoElement {
        // A single toolbar control replaces the separate sidebar export buttons.
        div()
            .id("export-menu")
            .debug_selector(|| "export-menu".into())
            .flex_shrink_0()
            .w(px(40.))
            .child(
                self.dropdown(
                    Popup::Export,
                    div()
                        .id("export-trigger")
                        .debug_selector(|| "export-trigger".into())
                        .h(px(30.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap_1()
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(0xf0f1f5)))
                        .child(icon(
                            if self.video_export.progress.is_some() {
                                "square"
                            } else {
                                "save"
                            },
                            0x555966,
                        ))
                        .child("▾")
                        .tooltip(|_, cx| {
                            cx.new(|_| HoverLabel("Export PNG / MP4 / GIF · ⌘S saves PNG".into()))
                                .into()
                        })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.commit_text(cx);
                                cx.stop_propagation();
                            }),
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.open_popup(Popup::Export, cx)))
                        .into_any_element(),
                    cx,
                ),
            )
    }
    pub(in crate::editor) fn backdrop_controls(&self, cx: &Context<Self>) -> impl IntoElement {
        let b = self
            .document
            .backdrop
            .or(self.panels.backdrop_disabled)
            .unwrap_or_default();
        div()
            .id("backdrop-panel")
            .debug_selector(|| "backdrop-panel".into())
            .w(px(260.))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_3()
            .p_5()
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
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Backdrop"),
                    )
                    .child(self.button("Done", false, cx, |this, cx| {
                        this.panels.backdrop = false;
                        this.panels.popup = None;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .child("Format")
                    .child(
                        self.dropdown(
                            Popup::Format,
                            div()
                                .id("backdrop-format")
                                .debug_selector(|| "backdrop-format".into())
                                .h(px(32.))
                                .px_2()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_1()
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(0xdfe1e7))
                                .bg(rgb(0xffffff))
                                .cursor_pointer()
                                .child(b.format.short_label())
                                .child("▾")
                                .on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.open_popup(Popup::Format, cx)
                                    }),
                                )
                                .into_any_element(),
                            cx,
                        ),
                    ),
            )
            .child(
                div()
                    .id("backdrop-effects")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(
                        [
                            [Control::Padding, Control::Shadow],
                            [Control::InnerRadius, Control::OuterRadius],
                        ]
                        .into_iter()
                        .map(|row| {
                            div()
                                .flex()
                                .gap_3()
                                .children(row.into_iter().map(|control| {
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .child(self.backdrop_slider(control, b, cx))
                                }))
                        }),
                    ),
            )
            .child(div().h(px(1.)).bg(rgb(0xe5e5ec)))
            .child(div().text_xs().child("Background"))
            .child(
                div()
                    .flex()
                    .gap_1()
                    .p_1()
                    .rounded_md()
                    .bg(rgb(0xeff0f4))
                    .children(
                        [("Solid", false), ("Gradient", true), ("Motion", true)]
                            .into_iter()
                            .map(|(label, gradient)| {
                                let moving = label == "Motion";
                                let active = if moving {
                                    b.motion != Motion::Still
                                } else {
                                    b.motion == Motion::Still && b.gradient == gradient
                                };
                                div()
                                    .id(label)
                                    .debug_selector(move || format!("backdrop-mode-{label}"))
                                    .flex_1()
                                    .h(px(30.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .text_xs()
                                    .cursor_pointer()
                                    .bg(rgb(if active { 0xffffff } else { 0xeff0f4 }))
                                    .text_color(rgb(if active { 0x292d37 } else { 0x646976 }))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.backdrop_style(
                                            |b| {
                                                b.gradient = gradient;
                                                b.motion = if moving {
                                                    if b.motion == Motion::Still {
                                                        Motion::Flow
                                                    } else {
                                                        b.motion
                                                    }
                                                } else {
                                                    Motion::Still
                                                };
                                            },
                                            cx,
                                        )
                                    }))
                                    .child(label)
                            }),
                    ),
            )
            .when(b.motion != Motion::Still, |el| {
                el.child(
                    div()
                        .id("backdrop-motion-effects")
                        .debug_selector(|| "backdrop-motion-effects".into())
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .children(Motion::EFFECTS.into_iter().map(|motion| {
                            div().w(px(101.)).child(self.button(
                                motion.label(),
                                b.motion == motion,
                                cx,
                                move |this, cx| {
                                    this.backdrop_style(
                                        |b| {
                                            if b.motion != motion
                                                && let Some(preset) = motion.suggested_preset()
                                            {
                                                b.preset = preset;
                                            }
                                            b.motion = motion;
                                        },
                                        cx,
                                    )
                                },
                            ))
                        })),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .children(PRESETS.iter().enumerate().map(|(i, &(name, _, _))| {
                        let style = Backdrop { preset: i, ..b };
                        div()
                            .id(("backdrop-preset", i))
                            .size(px(45.))
                            .rounded_md()
                            .border_2()
                            .border_color(rgb(if i == b.preset { 0x147d6d } else { 0xe5e6eb }))
                            .bg(style.background())
                            .cursor_pointer()
                            .tooltip(move |_, cx| cx.new(|_| HoverLabel(name.into())).into())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.backdrop_style(|b| b.preset = i, cx)
                            }))
                    })),
            )
            .when(b.motion != Motion::Still, |el| {
                el.child(
                    div()
                        .id("backdrop-duration")
                        .debug_selector(|| "backdrop-duration".into())
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .child(self.backdrop_slider(Control::Duration, b, cx)),
                        )
                        .child(self.compact_button(
                            if self.playback.paused {
                                "Play"
                            } else {
                                "Pause"
                            },
                            if self.playback.paused {
                                "play"
                            } else {
                                "pause"
                            },
                            false,
                            cx,
                            |this, cx| this.toggle_animation(cx),
                        )),
                )
            })
            .when_some(self.video_export.progress, |el, progress| {
                el.child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x646976))
                        .child(format!("Exporting… {progress}% · Escape to cancel")),
                )
            })
            .child(div().h(px(1.)).bg(rgb(0xe5e5ec)))
            .child(
                div()
                    .id("backdrop-enabled")
                    .debug_selector(|| "backdrop-enabled".into())
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .cursor_pointer()
                    .child(
                        div()
                            .size(px(14.))
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(0x147d6d))
                            .bg(rgb(if self.document.backdrop.is_some() {
                                0x147d6d
                            } else {
                                0xffffff
                            }))
                            .text_color(rgb(0xffffff))
                            .child(if self.document.backdrop.is_some() {
                                "✓"
                            } else {
                                ""
                            }),
                    )
                    .child("Enable backdrop")
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.is_busy() {
                            return;
                        }
                        this.document.remember();
                        if let Some(b) = this.document.backdrop.take() {
                            this.panels.backdrop_disabled = Some(b);
                        } else {
                            this.document.backdrop =
                                Some(this.panels.backdrop_disabled.take().unwrap_or_default());
                        }
                        cx.notify();
                    })),
            )
    }
}
