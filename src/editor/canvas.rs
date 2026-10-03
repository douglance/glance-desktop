use super::*;
impl Editor {
    pub(super) fn canvas(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
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
        let zoom = self.zoom;
        let pan = self.pan;
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
                        let _ =
                            sender.send_blocking(Message::Image(platform::load(&path).map(Some)));
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
                cx.listener(|this, e: &MouseDownEvent, _, _| this.pan_start = Some(e.position)),
            )
            .child(
                canvas(
                    move |bounds, _, _| bounds,
                    move |bounds, _, window, cx| {
                        canvas_bounds.set(bounds);
                        window.paint_quad(fill(bounds, rgb(0xeff0f4)));
                        let fit = ((f32::from(bounds.size.width) - 80.)
                            / output_dimensions.0 as f32)
                            .min((f32::from(bounds.size.height) - 70.) / output_dimensions.1 as f32)
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
                        let image_bounds = Bounds::new(point(px(x), px(y)), size(px(w), px(h)));
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
                                        animation::paint(b, animation_phase, frame_bounds, window)
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
                            px(backdrop.map_or(0., |b| b.inner_radius as f32 * scale)).into(),
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
                                if let Some((left, top, right, bottom)) =
                                    selection_bounds.filter(|_| {
                                        selected_mark.as_ref().is_none_or(|m| m.tool != Tool::Arrow)
                                    })
                                {
                                    let l = layout.get();
                                    let b = Bounds::new(
                                        point(px(l.x + left * l.scale), px(l.y + top * l.scale)),
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
                                text::paint(&text_entity, layout.get(), image_bounds, window, cx);
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
            .when(self.enhance_panel, |el| el.child(self.enhance_controls(cx)))
    }
}
