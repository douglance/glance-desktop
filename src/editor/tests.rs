//! Tests use GPUI's virtual platform. No desktop interaction or capture permission.
use super::feedback::CopyFeedback;
use super::{Document, Editor, Layout, Message, Tool, render_image};
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
        e.preview.image = render_image((*e.document.base).clone());
        e.viewport.layout.set(Layout {
            x: 0.,
            y: 0.,
            scale: 1.,
            width: 100.,
            height: 100.,
        });
        e.viewport
            .canvas_bounds
            .set(Bounds::new(point(px(0.), px(0.)), size(px(100.), px(100.))));
        e.focus.focus(window);
        e
    })
}
fn reset_layout(e: &Editor) {
    e.viewport.layout.set(Layout {
        x: 0.,
        y: 0.,
        scale: 1.,
        width: 100.,
        height: 100.,
    });
    e.viewport
        .canvas_bounds
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
fn remote_copy_toolbar_fits_at_minimum_window_width(cx: &mut TestAppContext) {
    let view = editor(cx);
    let mut visual = gpui::VisualTestContext::from_window(*view, cx);
    visual.simulate_resize(size(px(1050.), px(600.)));
    visual.run_until_parked();
    for selector in ["copy-remote", "header-zoom"] {
        let bounds = visual.debug_bounds(selector).unwrap();
        assert!(bounds.size.width > px(0.));
        assert!(bounds.origin.x >= px(0.));
        assert!(
            bounds.right() <= px(1050.),
            "{selector} overflows: {bounds:?}"
        );
    }
}
#[gpui::test]
fn remote_copy_updates_clipboard_only_after_upload_success(cx: &mut TestAppContext) {
    let view = editor(cx);
    view.update(cx, |e, _, cx| {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("existing clipboard".into()));
        e.start_operation(super::jobs::OperationKind::Upload)
            .unwrap();
        e.copy_remote(cx); // A second request must not start another upload.
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            "existing clipboard"
        );
        complete(
            e,
            super::jobs::OperationResult::RemoteCopied(Ok(crate::glance::Share {
                url: "https://glance.sh/example.png".into(),
                expires_at: u64::MAX,
            })),
            cx,
        );
        assert!(!e.is_busy());
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            "Screenshot: https://glance.sh/example.png"
        );
        assert!(e.feedback.status.contains("Glance link copied"));
        assert!(matches!(e.feedback.copy, Some(CopyFeedback::LinkCopied(_))));
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("keep on failure".into()));
        e.start_operation(super::jobs::OperationKind::Upload)
            .unwrap();
        complete(
            e,
            super::jobs::OperationResult::RemoteCopied(Err("Offline".into())),
            cx,
        );
        assert!(!e.is_busy());
        assert_eq!(e.feedback.status, "Offline");
        assert_eq!(e.feedback.copy, None);
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            "keep on failure"
        );
    })
    .unwrap();
}
#[gpui::test]
fn copy_confirmation_expires_without_dismissing_a_new_upload(cx: &mut TestAppContext) {
    use std::time::Duration;
    let view = editor(cx);
    view.update(cx, |e, _, cx| {
        complete(e, super::jobs::OperationResult::Copied(Ok(())), cx)
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(1));
    view.update(cx, |e, _, cx| {
        assert_eq!(e.feedback.copy, Some(CopyFeedback::Copied));
        e.set_copy_feedback(Some(CopyFeedback::Uploading), cx);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_secs(5));
    cx.run_until_parked();
    view.update(cx, |e, _, cx| {
        assert_eq!(e.feedback.copy, Some(CopyFeedback::Uploading));
        complete(
            e,
            super::jobs::OperationResult::RemoteCopied(Ok(crate::glance::Share {
                url: "https://glance.sh/example.png".into(),
                expires_at: u64::MAX,
            })),
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(3));
    cx.run_until_parked();
    view.update(cx, |e, _, _| assert_eq!(e.feedback.copy, None))
        .unwrap();
}
#[gpui::test]
fn copy_confirmation_fits_and_refreshes_on_repeated_copy(cx: &mut TestAppContext) {
    use std::time::Duration;
    let view = editor(cx);
    view.update(cx, |e, _, cx| {
        complete(e, super::jobs::OperationResult::Copied(Ok(())), cx)
    })
    .unwrap();
    let mut visual = gpui::VisualTestContext::from_window(*view, cx);
    visual.simulate_resize(size(px(1050.), px(600.)));
    visual.run_until_parked();
    let bounds = visual.debug_bounds("copy-feedback").unwrap();
    assert!(bounds.origin.x >= px(0.));
    assert!(bounds.origin.y >= px(48.));
    assert!(bounds.right() <= px(1050.));
    cx.executor().advance_clock(Duration::from_secs(1));
    view.update(cx, |e, _, cx| {
        complete(e, super::jobs::OperationResult::Copied(Ok(())), cx)
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    view.update(cx, |e, _, _| {
        assert_eq!(e.feedback.copy, Some(CopyFeedback::Copied))
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    view.update(cx, |e, _, cx| {
        assert_eq!(e.feedback.copy, None);
        complete(
            e,
            super::jobs::OperationResult::Copied(Err("Clipboard unavailable".into())),
            cx,
        );
        assert_eq!(e.feedback.copy, None);
    })
    .unwrap();
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
        assert_eq!(e.interaction.selected, Some(0));
        e.motion(&motion(40., 30.), cx);
        assert_eq!(e.document.marks[0].points[0], (10., 10.));
        e.finish(&up(40., 30.), cx);
        assert_eq!(e.document.marks[0].points[0], (30., 20.));
        e.duplicate_selected(cx);
        assert_eq!(e.document.marks.len(), 2);
        assert_eq!(e.interaction.selected, Some(1));
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
fn cancel_gesture_and_shift_release_preserve_document(cx: &mut TestAppContext) {
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
        assert!(e.interaction.selected.is_none());
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
        assert_eq!(e.viewport.pan, (20., 30.));
        assert!(e.document.marks.is_empty());
        e.viewport.space_down = false;
        e.receive(Message::Magnify(0.5, (30., 40.), false), cx);
        assert_eq!(e.viewport.zoom, Some(1.5));
        assert!(e.document.marks.is_empty());
        e.key(&key("cmd-1"), w, cx);
        assert!(e.viewport.zoom.is_none());
        assert_eq!(e.viewport.pan, (0., 0.));
        e.viewport.zoom_down = true;
        e.begin(&down(20., 20.), w, cx);
        assert_eq!(e.viewport.zoom, Some(2.));
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
            e.interaction.text_edit.as_ref().unwrap().buffer.text(),
            "Hi 👨‍👩‍👧‍👦 café 日本語"
        );
        e.key(&key("enter"), w, cx);
        assert_eq!(e.document.marks.len(), 1);
        e.begin(&down(50., 50.), w, cx);
        e.replace_text_in_range(None, "discard", w, cx);
        e.key(&key("escape"), w, cx);
        assert_eq!(e.document.marks.len(), 1);
        assert!(e.interaction.text_edit.is_none());
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
        let old = e.preview.image.clone();
        e.preview.revision = 5;
        e.receive(
            Message::Preview(4, 99, 40, render_image(image::RgbaImage::new(2, 2))),
            cx,
        );
        assert!(Arc::ptr_eq(&e.preview.image, &old));
        assert_ne!(e.preview.mark_count, 99);
        assert_eq!(e.preview.inside_padding, 0);
        e.receive(
            Message::Preview(5, 0, 12, render_image(image::RgbaImage::new(100, 100))),
            cx,
        );
        assert!(!Arc::ptr_eq(&e.preview.image, &old));
        assert_eq!(e.preview.mark_count, 0);
        assert_eq!(e.preview.inside_padding, 12);
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
    assert_eq!(
        view.read_with(&visual, |e, _| e.interaction.tool),
        Tool::Pen
    );
    let l = view.read_with(&visual, |e, _| e.viewport.layout.get());
    let pos = |x, y| point(px(l.x + x * l.scale), px(l.y + y * l.scale));
    visual.simulate_mouse_down(pos(10., 10.), MouseButton::Left, Default::default());
    visual.simulate_mouse_move(pos(40., 40.), MouseButton::Left, Default::default());
    visual.simulate_mouse_up(pos(40., 40.), MouseButton::Left, Default::default());
    assert_eq!(view.read_with(&visual, |e, _| e.document.marks.len()), 1);
    visual.simulate_keystrokes("t");
    visual.simulate_mouse_down(pos(50., 50.), MouseButton::Left, Default::default());
    visual.simulate_mouse_up(pos(50., 50.), MouseButton::Left, Default::default());
    assert_eq!(
        view.read_with(&visual, |e, _| e.interaction.tool),
        Tool::Text
    );
    assert!(
        view.read_with(&visual, |e, _| e.interaction.text_edit.is_some()),
        "text click must open canvas editor"
    );
    visual.simulate_input("p a r t h b x n v z hello café 日本語");
    assert_eq!(
        view.read_with(&visual, |e, _| e
            .interaction
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
        assert_eq!(
            e.interaction.text_edit.as_ref().unwrap().buffer.text(),
            "👋 日本"
        );
        assert_eq!(e.marked_text_range(w, cx), Some(3..5));
        assert_eq!(e.selected_text_range(false, w, cx).unwrap().range, 5..5);
        e.key(&key("enter"), w, cx);
        assert!(
            e.interaction.text_edit.is_some(),
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
        e.interaction.selected = Some(0);
        let color = e.document.marks[0].color;
        e.interaction.color = [0, 120, 255, 255];
        e.apply_style(true, cx);
        assert_eq!(e.document.marks[0].color, e.interaction.color);
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
        assert_eq!(e.interaction.selected, Some(0));
        assert_eq!(e.interaction.tool, Tool::Arrow);
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
        e.interaction.selected = Some(0);
        // Move the shaft while the Arrow tool remains active, away from the handles.
        let p = crate::arrow::at(&curved, 0.25);
        e.begin(&down(p.0, p.1), w, cx);
        e.finish(&up(p.0 + 5., p.1 + 5.), cx);
        assert_eq!(e.document.marks[0].points[0], (20., 65.));
        assert_eq!(crate::arrow::at(&e.document.marks[0], 0.5), (55., 30.));
        e.key(&key("backspace"), w, cx);
        assert!(e.document.marks.is_empty());
        e.receive(
            Message::Preview(
                e.preview.revision,
                0,
                0,
                render_image((*e.document.base).clone()),
            ),
            cx,
        );
        e.set_tool(Tool::Rectangle, cx);
        e.begin(&down(10., 10.), w, cx);
        e.finish(&up(40., 40.), cx);
        assert_eq!(e.interaction.selected, Some(0));
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
        e.playback.paused = true;
        e.playback.position = 1.25;
        assert!((e.animation_phase() - 0.25).abs() < 0.001);
        let track = Bounds::new(point(px(0.), px(0.)), size(px(130.), px(24.)));
        e.interaction.gesture =
            super::Gesture::AdjustingBackdrop(crate::backdrop::Control::Duration, track);
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
        e.interaction.gesture = super::Gesture::Idle;
        e.toggle_animation(cx);
        assert!(!e.playback.paused);
        e.toggle_animation(cx);
        assert!(e.playback.paused);
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        e.video_export.cancel = Some(cancel.clone());
        e.key(&key("escape"), w, cx);
        assert!(cancel.load(std::sync::atomic::Ordering::Relaxed));
        e.video_export.cancel = None;
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

#[gpui::test]
fn switching_pointer_gestures_cancels_the_previous_drag(cx: &mut TestAppContext) {
    let view = editor(cx);
    view.update(cx, |e, w, cx| {
        reset_layout(e);
        e.set_tool(Tool::Rectangle, cx);
        e.begin(&down(10., 10.), w, cx);
        e.finish(&up(30., 30.), cx);
        let original = e.document.marks[0].points.clone();
        e.begin(&down(10., 10.), w, cx);
        e.motion(&motion(20., 20.), cx);
        assert!(e.interaction.gesture.drag().is_some());
        e.begin_pan(point(px(40.), px(40.)), cx);
        assert!(matches!(e.interaction.gesture, super::Gesture::Panning(_)));
        assert_eq!(e.document.marks[0].points, original);
        e.key(&key("escape"), w, cx);
        assert!(!e.interaction.gesture.is_active());
        e.set_tool(Tool::Pen, cx);
        e.begin(&down(50., 50.), w, cx);
        assert!(e.interaction.gesture.draft().is_some());
        e.key(&key("escape"), w, cx);
        e.finish(&up(60., 60.), cx);
        assert_eq!(e.document.marks.len(), 1);
    })
    .unwrap();
}

fn complete(e: &mut Editor, result: super::jobs::OperationResult, cx: &mut gpui::Context<Editor>) {
    let kind = match &result {
        super::jobs::OperationResult::RemoteCopied(_) => super::jobs::OperationKind::Upload,
        super::jobs::OperationResult::Copied(_) => super::jobs::OperationKind::Copy,
        _ => panic!("Choose the operation kind for this test result"),
    };
    let id = e
        .operations
        .active
        .as_ref()
        .map(|op| op.id)
        .unwrap_or_else(|| e.start_operation(kind).unwrap());
    e.receive(Message::Operation(id, result), cx);
}

#[gpui::test]
fn stale_operation_completion_cannot_change_clipboard_or_clear_new_job(cx: &mut TestAppContext) {
    let view = editor(cx);
    view.update(cx, |e, _, cx| {
        use super::jobs::{OperationKind, OperationResult};
        let old = e.start_operation(OperationKind::Upload).unwrap();
        e.receive(
            Message::Operation(old, OperationResult::RemoteCopied(Err("Offline".into()))),
            cx,
        );
        let current = e.start_operation(OperationKind::Copy).unwrap();
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("keep this".into()));
        e.set_copy_feedback(Some(CopyFeedback::Copying), cx);
        e.receive(
            Message::Operation(
                old,
                OperationResult::RemoteCopied(Ok(crate::glance::Share {
                    url: "https://glance.sh/stale.png".into(),
                    expires_at: u64::MAX,
                })),
            ),
            cx,
        );
        assert_eq!(e.operations.active.as_ref().unwrap().id, current);
        assert_eq!(e.feedback.copy, Some(CopyFeedback::Copying));
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            "keep this"
        );
        e.receive(
            Message::Operation(current, OperationResult::Copied(Ok(()))),
            cx,
        );
        assert!(!e.is_busy());
        assert_eq!(e.feedback.copy, Some(CopyFeedback::Copied));
    })
    .unwrap();
}

#[gpui::test]
fn preview_completion_does_not_unlock_an_active_operation(cx: &mut TestAppContext) {
    let view = editor(cx);
    view.update(cx, |e, _, cx| {
        use super::jobs::{OperationKind, OperationResult};
        let id = e.start_operation(OperationKind::Copy).unwrap();
        e.preview.waiting = true;
        e.receive(
            Message::Preview(
                e.preview.revision,
                0,
                0,
                render_image((*e.document.base).clone()),
            ),
            cx,
        );
        assert!(!e.preview.waiting);
        assert!(e.is_busy());
        assert!(e.start_operation(OperationKind::Upload).is_none());
        e.receive(Message::Operation(id, OperationResult::Copied(Ok(()))), cx);
        assert!(!e.is_busy());
        let id = e.start_operation(OperationKind::Open).unwrap();
        e.receive(
            Message::Operation(
                id,
                OperationResult::Image(Ok(Some(image::RgbaImage::new(30, 20)))),
            ),
            cx,
        );
        assert!(e.operations.active.is_none());
        assert!(e.is_busy()); // The new image must finish preparing before editing resumes.
        e.receive(
            Message::Preview(
                e.preview.revision,
                0,
                0,
                render_image((*e.document.base).clone()),
            ),
            cx,
        );
        assert!(!e.is_busy());
    })
    .unwrap();
}

#[gpui::test]
fn video_progress_belongs_to_the_active_export(cx: &mut TestAppContext) {
    let view = editor(cx);
    view.update(cx, |e, _, cx| {
        use super::jobs::{OperationKind, OperationResult};
        let old = e.start_operation(OperationKind::Video).unwrap();
        e.receive(Message::VideoProgress(old, 40), cx);
        assert_eq!(e.video_export.progress, Some(40));
        e.receive(
            Message::Operation(old, OperationResult::VideoSaved(Ok(None))),
            cx,
        );
        let current = e.start_operation(OperationKind::Video).unwrap();
        e.video_export.progress = Some(0);
        e.receive(Message::VideoProgress(old, 90), cx);
        assert_eq!(e.video_export.progress, Some(0));
        e.receive(Message::VideoProgress(current, 25), cx);
        assert_eq!(e.video_export.progress, Some(25));
        e.receive(
            Message::Operation(current, OperationResult::VideoSaved(Ok(None))),
            cx,
        );
        assert!(!e.is_busy());
        assert_eq!(e.video_export.progress, None);
    })
    .unwrap();
}

#[gpui::test]
fn automation_applies_to_native_editor_and_rejects_stale_work(cx: &mut TestAppContext) {
    use crate::automation::{Request, Snapshot};
    use serde_json::json;
    let view = editor(cx);
    view.update(cx, |e, _, cx| {
        let mut snapshot = Snapshot {
            document: e.document.clone(),
            revision: e.preview.revision,
            phase: 0.,
        };
        crate::mcp::operate(
            "add_annotation",
            &json!({"mark": {
                "tool": "arrow", "points": [[10, 10], [60, 40]],
                "color": [255, 0, 0, 255], "width": 3, "text": ""
            }}),
            &mut snapshot,
        )
        .unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        e.automation(
            Request::Apply {
                document: snapshot.document.clone(),
                revision: snapshot.revision,
                replace: false,
                reply: tx,
            },
            cx,
        );
        assert!(rx.recv().unwrap().is_ok());
        assert_eq!(e.interaction.selected, Some(0));
        assert_eq!(e.interaction.tool, Tool::Select);
        assert_eq!(e.document.marks.len(), 1);
        let (tx, rx) = std::sync::mpsc::channel();
        e.automation(
            Request::Apply {
                document: snapshot.document,
                revision: snapshot.revision,
                replace: false,
                reply: tx,
            },
            cx,
        );
        assert!(rx.recv().unwrap().is_err());
        assert_eq!(e.document.marks.len(), 1);
        e.document.undo();
        assert!(e.document.marks.is_empty());
        e.start_operation(super::jobs::OperationKind::Upload)
            .unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        e.automation(Request::Snapshot(tx), cx);
        assert!(rx.recv().unwrap().is_err());
    })
    .unwrap();
}

#[gpui::test]
fn spotlight_magnifier_creation_handles_and_native_undo(cx: &mut TestAppContext) {
    let view = editor(cx);
    view.update(cx, |e, w, cx| {
        reset_layout(e);
        e.set_tool(Tool::Spotlight, cx);
        e.begin(&down(10., 10.), w, cx);
        e.motion(&motion(60., 60.), cx);
        e.finish(&up(60., 60.), cx);
        assert_eq!(e.document.marks[0].tool, Tool::Spotlight);
        assert_eq!(e.interaction.selected, Some(0));
        assert_eq!(e.document.marks[0].points.len(), 2);
        e.begin(&down(60., 60.), w, cx);
        assert_eq!(
            e.interaction.gesture.drag().and_then(|(_, handle)| handle),
            Some(2)
        );
        e.finish(&up(70., 65.), cx);
        assert_eq!(e.document.marks[0].points, vec![(10., 10.), (70., 65.)]);
        e.document.undo();
        e.set_tool(Tool::Magnifier, cx);
        e.begin(&down(20., 20.), w, cx);
        e.motion(&motion(75., 75.), cx);
        e.finish(&up(75., 75.), cx);
        assert_eq!(e.document.marks[1].points, vec![(20., 20.), (75., 75.)]);
        assert_eq!(e.interaction.selected, Some(1));
        e.begin(&down(75., 75.), w, cx);
        assert_eq!(
            e.interaction.gesture.drag().and_then(|(_, handle)| handle),
            Some(2)
        );
        e.motion(&motion(85., 70.), cx);
        e.finish(&up(85., 70.), cx);
        assert_eq!(e.document.marks[1].points, vec![(20., 20.), (85., 70.)]);
        e.document.undo();
        assert_eq!(e.document.marks[1].points[1], (75., 75.));
        e.begin(&down(20., 20.), w, cx);
        assert_eq!(
            e.interaction.gesture.drag().and_then(|(_, handle)| handle),
            Some(0)
        );
        e.finish(&up(30., 25.), cx);
        assert_eq!(e.document.marks[1].points, vec![(30., 25.), (75., 75.)]);
        e.delete_selected(cx);
        assert_eq!(e.document.marks.len(), 1);
        e.document.undo();
        assert_eq!(e.document.marks.len(), 2);
    })
    .unwrap();
}

#[gpui::test]
fn backdrop_grid_modes_and_format_menu_work_at_minimum_window_size(cx: &mut TestAppContext) {
    use crate::backdrop::Format;
    let view = editor(cx);
    view.update(cx, |e, _, cx| e.toggle_backdrop(cx)).unwrap();
    let mut visual = gpui::VisualTestContext::from_window(*view, cx);
    visual.simulate_resize(size(px(1050.), px(600.)));
    visual.run_until_parked();
    let click = |visual: &mut gpui::VisualTestContext, selector: &'static str| {
        let point = visual.debug_bounds(selector).unwrap().center();
        visual.simulate_mouse_down(point, MouseButton::Left, Default::default());
        visual.simulate_mouse_up(point, MouseButton::Left, Default::default());
        visual.run_until_parked();
    };
    for (mode, selector) in [
        ("Motion", "backdrop-mode-Motion"),
        ("Gradient", "backdrop-mode-Gradient"),
        ("Solid", "backdrop-mode-Solid"),
    ] {
        click(&mut visual, selector);
        let actual = view
            .read_with(&visual, |e, _| e.document.backdrop.unwrap())
            .unwrap();
        assert_eq!(
            actual.motion != crate::animation::Motion::Still,
            mode == "Motion"
        );
        if mode != "Motion" {
            assert_eq!(actual.gradient, mode == "Gradient");
        }
        let mode_bounds = visual.debug_bounds(selector).unwrap();
        let bounds: Vec<_> = [
            "backdrop-Outside padding",
            "backdrop-Inside padding",
            "backdrop-Image corners",
            "backdrop-Shadow",
        ]
        .into_iter()
        .map(|selector| visual.debug_bounds(selector).unwrap())
        .collect();
        assert_eq!(bounds[0].top(), bounds[1].top());
        assert_eq!(bounds[2].top(), bounds[3].top());
        assert!(bounds[0].right() < bounds[1].left());
        for bound in bounds {
            assert!(
                bound.bottom() < mode_bounds.top(),
                "Shared control must be above the modes"
            );
            assert!(bound.right() <= px(1050.) && bound.bottom() <= px(600.));
        }
        // GPUI retains debug selectors from older frames; only assert presence
        // for visible controls, and verify mode transitions through document state.
        if mode == "Motion" {
            assert!(visual.debug_bounds("backdrop-duration").is_some());
            assert!(visual.debug_bounds("backdrop-motion-effects").is_some());
        }
    }
    click(&mut visual, "backdrop-format");
    assert!(visual.debug_bounds("backdrop-popup").is_some());
    visual.simulate_keystrokes("down enter");
    visual.run_until_parked();
    assert_eq!(
        view.read_with(&visual, |e, _| e.document.backdrop.unwrap().format)
            .unwrap(),
        Format::Square
    );
    assert!(
        view.read_with(&visual, |e, _| e.panels.popup.is_none())
            .unwrap()
    );
    view.update(&mut visual, |e, _, _| e.document.undo())
        .unwrap();
    assert_eq!(
        view.read_with(&visual, |e, _| e.document.backdrop.unwrap().format)
            .unwrap(),
        Format::Auto
    );
    click(&mut visual, "backdrop-format");
    click(&mut visual, "popup-option-6");
    assert_eq!(
        view.read_with(&visual, |e, _| e.document.backdrop.unwrap().format)
            .unwrap(),
        Format::Vertical
    );
    click(&mut visual, "backdrop-enabled");
    assert!(
        view.read_with(&visual, |e, _| e.document.backdrop.is_none())
            .unwrap()
    );
    click(&mut visual, "backdrop-enabled");
    assert_eq!(
        view.read_with(&visual, |e, _| e.document.backdrop.unwrap().format)
            .unwrap(),
        Format::Vertical
    );
    click(&mut visual, "export-trigger");
    assert!(visual.debug_bounds("backdrop-popup").is_some());
    visual.simulate_keystrokes("escape");
    visual.run_until_parked();
    assert!(
        view.read_with(&visual, |e, _| e.panels.popup.is_none())
            .unwrap()
    );
}
