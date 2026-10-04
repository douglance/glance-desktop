use super::super::{
    Editor,
    actions::Action,
    view::{HoverLabel, icon},
};
use super::{
    controls::{self, Sample},
    number,
};
use crate::{
    document::{Mark, Tool},
    style::{Cleanup, Dash, End, Fill},
};
use gpui::{prelude::*, *};

impl Editor {
    pub(in crate::editor) fn tool_scope(&self) -> number::Scope {
        (
            self.preview.revision,
            self.options_tool().index(),
            self.interaction.selected,
            self.interaction.text_session,
        )
    }
    pub(in crate::editor) fn number_action(&self, label: &str, value: f32) -> Action {
        let settings = self.tool_settings();
        match label {
            "Thickness" => Action::SetStrokeWidth { width: value },
            "Font size" => Action::SetStrokeWidth { width: value / 7. },
            "Block size" => Action::SetStrokeWidth { width: value / 4. },
            "Badge diameter" => Action::SetStrokeWidth { width: value / 7.2 },
            "Lens diameter" => Action::SetStrokeWidth { width: value / 24. },
            "Number" | "Next number" => Action::SetCounterNumber {
                number: value.round() as u32,
            },
            "Dim surroundings" => Action::SetAppearance {
                style: crate::style::Style {
                    dim: value / 100.,
                    ..settings.style
                },
            },
            "Corner radius" => Action::SetAppearance {
                style: crate::style::Style {
                    radius: value,
                    ..settings.style
                },
            },
            "Opacity" | "Intensity" => {
                let mut color = settings.color;
                color[3] = (value / 100. * 255.).round() as u8;
                Action::SetColor { color }
            }
            _ => unreachable!("registered numeric field"),
        }
    }
    fn option_button(
        &self,
        id: String,
        label: String,
        active: bool,
        enabled: bool,
        action: Action,
        cx: &Context<Self>,
    ) -> AnyElement {
        self.choice(
            id,
            label.clone(),
            div().child(label).into_any_element(),
            (active, enabled),
            action,
            cx,
        )
    }
    fn option_sample(
        &self,
        id: String,
        label: &str,
        sample: Sample,
        state: (bool, bool),
        action: Action,
        cx: &Context<Self>,
    ) -> AnyElement {
        let (active, enabled) = state;
        let color = if !enabled {
            0xa6a8b2
        } else if active {
            0xd94d38
        } else {
            0x555966
        };
        self.choice(
            id,
            label.into(),
            controls::sample(sample, color).into_any_element(),
            (active, enabled),
            action,
            cx,
        )
    }
    fn option_number(
        &self,
        label: &'static str,
        value: f32,
        unit: &'static str,
        limits: (f32, f32, f32),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let input = &self.number_inputs[label];
        input.update(cx, |input, cx| {
            input.set_value(value, unit, limits, self.tool_scope(), cx)
        });
        controls::field(label, input.clone())
    }
    pub(in crate::editor) fn tool_controls(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let tool = self.options_tool();
        let settings = self.tool_settings();
        let selected = self.interaction.selected.is_some();
        let mut sections = Vec::new();
        if !matches!(tool, Tool::Select | Tool::Crop | Tool::Pixelate) {
            self.tool_color_picker.update(cx, |picker, cx| {
                picker.set_value(
                    [settings.color[0], settings.color[1], settings.color[2]],
                    cx,
                )
            });
            let colors = [
                ([255, 56, 100, 255], "Coral"),
                ([255, 184, 46, 255], "Amber"),
                ([38, 182, 144, 255], "Mint"),
                ([76, 141, 255, 255], "Blue"),
                ([255, 255, 255, 255], "White"),
                ([32, 34, 42, 255], "Ink"),
            ];
            sections.push(controls::field(
                if matches!(tool, Tool::Magnifier | Tool::Spotlight) {
                    "Border color"
                } else {
                    "Color"
                },
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .children(
                        colors
                            .into_iter()
                            .enumerate()
                            .map(|(i, (mut color, name))| {
                                color[3] = settings.color[3];
                                div()
                                    .id(("tool-color", i))
                                    .debug_selector(move || format!("tool-color-{i}"))
                                    .size(px(23.))
                                    .flex_shrink_0()
                                    .rounded_full()
                                    .border_2()
                                    .border_color(rgb(if settings.color[..3] == color[..3] {
                                        0xf35d45
                                    } else {
                                        0xd7d7df
                                    }))
                                    .bg(rgb(u32::from_be_bytes(color) >> 8))
                                    .cursor_pointer()
                                    .tooltip(move |_, cx| {
                                        cx.new(|_| HoverLabel(name.into())).into()
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.dispatch_ui(Action::SetColor { color }, cx)
                                    }))
                            }),
                    )
                    .child(div().ml_1().child(self.tool_color_picker.clone())),
            ));
        }
        let mut primary: Vec<AnyElement> = vec![];
        match tool {
            Tool::Select => sections.push(
                div()
                    .text_xs()
                    .text_color(rgb(0x646976))
                    .child("Select an annotation to edit its properties.")
                    .into_any_element(),
            ),
            Tool::Pen | Tool::Arrow | Tool::Rectangle => {
                if tool != Tool::Rectangle || settings.style.fill == Fill::Outline {
                    primary.push(self.option_number(
                        "Thickness",
                        settings.width,
                        "px",
                        (1., 32., 1.),
                        cx,
                    ));
                }
            }
            Tool::Text => primary.push(self.option_number(
                "Font size",
                settings.width * 7.,
                "px",
                (7., 112., 1.),
                cx,
            )),
            Tool::Pixelate => primary.push(self.option_number(
                "Block size",
                (settings.width * 4.).max(12.),
                "px",
                (12., 128., 4.),
                cx,
            )),
            Tool::Counter => {
                primary.push(self.option_number(
                    "Badge diameter",
                    settings.width * 7.2,
                    "px",
                    (24., 144., 7.2),
                    cx,
                ));
                primary.push(self.option_number(
                    if selected { "Number" } else { "Next number" },
                    self.counter_number() as f32,
                    "",
                    (1., 999., 1.),
                    cx,
                ));
            }
            Tool::Magnifier => {
                let diameter = crate::effects::radius(&Mark {
                    tool,
                    style: settings.style,
                    curve: None,
                    points: vec![],
                    color: settings.color,
                    width: settings.width,
                    text: String::new(),
                }) * 2.;
                primary.push(self.option_number(
                    "Lens diameter",
                    diameter,
                    "px",
                    (48., 600., 24.),
                    cx,
                ));
                primary.push(controls::field(
                    "Zoom",
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
            Tool::Spotlight => primary.push(self.option_number(
                "Dim surroundings",
                settings.style.dim * 100.,
                "%",
                (0., 95., 5.),
                cx,
            )),
            Tool::Crop | Tool::Highlight => {}
        }
        if matches!(
            tool,
            Tool::Pen | Tool::Arrow | Tool::Rectangle | Tool::Text | Tool::Highlight
        ) {
            primary.push(self.option_number(
                if tool == Tool::Highlight {
                    "Intensity"
                } else {
                    "Opacity"
                },
                settings.color[3] as f32 / 255. * 100.,
                "%",
                (10., 100., 10.),
                cx,
            ));
        }
        let mut fields = primary.into_iter();
        while let Some(a) = fields.next() {
            sections.push(controls::pair(
                a,
                fields.next().unwrap_or_else(|| div().into_any_element()),
            ));
        }
        let stroke = || {
            controls::field(
                "Stroke",
                div().flex().gap_1().children(
                    [
                        (Dash::Solid, "Solid"),
                        (Dash::Dashed, "Dash"),
                        (Dash::Dotted, "Dot"),
                    ]
                    .map(|(dash, label)| {
                        self.option_sample(
                            format!("dash-{label}"),
                            match dash {
                                Dash::Solid => "Solid stroke",
                                Dash::Dashed => "Dashed stroke",
                                Dash::Dotted => "Dotted stroke",
                            },
                            Sample::Stroke(dash),
                            (settings.style.dash == dash, true),
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
            )
        };
        if tool == Tool::Arrow {
            let points = controls::field(
                "Points",
                div()
                    .flex()
                    .gap_1()
                    .child(self.option_sample(
                        "line-add-point".into(),
                        "Add point",
                        Sample::Plus,
                        (false, selected),
                        Action::AddLinePoint,
                        cx,
                    ))
                    .child(self.option_sample(
                        "line-straighten".into(),
                        "Straighten",
                        Sample::Straight,
                        (false, selected),
                        Action::StraightenLine,
                        cx,
                    )),
            );
            sections.push(controls::pair(stroke(), points));
            let ends = [true, false].map(|start| {
                let label = if start { "Start" } else { "End" };
                controls::field(
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
                                style.start = end;
                            } else {
                                style.end = end;
                            }
                            self.option_sample(
                                format!("line-{label}-{name}"),
                                &format!("{label}: {name}"),
                                Sample::End(end, start),
                                (
                                    if start {
                                        settings.style.start == end
                                    } else {
                                        settings.style.end == end
                                    },
                                    true,
                                ),
                                Action::SetAppearance { style },
                                cx,
                            )
                        }),
                    ),
                )
            });
            let [start, end] = ends;
            sections.push(controls::pair(start, end));
        }
        if tool == Tool::Rectangle {
            let fill = controls::field(
                "Fill",
                div().flex().gap_1().children(
                    [(Fill::Outline, "Outline"), (Fill::Filled, "Filled")].map(|(fill, label)| {
                        self.option_sample(
                            format!("fill-{label}"),
                            label,
                            Sample::Fill(fill),
                            (settings.style.fill == fill, true),
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
            );
            if settings.style.fill == Fill::Outline {
                sections.push(controls::pair(stroke(), fill));
                sections.push(controls::pair(
                    self.option_number(
                        "Corner radius",
                        settings.style.radius,
                        "px",
                        (0., 128., 4.),
                        cx,
                    ),
                    div(),
                ));
            } else {
                sections.push(controls::pair(
                    fill,
                    self.option_number(
                        "Corner radius",
                        settings.style.radius,
                        "px",
                        (0., 128., 4.),
                        cx,
                    ),
                ));
            }
        }
        if tool == Tool::Pen {
            sections.push(controls::field(
                "Cleanup",
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
            ));
        }
        if tool == Tool::Crop {
            sections.push(controls::field(
                "Aspect ratio",
                div().flex().flex_wrap().gap_1().children(
                    [
                        ("Free", None),
                        ("1:1", Some(1.)),
                        ("4:3", Some(4. / 3.)),
                        ("16:9", Some(16. / 9.)),
                        ("9:16", Some(9. / 16.)),
                    ]
                    .map(|(label, ratio)| {
                        self.option_button(
                            format!("crop-{label}"),
                            label.into(),
                            self.interaction.crop_ratio == ratio,
                            true,
                            Action::SetCropRatio { ratio },
                            cx,
                        )
                    }),
                ),
            ));
        }
        let help = match tool {
            Tool::Arrow if selected => {
                "Drag handles to move endpoints, bends and waypoints. Add point inserts a waypoint; Straighten removes bends and waypoints."
            }
            Tool::Arrow => {
                "Draw a line, then select it to add and drag points. Start and End choose each endpoint independently."
            }
            Tool::Pen => match settings.style.cleanup {
                Cleanup::Raw => "Keeps your original movement.",
                Cleanup::Smooth => "Rounds small wobbles for a softer stroke.",
                Cleanup::Adaptive => "Smooths gentle curves and preserves sharp turns.",
            },
            Tool::Crop => "Drag to crop immediately. Shift draws a square. ⌘Z restores the image.",
            Tool::Rectangle | Tool::Pixelate => "Shift-drag to draw a square.",
            Tool::Highlight => {
                "Intensity scales the transparent tint; image details stay visible. Shift-drag draws a square."
            }
            Tool::Text => "Click and type. Enter finishes; Escape cancels.",
            Tool::Spotlight => "Drag a focus window. Corner handles resize it.",
            Tool::Magnifier => {
                "Drag from a detail to its lens position. Handles move the source and lens."
            }
            Tool::Counter => "Click to place a step. Numbers advance automatically.",
            Tool::Select => "Click an annotation to see its options.",
        };
        let hint = match tool {
            Tool::Pen => "P · draw a stroke",
            Tool::Arrow => "A · draw a line",
            Tool::Rectangle => "R · Shift-drag for a square",
            Tool::Text => "T · click and type",
            Tool::Highlight => "H · drag a highlight",
            Tool::Pixelate => "B · drag to redact",
            Tool::Crop => "X · drag to crop",
            Tool::Counter => "N · click to place a step",
            Tool::Spotlight => "S · drag a focus window",
            Tool::Magnifier => "M · drag from detail to lens",
            Tool::Select => "V · select an annotation",
        };
        controls::panel("tool-panel", cx)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
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
                            .text_color(rgb(0x646976))
                            .child(if selected {
                                "Selected"
                            } else {
                                "New annotation"
                            }),
                    ),
            )
            .children(
                sections
                    .into_iter()
                    .map(|section| div().flex_shrink_0().child(section)),
            )
            .when(selected, |el| {
                el.child(
                    div()
                        .flex()
                        .gap_1()
                        .child(self.choice(
                            "object-duplicate".into(),
                            "Duplicate annotation · ⌘D".into(),
                            icon("copy", 0x555966).into_any_element(),
                            (false, true),
                            Action::DuplicateSelection,
                            cx,
                        ))
                        .child(self.choice(
                            "object-delete".into(),
                            "Delete annotation · Backspace".into(),
                            div().text_lg().child("×").into_any_element(),
                            (false, true),
                            Action::Delete,
                            cx,
                        )),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x646976))
                            .child(if selected {
                                "Arrow keys move · Shift moves 10 px"
                            } else {
                                hint
                            }),
                    )
                    .child(
                        div()
                            .id("tool-help")
                            .size(px(24.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .text_color(rgb(0x646976))
                            .child("?")
                            .tooltip(move |_, cx| cx.new(|_| HoverLabel(help.into())).into()),
                    ),
            )
    }
}
