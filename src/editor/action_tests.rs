use super::{Editor, actions::Action, jobs::OperationKind};
use crate::document::{Document, Tool};
use gpui::{AppContext, TestAppContext, VisualTestContext};

#[gpui::test]
fn mcp_transport_dispatches_actions_to_the_live_editor(cx: &mut TestAppContext) {
    let entity = cx.new(|cx| Editor::with_native(cx, false));
    let (sender, receiver) = async_channel::unbounded();
    let (messages, incoming) = std::sync::mpsc::channel();
    let proxy = std::thread::spawn(move || {
        while let Ok(message) = receiver.recv_blocking() {
            if messages.send(message).is_err() {
                break;
            }
        }
    });
    // Use the real transport parser and response path, without a desktop/socket.
    let worker = std::thread::spawn(move || {
        let selected = crate::automation::dispatch(
            &sender,
            "dispatch_action",
            serde_json::json!({
                "action": {"type":"select_tool", "tool":"arrow"}, "expected_revision":0
            }),
        )
        .unwrap();
        assert_eq!(selected["revision"], 0);
        assert!(selected["operation_id"].is_null());
        let state = crate::automation::dispatch(&sender, "get_editor_state", serde_json::json!({}))
            .unwrap();
        assert_eq!(state["tool"], "arrow");
        assert_eq!(state["busy"], false);
        assert!(
            crate::automation::dispatch(
                &sender,
                "dispatch_action",
                serde_json::json!({
                    "action": {"type":"select_tool", "tool":"pen"}, "expected_revision":99
                })
            )
            .is_err()
        );
    });
    for _ in 0..3 {
        let message = incoming
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        entity.update(cx, |e, cx| e.receive(message, cx));
    }
    worker.join().unwrap();
    proxy.join().unwrap();
    entity.read_with(cx, |e, _| assert_eq!(e.interaction.tool, Tool::Arrow));
}

#[gpui::test]
fn native_and_mcp_edits_produce_the_same_document_and_undo(cx: &mut TestAppContext) {
    use crate::{automation::Snapshot, document::actions::DocumentAction};
    let entity = cx.new(|cx| Editor::with_native(cx, false));
    entity.update(cx, |e, cx| {
        let mark = crate::document::Mark {
            tool: Tool::Arrow,
            points: vec![(10., 10.), (70., 50.)],
            curve: Some((30., 5.)),
            color: [255, 0, 0, 255],
            width: 3.,
            text: String::new(),
        };
        let mut snapshot = Snapshot {
            document: e.document.clone(),
            revision: 0,
            phase: 0.,
        };
        e.dispatch(
            Action::Edit {
                edit: DocumentAction::AddAnnotation { mark: mark.clone() },
            },
            cx,
        )
        .unwrap();
        crate::mcp::operate(
            "add_annotation",
            &serde_json::json!({"mark":mark}),
            &mut snapshot,
        )
        .unwrap();
        e.dispatch(
            Action::NudgeSelection {
                delta: (8., 3.),
                remember: true,
            },
            cx,
        )
        .unwrap();
        crate::mcp::operate(
            "move_annotation",
            &serde_json::json!({"id":"0:0","dx":8,"dy":3}),
            &mut snapshot,
        )
        .unwrap();
        assert_eq!(e.document.marks, snapshot.document.marks);
        assert_eq!(e.document.render(None), snapshot.document.render(None));
        e.dispatch(Action::Undo, cx).unwrap();
        crate::mcp::operate("undo", &serde_json::json!({}), &mut snapshot).unwrap();
        assert_eq!(e.document.marks, snapshot.document.marks);
        assert_eq!(e.document.marks[0].curve, Some((30., 5.)));
    });
}

#[gpui::test]
fn backdrop_actions_invalidate_prepared_mcp_work(cx: &mut TestAppContext) {
    let entity = cx.new(|cx| Editor::with_native(cx, false));
    entity.update(cx, |e, cx| {
        let prepared = Box::new(e.document.clone());
        let revision = e.preview.revision;
        e.dispatch(
            Action::SetBackdrop {
                backdrop: Some(crate::backdrop::Backdrop::default()),
            },
            cx,
        )
        .unwrap();
        assert!(
            e.dispatch(
                Action::ApplyPreparedDocument {
                    document: prepared,
                    revision,
                    replace: false
                },
                cx
            )
            .is_err()
        );
        assert!(e.document.backdrop.is_some());
        // A slider tick must also invalidate work snapped before the drag began.
        let prepared = Box::new(e.document.clone());
        let revision = e.preview.revision;
        e.dispatch(
            Action::BeginBackdropAdjustment {
                control: crate::backdrop::Control::Padding,
                track: (0., 0., 100., 24.),
                position: (90., 12.),
            },
            cx,
        )
        .unwrap();
        e.interaction.gesture = super::state::Gesture::Idle;
        assert!(e.preview.revision > revision);
        assert!(
            e.dispatch(
                Action::ApplyPreparedDocument {
                    document: prepared,
                    revision,
                    replace: false
                },
                cx
            )
            .is_err()
        );
        assert_eq!(e.document.backdrop.unwrap().padding, 180);
    });
}

#[gpui::test]
fn rectangle_gesture_dispatches_a_normalized_annotation(cx: &mut TestAppContext) {
    let view = cx.add_window(|window, cx| {
        let editor = Editor::with_native(cx, false);
        editor.focus.focus(window);
        editor
    });
    let root = view.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(*view, cx);
    visual.update(|_, cx| crate::menus::install(cx));
    visual.run_until_parked();
    visual.simulate_keystrokes("r");
    let layout = root.read_with(&visual, |e, _| e.viewport.layout.get());
    let p = |x, y| {
        gpui::point(
            gpui::px(layout.x + x * layout.scale),
            gpui::px(layout.y + y * layout.scale),
        )
    };
    visual.simulate_mouse_down(p(10., 10.), gpui::MouseButton::Left, Default::default());
    visual.simulate_mouse_move(p(80., 70.), gpui::MouseButton::Left, Default::default());
    visual.simulate_mouse_up(p(80., 70.), gpui::MouseButton::Left, Default::default());
    root.read_with(&visual, |e, _| {
        assert_eq!(e.document.marks.len(), 1);
        assert_eq!(e.document.marks[0].tool, Tool::Rectangle);
        assert_eq!(e.document.marks[0].points.len(), 2);
    });
}

#[gpui::test]
fn buttons_shortcuts_and_menu_actions_share_tool_selection(cx: &mut TestAppContext) {
    cx.update(crate::menus::install);
    let view = cx.add_window(|window, cx| {
        let editor = Editor::with_native(cx, false);
        editor.focus.focus(window);
        editor
    });
    let mut visual = VisualTestContext::from_window(*view, cx);
    visual.run_until_parked();
    visual.simulate_keystrokes("a");
    visual.run_until_parked();
    view.update(&mut visual, |e, _, _| {
        assert_eq!(e.interaction.tool, Tool::Arrow)
    })
    .unwrap();
    visual.dispatch_action(crate::menus::Pen);
    visual.run_until_parked();
    view.update(&mut visual, |e, _, _| {
        assert_eq!(e.interaction.tool, Tool::Pen)
    })
    .unwrap();
    let button = visual.debug_bounds("tool-arrow-up-right").unwrap();
    visual.simulate_click(button.center(), Default::default());
    visual.run_until_parked();
    view.update(&mut visual, |e, _, _| {
        assert_eq!(e.interaction.tool, Tool::Arrow)
    })
    .unwrap();
}

#[gpui::test]
fn rejected_actions_do_not_modify_state(cx: &mut TestAppContext) {
    let entity = cx.new(|cx| Editor::with_native(cx, false));
    entity.update(cx, |e, cx| {
        assert!(
            e.dispatch(
                Action::Resize {
                    scale: f32::NAN,
                    smart: false
                },
                cx
            )
            .is_err()
        );
        assert_eq!(e.panels.resize_scale, 2.);
        assert!(e.panels.resize_smart);
        e.start_operation(OperationKind::Upload).unwrap();
        assert!(
            e.dispatch(Action::SelectTool { tool: Tool::Arrow }, cx)
                .is_err()
        );
        assert_eq!(e.interaction.tool, Tool::Select);
        assert!(e.dispatch(Action::CancelExport, cx).is_ok());
        assert!(e.operations.active.is_some());
    });
}

#[gpui::test]
fn menu_undo_operates_on_inline_text_without_touching_document(cx: &mut TestAppContext) {
    let view = cx.add_window(|window, cx| {
        let mut editor = Editor::with_native(cx, false);
        editor.document = Document::new(image::RgbaImage::new(100, 100));
        editor.focus.focus(window);
        editor
    });
    let mut visual = VisualTestContext::from_window(*view, cx);
    view.update(&mut visual, |e, _, _| {
        e.interaction.text_edit = Some(crate::text::Edit::new(crate::document::Mark {
            tool: Tool::Text,
            points: vec![(10., 10.)],
            curve: None,
            color: [255; 4],
            width: 3.,
            text: String::new(),
        }));
        e.interaction
            .text_edit
            .as_mut()
            .unwrap()
            .replace_text("hello");
    })
    .unwrap();
    visual.run_until_parked();
    visual.dispatch_action(crate::menus::Undo);
    visual.run_until_parked();
    view.update(&mut visual, |e, _, _| {
        assert_eq!(e.interaction.text_edit.as_ref().unwrap().buffer.text(), "");
        assert!(e.document.marks.is_empty());
        assert_eq!(e.preview.revision, 0);
    })
    .unwrap();
}
