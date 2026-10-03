//! Tests use GPUI's virtual platform. No desktop interaction or capture permission.
use crate::{Document, Editor, Layout, Message, Tool, render_image};
use gpui::{
    Bounds, EntityInputHandler, KeyDownEvent, Keystroke, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, TestAppContext, WindowHandle, point, px, size,
};
use std::sync::Arc;
fn editor(cx: &mut TestAppContext) -> WindowHandle<Editor> {
    cx.add_window(|window, cx| {
        let mut e = Editor::with_native(cx, false);
        e.document = Document::new(image::RgbaImage::from_pixel(
            100,
            100,
            image::Rgba([0, 0, 0, 255]),
        ));
        e.preview = render_image((*e.document.base).clone());
        e.layout.set(Layout {
            x: 0.,
            y: 0.,
            scale: 1.,
            width: 100.,
            height: 100.,
        });
        e.canvas_bounds
            .set(Bounds::new(point(px(0.), px(0.)), size(px(100.), px(100.))));
        e.focus.focus(window);
        e
    })
}
fn reset_layout(e: &Editor) {
    e.layout.set(Layout {
        x: 0.,
        y: 0.,
        scale: 1.,
        width: 100.,
        height: 100.,
    });
    e.canvas_bounds
        .set(Bounds::new(point(px(0.), px(0.)), size(px(100.), px(100.))));
}
fn down(x: f32, y: f32) -> MouseDownEvent {
    MouseDownEvent {
        position: point(px(x), px(y)),
        button: MouseButton::Left,
        modifiers: Default::default(),
        click_count: 1,
        first_mouse: false,
    }
}
fn up(x: f32, y: f32) -> MouseUpEvent {
    MouseUpEvent {
        position: point(px(x), px(y)),
        button: MouseButton::Left,
        modifiers: Default::default(),
        click_count: 1,
    }
}
fn motion(x: f32, y: f32) -> MouseMoveEvent {
    MouseMoveEvent {
        position: point(px(x), px(y)),
        pressed_button: Some(MouseButton::Left),
        modifiers: Default::default(),
    }
}
fn key(k: &str) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke::parse(k).unwrap(),
        is_held: false,
    }
}
#[gpui::test]
fn drawing_pick_move_duplicate_delete_and_undo(cx: &mut TestAppContext) {
    let view = editor(cx);
    view.update(cx, |e, w, cx| {
        reset_layout(e);
        e.set_tool(Tool::Pen, cx);
        e.begin(&down(10., 10.), w, cx);
        e.motion(&motion(30., 30.), cx);
        e.finish(&up(40., 40.), cx);
        assert_eq!(e.document.marks.len(), 1);
        e.set_tool(Tool::Select, cx);
        e.begin(&down(20., 20.), w, cx);
        assert_eq!(e.selected, Some(0));
        e.motion(&motion(40., 30.), cx);
        assert_eq!(e.document.marks[0].points[0], (10., 10.));
        e.finish(&up(40., 30.), cx);
        assert_eq!(e.document.marks[0].points[0], (30., 20.));
        e.duplicate_selected(cx);
        assert_eq!(e.document.marks.len(), 2);
        assert_eq!(e.selected, Some(1));
        e.delete_selected(cx);
        assert_eq!(e.document.marks.len(), 1);
        e.document.undo();
        assert_eq!(e.document.marks.len(), 2);
        e.document.undo();
        assert_eq!(e.document.marks.len(), 1);
        e.document.undo();
        assert_eq!(e.document.marks[0].points[0], (10., 10.));
    })
    .unwrap();
}
#[gpui::test]
fn cancel_move_and_shift_release_preserve_document(cx: &mut TestAppContext) {
    let view = editor(cx);
    view.update(cx, |e, w, cx| {
        reset_layout(e);
        e.set_tool(Tool::Rectangle, cx);
        e.begin(&down(10., 10.), w, cx);
        let mut release = up(50., 35.);
        release.modifiers.shift = true;
        e.finish(&release, cx);
        assert_eq!(e.document.marks[0].points.last(), Some(&(50., 50.)));
        e.set_tool(Tool::Select, cx);
        e.begin(&down(10., 30.), w, cx);
        e.motion(&motion(30., 40.), cx);
        e.key(&key("escape"), w, cx);
        e.finish(&up(30., 40.), cx);
        assert_eq!(e.document.marks[0].points[0], (10., 10.));
        assert!(e.selected.is_none());
    })
    .unwrap();
}
#[gpui::test]
fn space_pan_and_zoom_do_not_add_marks(cx: &mut TestAppContext) {
    let view = editor(cx);
    view.update(cx, |e, w, cx| {
        reset_layout(e);
        e.set_tool(Tool::Pen, cx);
        e.key(&key("space"), w, cx);
        e.begin(&down(10., 10.), w, cx);
        e.motion(&motion(30., 40.), cx);
        e.finish(&up(30., 40.), cx);
        assert_eq!(e.pan, (20., 30.));
        assert!(e.document.marks.is_empty());
        e.space_down = false;
        e.receive(Message::Magnify(0.5, (30., 40.), false), cx);
        assert_eq!(e.zoom, Some(1.5));
        assert!(e.document.marks.is_empty());
        e.key(&key("cmd-1"), w, cx);
        assert!(e.zoom.is_none());
        assert_eq!(e.pan, (0., 0.));
        e.zoom_down = true;
        e.begin(&down(20., 20.), w, cx);
        assert_eq!(e.zoom, Some(2.));
    })
    .unwrap();
}
#[gpui::test]
fn text_typing_unicode_editing_commit_and_cancel(cx: &mut TestAppContext) {
    let view = editor(cx);
    view.update(cx, |e, w, cx| {
        reset_layout(e);
        e.set_tool(Tool::Text, cx);
        e.begin(&down(10., 10.), w, cx);
        e.replace_text_in_range(None, "Hi 👨‍👩‍👧‍👦 café 日本語", w, cx);
        e.key(&key("backspace"), w, cx);
        e.key(&key("cmd-z"), w, cx);
        assert_eq!(
            e.text_edit.as_ref().unwrap().buffer.text(),
            "Hi 👨‍👩‍👧‍👦 café 日本語"
        );
        e.key(&key("enter"), w, cx);
        assert_eq!(e.document.marks.len(), 1);
        e.begin(&down(50., 50.), w, cx);
        e.replace_text_in_range(None, "discard", w, cx);
        e.key(&key("escape"), w, cx);
        assert_eq!(e.document.marks.len(), 1);
        assert!(e.text_edit.is_none());
    })
    .unwrap();
}
#[gpui::test]
fn counter_numbers_remain_unique_after_deleting_a_step(cx: &mut TestAppContext) {
    let view = editor(cx);
    view.update(cx, |e, w, cx| {
        reset_layout(e);
        e.set_tool(Tool::Counter, cx);
        e.begin(&down(10., 10.), w, cx);
        e.begin(&down(40., 40.), w, cx);
        e.document.delete_mark(0);
        e.begin(&down(70., 70.), w, cx);
        let labels: Vec<_> = e.document.marks.iter().map(|m| m.text.as_str()).collect();
        assert_eq!(labels, vec!["2", "3"]);
    })
    .unwrap();
}
#[gpui::test]
fn stale_previews_cannot_overwrite_new_edits(cx: &mut TestAppContext) {
    let view = editor(cx);
    view.update(cx, |e, _, cx| {
        let old = e.preview.clone();
        e.revision = 5;
        e.receive(
            Message::Preview(4, 99, render_image(image::RgbaImage::new(2, 2))),
            cx,
        );
        assert!(Arc::ptr_eq(&e.preview, &old));
        assert_ne!(e.preview_count, 99);
        e.receive(
            Message::Preview(5, 0, render_image(image::RgbaImage::new(100, 100))),
            cx,
        );
        assert!(!Arc::ptr_eq(&e.preview, &old));
        assert_eq!(e.preview_count, 0);
    })
    .unwrap();
}
#[gpui::test]
fn real_event_dispatch_draws_and_commits_text_without_double_actions(cx: &mut TestAppContext) {
    let window = editor(cx);
    let view = window.root(cx).unwrap();
    let mut visual = gpui::VisualTestContext::from_window(*window, cx);
    visual.update(|_, cx| crate::menus::install(cx));
    visual.simulate_keystrokes("p");
    assert_eq!(view.read_with(&visual, |e, _| e.tool), Tool::Pen);
    let l = view.read_with(&visual, |e, _| e.layout.get());
    let pos = |x, y| point(px(l.x + x * l.scale), px(l.y + y * l.scale));
    visual.simulate_mouse_down(pos(10., 10.), MouseButton::Left, Default::default());
    visual.simulate_mouse_move(pos(40., 40.), MouseButton::Left, Default::default());
    visual.simulate_mouse_up(pos(40., 40.), MouseButton::Left, Default::default());
    assert_eq!(view.read_with(&visual, |e, _| e.document.marks.len()), 1);
    visual.simulate_keystrokes("t");
    visual.simulate_mouse_down(pos(50., 50.), MouseButton::Left, Default::default());
    visual.simulate_mouse_up(pos(50., 50.), MouseButton::Left, Default::default());
    assert_eq!(view.read_with(&visual, |e, _| e.tool), Tool::Text);
    assert!(
        view.read_with(&visual, |e, _| e.text_edit.is_some()),
        "text click must open canvas editor"
    );
    visual.simulate_input("p a r t h b x n v z hello café 日本語");
    assert_eq!(
        view.read_with(&visual, |e, _| e
            .text_edit
            .as_ref()
            .unwrap()
            .buffer
            .text()
            .to_owned()),
        "p a r t h b x n v z hello café 日本語"
    );
    visual.simulate_keystrokes("enter");
    assert_eq!(view.read_with(&visual, |e, _| e.document.marks.len()), 2);
    assert_eq!(
        view.read_with(&visual, |e, _| e.document.marks[1].text.clone()),
        "p a r t h b x n v z hello café 日本語"
    );
    visual.simulate_keystrokes("cmd-z");
    assert_eq!(
        view.read_with(&visual, |e, _| e.document.marks.len()),
        1,
        "one cmd-z must undo once despite a native menu binding"
    );
}
#[gpui::test]
fn ime_preedit_replacement_and_commit_use_utf16_ranges(cx: &mut TestAppContext) {
    let view = editor(cx);
    view.update(cx, |e, w, cx| {
        reset_layout(e);
        e.set_tool(Tool::Text, cx);
        e.begin(&down(10., 10.), w, cx);
        e.replace_text_in_range(None, "👋 ", w, cx);
        e.replace_and_mark_text_in_range(None, "に", Some(1..1), w, cx);
        e.replace_and_mark_text_in_range(None, "日本", Some(2..2), w, cx);
        assert_eq!(e.text_edit.as_ref().unwrap().buffer.text(), "👋 日本");
        assert_eq!(e.marked_text_range(w, cx), Some(3..5));
        assert_eq!(e.selected_text_range(false, w, cx).unwrap().range, 5..5);
        e.key(&key("enter"), w, cx);
        assert!(
            e.text_edit.is_some(),
            "Enter during composition must be handled by the IME"
        );
        e.unmark_text(w, cx);
        e.key(&key("enter"), w, cx);
        assert_eq!(e.document.marks[0].text, "👋 日本");
    })
    .unwrap();
}
#[gpui::test]
fn selected_style_and_held_nudges_are_undoable(cx: &mut TestAppContext) {
    let view = editor(cx);
    view.update(cx, |e, w, cx| {
        reset_layout(e);
        e.set_tool(Tool::Arrow, cx);
        e.begin(&down(10., 10.), w, cx);
        e.finish(&up(70., 70.), cx);
        e.selected = Some(0);
        let color = e.document.marks[0].color;
        e.color = [0, 120, 255, 255];
        e.apply_style(true, cx);
        assert_eq!(e.document.marks[0].color, e.color);
        e.document.undo();
        assert_eq!(e.document.marks[0].color, color);
        e.key(&key("right"), w, cx);
        let mut repeat = key("right");
        repeat.is_held = true;
        for _ in 0..20 {
            e.key(&repeat, w, cx);
        }
        assert_eq!(e.document.marks[0].points[0], (31., 10.));
        e.document.undo();
        assert_eq!(e.document.marks[0].points[0], (10., 10.));
    })
    .unwrap();
}

#[gpui::test]
fn arrow_handles_edit_independently_and_new_marks_stay_selected(cx: &mut TestAppContext) {
    let view = editor(cx);
    view.update(cx, |e, w, cx| {
        reset_layout(e);
        e.set_tool(Tool::Arrow, cx);
        e.begin(&down(10., 50.), w, cx);
        e.motion(&motion(90., 50.), cx);
        e.finish(&up(90., 50.), cx);
        assert_eq!(e.selected, Some(0));
        assert_eq!(e.tool, Tool::Arrow);
        assert_eq!(e.document.marks[0].points, vec![(10., 50.), (90., 50.)]);
        e.begin(&down(90., 50.), w, cx);
        e.motion(&motion(85., 70.), cx);
        e.finish(&up(85., 70.), cx);
        assert_eq!(e.document.marks[0].points, vec![(10., 50.), (85., 70.)]);
        e.begin(&down(10., 50.), w, cx);
        e.finish(&up(15., 60.), cx);
        assert_eq!(e.document.marks[0].points, vec![(15., 60.), (85., 70.)]);
        let mid = crate::arrow::at(&e.document.marks[0], 0.5);
        e.begin(&down(mid.0, mid.1), w, cx);
        e.motion(&motion(50., 25.), cx);
        e.finish(&up(50., 25.), cx);
        let m = &e.document.marks[0];
        assert_eq!(m.points, vec![(15., 60.), (85., 70.)]);
        assert_eq!(crate::arrow::at(m, 0.5), (50., 25.));
        assert!(m.hit((50., 25.), 1.));
        assert!(!m.hit((50., 65.), 1.));
        let curved = m.clone();
        e.begin(&down(85., 70.), w, cx);
        e.motion(&motion(95., 80.), cx);
        e.key(&key("escape"), w, cx);
        assert_eq!(e.document.marks[0].points, curved.points);
        e.document.undo();
        assert!(e.document.marks[0].curve.is_none());
        e.document.redo();
        assert_eq!(e.document.marks[0].curve, curved.curve);
        e.selected = Some(0);
        // Move the shaft while the Arrow tool remains active, away from the handles.
        let p = crate::arrow::at(&curved, 0.25);
        e.begin(&down(p.0, p.1), w, cx);
        e.finish(&up(p.0 + 5., p.1 + 5.), cx);
        assert_eq!(e.document.marks[0].points[0], (20., 65.));
        assert_eq!(crate::arrow::at(&e.document.marks[0], 0.5), (55., 30.));
        e.key(&key("backspace"), w, cx);
        assert!(e.document.marks.is_empty());
        e.receive(
            Message::Preview(e.revision, 0, render_image((*e.document.base).clone())),
            cx,
        );
        e.set_tool(Tool::Rectangle, cx);
        e.begin(&down(10., 10.), w, cx);
        e.finish(&up(40., 40.), cx);
        assert_eq!(e.selected, Some(0));
        e.key(&key("backspace"), w, cx);
        assert!(e.document.marks.is_empty());
    })
    .unwrap();
}

#[gpui::test]
fn motion_controls_pause_duration_and_history(cx: &mut TestAppContext) {
    let view = editor(cx);
    view.update(cx, |e, w, cx| {
        e.backdrop_style(
            |b| {
                b.motion = crate::animation::Motion::Lava;
                b.seconds = 5;
            },
            cx,
        );
        let base = e.document.base.clone();
        e.animation_paused = true;
        e.animation_position = 1.25;
        assert!((e.animation_phase() - 0.25).abs() < 0.001);
        let track = Bounds::new(point(px(0.), px(0.)), size(px(130.), px(24.)));
        e.backdrop_drag = Some((crate::backdrop::Control::Duration, track));
        e.backdrop_slider_move(point(px(80.), px(12.)), cx);
        assert_eq!(e.document.backdrop.unwrap().seconds, 10);
        assert!(
            (e.animation_phase() - 0.25).abs() < 0.001,
            "duration preserves current phase"
        );
        e.backdrop_slider_move(point(px(-20.), px(12.)), cx);
        assert_eq!(e.document.backdrop.unwrap().seconds, 2);
        e.backdrop_slider_move(point(px(200.), px(12.)), cx);
        assert_eq!(e.document.backdrop.unwrap().seconds, 15);
        e.backdrop_drag = None;
        e.toggle_animation(cx);
        assert!(!e.animation_paused);
        e.toggle_animation(cx);
        assert!(e.animation_paused);
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        e.video_cancel = Some(cancel.clone());
        e.key(&key("escape"), w, cx);
        assert!(cancel.load(std::sync::atomic::Ordering::Relaxed));
        e.video_cancel = None;
        e.document.undo();
        assert!(e.document.backdrop.is_none());
        e.document.redo();
        assert_eq!(
            e.document.backdrop.unwrap().motion,
            crate::animation::Motion::Lava
        );
        assert!(Arc::ptr_eq(&base, &e.document.base));
    })
    .unwrap();
}
