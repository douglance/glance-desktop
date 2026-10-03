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
