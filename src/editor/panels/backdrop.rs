use super::super::Editor;
use super::super::{
    state::Gesture,
    view::{HoverLabel, icon},
};
use crate::{
    animation::Motion,
    backdrop::{Backdrop, Control, PRESETS},
};
use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc};
impl Editor {
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
            .gap_2()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_xs()
                    .child(control.label())
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
    pub(in crate::editor) fn backdrop_controls(&self, cx: &Context<Self>) -> impl IntoElement {
        let b = self.document.backdrop.unwrap_or_default();
        div()
            .id("backdrop-panel")
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
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(icon("square", 0x32a68e))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Backdrop"),
                            ),
                    )
                    .child(self.button("Done", false, cx, |this, cx| {
                        this.panels.backdrop = false;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex()
                    .gap_1()
                    .p_1()
                    .rounded_md()
                    .bg(rgb(0xeff0f4))
                    .children([("Solid", false), ("Gradient", true)].into_iter().map(
                        |(label, gradient)| {
                            div()
                                .id(label)
                                .flex_1()
                                .h(px(30.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_md()
                                .text_xs()
                                .cursor_pointer()
                                .bg(rgb(
                                    if b.gradient == gradient && b.motion == Motion::Still {
                                        0xffffff
                                    } else {
                                        0xeff0f4
                                    },
                                ))
                                .text_color(rgb(
                                    if b.gradient == gradient && b.motion == Motion::Still {
                                        0x292d37
                                    } else {
                                        0x858995
                                    },
                                ))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.backdrop_style(
                                        |b| {
                                            b.gradient = gradient;
                                            b.motion = Motion::Still;
                                        },
                                        cx,
                                    )
                                }))
                                .child(label)
                        },
                    ))
                    .child(
                        div()
                            .id("motion-tab")
                            .flex_1()
                            .h(px(30.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .text_xs()
                            .cursor_pointer()
                            .bg(rgb(if b.motion != Motion::Still {
                                0xffffff
                            } else {
                                0xeff0f4
                            }))
                            .text_color(rgb(if b.motion != Motion::Still {
                                0x292d37
                            } else {
                                0x858995
                            }))
                            .child("Motion")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.backdrop_style(
                                    |b| {
                                        b.motion = Motion::Flow;
                                        b.gradient = true;
                                    },
                                    cx,
                                )
                            })),
                    ),
            )
            .when(b.motion != Motion::Still, |el| {
                el.child(div().flex().flex_wrap().gap_2().children(
                    Motion::EFFECTS.into_iter().map(|motion| {
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
                    }),
                ))
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
                            .border_color(rgb(
                                if i == b.preset && self.document.backdrop.is_some() {
                                    0x292d37
                                } else {
                                    0xe5e6eb
                                },
                            ))
                            .bg(style.background())
                            .cursor_pointer()
                            .tooltip(move |_, cx| cx.new(|_| HoverLabel(name.into())).into())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.backdrop_style(|b| b.preset = i, cx)
                            }))
                    })),
            )
            .when(b.motion != Motion::Still, |el| {
                el.child(div().h(px(1.)).bg(rgb(0xe5e6eb)))
                    .child(self.backdrop_slider(Control::Duration, b, cx))
                    .child(
                        div()
                            .flex()
                            .gap_2()
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
                            ))
                            .child(self.button(
                                if self.video_export.progress.is_some() {
                                    "Cancel export"
                                } else {
                                    "MP4…"
                                },
                                true,
                                cx,
                                |this, cx| {
                                    if this.video_export.progress.is_some() {
                                        this.cancel_video(cx);
                                    } else {
                                        this.export_video(cx);
                                    }
                                },
                            ))
                            .when(self.video_export.progress.is_none(), |el| {
                                el.child(
                                    self.button("GIF…", false, cx, |this, cx| {
                                        this.export_gif(cx)
                                    }),
                                )
                            }),
                    )
                    .child(
                        div().text_xs().text_color(rgb(0x878b98)).child(
                            self.video_export
                                .progress
                                .map_or("Loop · MP4 30 fps · GIF 20 fps".into(), |p| {
                                    format!("Exporting animation… {p}%")
                                }),
                        ),
                    )
            })
            .child(self.backdrop_slider(Control::Padding, b, cx))
            .child(self.backdrop_slider(Control::Shadow, b, cx))
            .child(self.backdrop_slider(Control::InnerRadius, b, cx))
            .child(self.backdrop_slider(Control::OuterRadius, b, cx))
            .when(self.video_export.last_video.is_some(), |el| {
                el.child(
                    self.button("Show exported animation", false, cx, |this, _| {
                        if let Some(path) = &this.video_export.last_video {
                            let _ = std::process::Command::new("/usr/bin/open")
                                .arg("-R")
                                .arg(path)
                                .spawn();
                        }
                    }),
                )
            })
            .child(div().flex_1())
            .child(self.button("Remove backdrop", false, cx, |this, cx| {
                if this.is_busy() {
                    return;
                }
                if this.document.backdrop.is_some() {
                    this.document.remember();
                    this.document.backdrop = None;
                    this.feedback.status = "Backdrop removed • ⌘Z to restore".into();
                }
                this.panels.backdrop = false;
                cx.notify();
            }))
    }
}
