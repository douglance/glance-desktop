use super::super::{Editor, actions::Action};
use crate::{
    document::{Mark, Tool},
    style::{Cleanup, Dash, End, Fill},
};
use gpui::{prelude::*, *};
impl Editor {
    fn option_button(
        &self,
        id: String,
        label: String,
        active: bool,
        enabled: bool,
        action: Action,
        cx: &Context<Self>,
    ) -> AnyElement {
        let debug_id = id.clone();
        div()
            .id(SharedString::from(id))
            .debug_selector(move || debug_id.clone())
            .flex_1()
            .min_w(px(28.))
            .h(px(30.))
            .flex()
            .items_center()
            .justify_center()
            .px_2()
            .rounded_md()
            .text_xs()
            .bg(rgb(if active { 0xffe9e4 } else { 0xf0f1f5 }))
            .text_color(rgb(if !enabled {
                0xa6a8b2
            } else if active {
                0xd94d38
            } else {
                0x555966
            }))
            .when(enabled, |el| {
                el.cursor_pointer().hover(|s| s.bg(rgb(0xe9e9ef))).on_click(
                    cx.listener(move |this, _, _, cx| this.dispatch_ui(action.clone(), cx)),
                )
            })
            .child(label)
            .into_any_element()
    }
    fn option_section(&self, label: &str, content: impl IntoElement) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(label.to_string()),
            )
            .child(content)
            .into_any_element()
    }
    fn option_number(
        &self,
        label: &str,
        value: f32,
        unit: &str,
        limits: (f32, f32, f32),
        action: impl Fn(f32) -> Action,
        cx: &Context<Self>,
    ) -> AnyElement {
        let (min, max, step) = limits;
        let id = label.to_lowercase().replace(' ', "-");
        self.option_section(
            label,
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(self.option_button(
                    format!("{id}-less"),
                    "−".into(),
                    false,
                    value > min,
                    action((value - step).clamp(min, max)),
                    cx,
                ))
                .child(
                    div()
                        .w(px(92.))
                        .text_center()
                        .text_sm()
                        .child(format!("{value:.0}{unit}")),
                )
                .child(self.option_button(
                    format!("{id}-more"),
                    "+".into(),
                    false,
                    value < max,
                    action((value + step).clamp(min, max)),
                    cx,
                )),
        )
    }
    pub(in crate::editor) fn tool_controls(&self, cx: &Context<Self>) -> impl IntoElement {
        let tool = self.options_tool();
        let settings = self.tool_settings();
        let selected = self.interaction.selected.is_some();
        let mut sections: Vec<AnyElement> = vec![];
        if !matches!(tool, Tool::Select | Tool::Crop | Tool::Pixelate) {
            let colors = [
                [255, 56, 100, 255],
                [255, 184, 46, 255],
                [38, 182, 144, 255],
                [76, 141, 255, 255],
                [255, 255, 255, 255],
                [32, 34, 42, 255],
            ];
            sections.push(
                self.option_section(
                    if matches!(tool, Tool::Magnifier | Tool::Spotlight) {
                        "Border color"
                    } else {
                        "Color"
                    },
                    div()
                        .flex()
                        .gap_2()
                        .children(colors.into_iter().enumerate().map(|(i, mut color)| {
                            color[3] = settings.color[3];
                            div()
                                .id(("tool-color", i))
                                .size(px(27.))
                                .rounded_full()
                                .border_2()
                                .border_color(rgb(if settings.color[..3] == color[..3] {
                                    0xf35d45
                                } else {
                                    0xd7d7df
                                }))
                                .bg(rgb(u32::from_be_bytes(color) >> 8))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.dispatch_ui(Action::SetColor { color }, cx)
                                }))
                        })),
                ),
            );
        }
        match tool {
            Tool::Select => {
                sections.push(
                    div()
                        .text_sm()
                        .text_color(rgb(0x878b98))
                        .child("Click an annotation to see its options.")
                        .into_any_element(),
                );
            }
            Tool::Pen | Tool::Arrow | Tool::Rectangle => {
                if tool != Tool::Rectangle || settings.style.fill == Fill::Outline {
                    sections.push(self.option_number(
                        "Thickness",
                        settings.width,
                        " px",
                        (1., 32., 1.),
                        |width| Action::SetStrokeWidth { width },
                        cx,
                    ));
                }
            }
            Tool::Text => sections.push(self.option_number(
                "Font size",
                settings.width * 7.,
                " px",
                (7., 112., 1.),
                |value| Action::SetStrokeWidth { width: value / 7. },
                cx,
            )),
            Tool::Pixelate => sections.push(self.option_number(
                "Block size",
                (settings.width * 4.).max(12.),
                " px",
                (12., 128., 4.),
                |value| Action::SetStrokeWidth { width: value / 4. },
                cx,
            )),
            Tool::Counter => {
                sections.push(self.option_number(
                    "Badge diameter",
                    settings.width * 7.2,
                    " px",
                    (24., 144., 7.2),
                    |value| Action::SetStrokeWidth { width: value / 7.2 },
                    cx,
                ));
                sections.push(self.option_number(
                    if selected { "Number" } else { "Next number" },
                    self.counter_number() as f32,
                    "",
                    (1., 999., 1.),
                    |value| Action::SetCounterNumber {
                        number: value as u32,
                    },
                    cx,
                ));
            }
            Tool::Magnifier => {
                sections.push(self.option_number(
                    "Lens diameter",
                    crate::effects::radius(&Mark {
                        tool,
                        style: settings.style,
                        curve: None,
                        points: vec![],
                        color: settings.color,
                        width: settings.width,
                        text: String::new(),
                    }) * 2.,
                    " px",
                    (48., 600., 24.),
                    |value| Action::SetStrokeWidth { width: value / 24. },
                    cx,
                ));
                sections.push(self.option_section(
                    "Magnification",
                    div().flex().gap_1().children([2., 3., 4.].map(|zoom| {
                        self.option_button(
                            format!("magnify-{zoom}"),
                            format!("{zoom}×"),
                            settings.magnification == zoom,
                            true,
                            Action::SetMagnifierZoom { zoom },
                            cx,
                        )
                    })),
                ));
            }
            Tool::Spotlight => sections.push(self.option_number(
                "Dim surroundings",
                settings.style.dim * 100.,
                "%",
                (0., 95., 5.),
                |v| Action::SetAppearance {
                    style: crate::style::Style {
                        dim: v / 100.,
                        ..settings.style
                    },
                },
                cx,
            )),
            Tool::Crop => {
                let ratios = [
                    ("Free", None),
                    ("1:1", Some(1.)),
                    ("4:3", Some(4. / 3.)),
                    ("16:9", Some(16. / 9.)),
                    ("9:16", Some(9. / 16.)),
                ];
                sections.push(
                    self.option_section(
                        "Aspect ratio",
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .children(ratios.into_iter().map(|(label, ratio)| {
                                self.option_button(
                                    format!("crop-{label}"),
                                    label.into(),
                                    self.interaction.crop_ratio == ratio,
                                    true,
                                    Action::SetCropRatio { ratio },
                                    cx,
                                )
                            })),
                    ),
                );
            }
            Tool::Highlight => {}
        }
        if matches!(
            tool,
            Tool::Pen | Tool::Arrow | Tool::Rectangle | Tool::Text | Tool::Highlight
        ) {
            sections.push(self.option_number(
                if tool == Tool::Highlight {
                    "Intensity"
                } else {
                    "Opacity"
                },
                settings.color[3] as f32 / 255. * 100.,
                "%",
                (10., 100., 10.),
                |value| {
                    let mut color = settings.color;
                    color[3] = (value / 100. * 255.).round() as u8;
                    Action::SetColor { color }
                },
                cx,
            ));
            if tool == Tool::Highlight {
                sections.push(
                    div()
                        .text_xs()
                        .text_color(rgb(0x878b98))
                        .child("Intensity scales the transparent tint; image details stay visible.")
                        .into_any_element(),
                );
            }
        }
        if matches!(tool, Tool::Arrow | Tool::Rectangle)
            && (tool != Tool::Rectangle || settings.style.fill == Fill::Outline)
        {
            sections.push(
                self.option_section(
                    "Stroke style",
                    div().flex().gap_1().children(
                        [
                            (Dash::Solid, "Solid"),
                            (Dash::Dashed, "Dash"),
                            (Dash::Dotted, "Dot"),
                        ]
                        .map(|(dash, label)| {
                            self.option_button(
                                format!("dash-{label}"),
                                label.into(),
                                settings.style.dash == dash,
                                true,
                                Action::SetAppearance {
                                    style: crate::style::Style {
                                        dash,
                                        ..settings.style
                                    },
                                },
                                cx,
                            )
                        }),
                    ),
                ),
            );
        }
        if tool == Tool::Rectangle {
            sections.push(self.option_section(
                "Fill",
                div().flex().gap_1().children(
                    [(Fill::Outline, "Outline"), (Fill::Filled, "Filled")].map(|(fill, label)| {
                        self.option_button(
                            format!("fill-{label}"),
                            label.into(),
                            settings.style.fill == fill,
                            true,
                            Action::SetAppearance {
                                style: crate::style::Style {
                                    fill,
                                    ..settings.style
                                },
                            },
                            cx,
                        )
                    }),
                ),
            ));
            sections.push(self.option_number(
                "Corner radius",
                settings.style.radius,
                " px",
                (0., 128., 4.),
                |radius| Action::SetAppearance {
                    style: crate::style::Style {
                        radius,
                        ..settings.style
                    },
                },
                cx,
            ));
        }
        if tool == Tool::Pen {
            sections.push(
                self.option_section(
                    "Line cleanup",
                    div().flex().gap_1().children(
                        [
                            (Cleanup::Raw, "Raw"),
                            (Cleanup::Smooth, "Smooth"),
                            (Cleanup::Adaptive, "Adaptive"),
                        ]
                        .map(|(cleanup, label)| {
                            self.option_button(
                                format!("cleanup-{label}"),
                                label.into(),
                                settings.style.cleanup == cleanup,
                                true,
                                Action::SetAppearance {
                                    style: crate::style::Style {
                                        cleanup,
                                        ..settings.style
                                    },
                                },
                                cx,
                            )
                        }),
                    ),
                ),
            );
            sections.push(
                div()
                    .text_xs()
                    .text_color(rgb(0x878b98))
                    .child(match settings.style.cleanup {
                        Cleanup::Raw => "Keeps your original movement.",
                        Cleanup::Smooth => "Rounds small wobbles for a softer stroke.",
                        Cleanup::Adaptive => "Smooths gentle curves and preserves sharp turns.",
                    })
                    .into_any_element(),
            );
        }
        if tool == Tool::Arrow {
            for (label, start) in [("Start", true), ("End", false)] {
                sections.push(
                    self.option_section(
                        label,
                        div().flex().gap_1().children(
                            [
                                (End::None, "None"),
                                (End::Arrow, "Arrow"),
                                (End::Dot, "Dot"),
                            ]
                            .map(|(end, name)| {
                                let mut style = settings.style;
                                if start {
                                    style.start = end
                                } else {
                                    style.end = end
                                };
                                self.option_button(
                                    format!("line-{label}-{name}"),
                                    name.into(),
                                    if start {
                                        settings.style.start == end
                                    } else {
                                        settings.style.end == end
                                    },
                                    true,
                                    Action::SetAppearance { style },
                                    cx,
                                )
                            }),
                        ),
                    ),
                );
            }
            sections.push(
                self.option_section(
                    "Points",
                    div()
                        .flex()
                        .gap_1()
                        .child(self.option_button(
                            "line-add-point".into(),
                            "Add point".into(),
                            false,
                            selected,
                            Action::AddLinePoint,
                            cx,
                        ))
                        .child(self.option_button(
                            "line-straighten".into(),
                            "Straighten".into(),
                            false,
                            selected,
                            Action::StraightenLine,
                            cx,
                        )),
                ),
            );
            sections.push(
                div()
                    .text_xs()
                    .text_color(rgb(0x878b98))
                    .child(if selected {
                        "Drag a handle to move an end, bend, or waypoint."
                    } else {
                        "Draw a line, then add and drag points to shape its route."
                    })
                    .into_any_element(),
            );
        }
        let help = match tool {
            Tool::Crop => "Drag to crop immediately. Shift draws a square. ⌘Z restores the image.",
            Tool::Rectangle | Tool::Highlight | Tool::Pixelate => "Shift-drag to draw a square.",
            Tool::Text => "Click and type. Enter finishes; Escape cancels.",
            Tool::Spotlight => "Drag a focus window. Corner handles resize it.",
            Tool::Magnifier => {
                "Drag from a detail to its lens position. Handles move the source and lens."
            }
            Tool::Counter => "Click to place a step. Numbers advance automatically.",
            _ => "",
        };
        if !help.is_empty() {
            sections.push(
                div()
                    .text_xs()
                    .text_color(rgb(0x878b98))
                    .child(help)
                    .into_any_element(),
            );
        }
        if selected {
            sections.push(
                self.option_section(
                    "Object",
                    div()
                        .flex()
                        .gap_1()
                        .child(self.option_button(
                            "object-duplicate".into(),
                            "Duplicate".into(),
                            false,
                            true,
                            Action::DuplicateSelection,
                            cx,
                        ))
                        .child(self.option_button(
                            "object-delete".into(),
                            "Delete".into(),
                            false,
                            true,
                            Action::Delete,
                            cx,
                        )),
                ),
            );
            sections.push(
                div()
                    .text_xs()
                    .text_color(rgb(0x878b98))
                    .child("Arrow keys move 1 px; Shift moves 10 px. ⌘Z undoes edits.")
                    .into_any_element(),
            );
        }
        div()
            .id("tool-panel")
            .debug_selector(|| "tool-panel".into())
            .w(px(260.))
            .h_full()
            .flex_shrink_0()
            .p_5()
            .flex()
            .flex_col()
            .gap_5()
            .overflow_y_scroll()
            .bg(rgb(0xfcfcfd))
            .border_l_1()
            .border_color(rgb(0xe5e5ec))
            .cursor(CursorStyle::Arrow)
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.dispatch_ui(Action::CommitText, cx);
                    cx.stop_propagation();
                }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(
                        if tool == Tool::Arrow {
                            "Line / Arrow"
                        } else {
                            tool.label()
                        },
                    ))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x878b98))
                            .child(if selected {
                                "Selected annotation"
                            } else {
                                "Options for the next annotation"
                            }),
                    ),
            )
            .children(
                sections
                    .into_iter()
                    .map(|section| div().flex_shrink_0().child(section)),
            )
    }
}
