use super::super::{
    Editor,
    actions::{Action, AnimationFormat, Panel},
    state::Gesture,
    view::icon,
};
use crate::animation::{AnimationControl, Entrance, Motion};
use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc};

impl Editor {
    fn animation_slider(&self, control: AnimationControl, cx: &Context<Self>) -> impl IntoElement {
        let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
        let painted = bounds.clone();
        let a = self.document.image_animation;
        let value = control.value(a, self.clip_time());
        let (min, max, _) = control.range(a);
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_xs()
                    .child(control.label())
                    .child(
                        div()
                            .text_color(rgb(0x646976))
                            .child(control.display(value)),
                    ),
            )
            .child(
                div()
                    .id(SharedString::from(format!("animation-{}", control.label())))
                    .debug_selector(move || format!("animation-{}", control.label()))
                    .h(px(26.))
                    .w_full()
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            let track = bounds.get();
                            this.dispatch_ui(
                                Action::BeginAnimationAdjustment {
                                    control,
                                    track: (
                                        f32::from(track.origin.x),
                                        f32::from(track.origin.y),
                                        f32::from(track.size.width),
                                        f32::from(track.size.height),
                                    ),
                                    position: (f32::from(e.position.x), f32::from(e.position.y)),
                                },
                                cx,
                            );
                        }),
                    )
                    .child(
                        canvas(
                            move |b, _, _| {
                                painted.set(b);
                                b
                            },
                            move |b, _, window, _| {
                                let fraction = ((value.saturating_sub(min)) as f32
                                    / (max - min).max(1) as f32)
                                    .clamp(0., 1.);
                                let x = b.left() + b.size.width * fraction;
                                let y = b.center().y;
                                window.paint_quad(quad(
                                    Bounds::new(
                                        point(b.left(), y - px(2.)),
                                        size(b.size.width, px(4.)),
                                    ),
                                    px(2.),
                                    rgb(0xe7e8ee),
                                    px(0.),
                                    rgb(0xe7e8ee),
                                    Default::default(),
                                ));
                                window.paint_quad(quad(
                                    Bounds::new(
                                        point(b.left(), y - px(2.)),
                                        size(x - b.left(), px(4.)),
                                    ),
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
    pub(in crate::editor) fn animation_controls(&self, cx: &Context<Self>) -> impl IntoElement {
        let a = self.document.image_animation;
        let motion = self.document.backdrop.map_or(Motion::Still, |b| b.motion);
        let finished = self.clip_time() >= a.seconds as f32;
        super::controls::panel("animation-panel", cx)
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
                            .child(icon("play", 0x147d6d))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Animation"),
                            ),
                    )
                    .child(self.button(
                        "Done",
                        false,
                        cx,
                        Action::ClosePanel {
                            panel: Panel::Animation,
                        },
                    )),
            )
            .child(div().text_xs().child("Image entrance"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .children(Entrance::ALL.into_iter().map(|effect| {
                        let name = match effect {
                            Entrance::None => "square",
                            Entrance::Diagonal => "arrow-up-right",
                            Entrance::Pop => "maximize",
                            Entrance::Tilt => "rotate-cw",
                        };
                        div().w(px(110.)).child(
                            self.choice(
                                format!("entrance-{effect:?}"),
                                effect.label().into(),
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(icon(
                                        name,
                                        if a.effect == effect {
                                            0xd94d38
                                        } else {
                                            0x555966
                                        },
                                    ))
                                    .child(effect.label())
                                    .into_any_element(),
                                (a.effect == effect, true),
                                Action::SelectEntrance { effect },
                                cx,
                            ),
                        )
                    })),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.button("Replay", false, cx, Action::ReplayAnimation))
                    .child(self.button(
                        if self.playback.paused || finished {
                            "Play"
                        } else {
                            "Pause"
                        },
                        false,
                        cx,
                        Action::TogglePlayback,
                    )),
            )
            .child(self.animation_slider(AnimationControl::Time, cx))
            .child(div().text_xs().text_color(rgb(0x646976)).child(format!(
                "{:.1} / {} s",
                self.clip_time(),
                a.seconds
            )))
            .child(super::controls::pair(
                self.animation_slider(AnimationControl::Duration, cx),
                self.animation_slider(AnimationControl::Delay, cx),
            ))
            .child(self.animation_slider(AnimationControl::Length, cx))
            .child(super::controls::field(
                "After entrance",
                div()
                    .flex()
                    .gap_1()
                    .child(self.choice(
                        "animation-hold".into(),
                        "Enter and hold".into(),
                        div().child("Hold").into_any_element(),
                        (!a.exit, true),
                        Action::SetImageAnimation {
                            animation: crate::animation::ImageAnimation { exit: false, ..a },
                        },
                        cx,
                    ))
                    .child(self.choice(
                        "animation-exit".into(),
                        "Enter, hold and exit".into(),
                        div().child("Exit").into_any_element(),
                        (a.exit, true),
                        Action::SetImageAnimation {
                            animation: crate::animation::ImageAnimation { exit: true, ..a },
                        },
                        cx,
                    )),
            ))
            .child(div().h(px(1.)).bg(rgb(0xe5e5ec)))
            .child(div().text_xs().child("Backdrop motion"))
            .child(
                div().flex().flex_wrap().gap_2().children(
                    std::iter::once(Motion::Still)
                        .chain(Motion::EFFECTS)
                        .map(|motion_choice| {
                            self.motion_button(motion_choice, motion == motion_choice, cx)
                        }),
                ),
            )
            .child(self.button("Backdrop style", false, cx, Action::ToggleBackdrop))
            .child(div().h(px(1.)).bg(rgb(0xe5e5ec)))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.button(
                        "MP4",
                        false,
                        cx,
                        Action::ExportAnimation {
                            format: AnimationFormat::Mp4,
                        },
                    ))
                    .child(self.button(
                        "GIF",
                        false,
                        cx,
                        Action::ExportAnimation {
                            format: AnimationFormat::Gif,
                        },
                    )),
            )
            .when_some(self.video_export.progress, |el, progress| {
                el.child(
                    div()
                        .text_xs()
                        .child(format!("Exporting… {progress}% · Escape to cancel")),
                )
            })
    }
    pub(in crate::editor) fn animation_slider_move(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Gesture::AdjustingAnimation(control, bounds) = self.interaction.gesture else {
            return false;
        };
        let (min, max, step) = control.range(self.document.image_animation);
        let ratio =
            (f32::from(position.x - bounds.left()) / f32::from(bounds.size.width)).clamp(0., 1.);
        let value = (min as f32 + ((max - min) as f32 * ratio / step as f32).round() * step as f32)
            .round() as u32;
        self.dispatch_ui(
            Action::SetAnimationControl {
                control,
                value: value.clamp(min, max),
            },
            cx,
        );
        true
    }
}
