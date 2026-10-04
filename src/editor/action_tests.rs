use super::{Editor, actions::Action, jobs::OperationKind};
use crate::document::{Document, Tool};
use gpui::{AppContext, TestAppContext, VisualTestContext};

#[gpui::test]
fn nebula_and_legacy_stars_use_shared_motion_action_and_canonical_readback(
    cx: &mut TestAppContext,
) {
    use crate::{animation::Motion, automation::Request, backdrop::Backdrop};
    for name in ["nebula", "stars"] {
        let entity = cx.new(|cx| Editor::with_native(cx, false));
        entity.update(cx, |e, cx| {
            let original = Backdrop {
                motion: Motion::Lava,
                seed: 42,
                ..Default::default()
            };
            e.dispatch(
                Action::SetBackdrop {
                    backdrop: Some(original),
                },
                cx,
            )
            .unwrap();
            let revision = e.preview.revision;
            let action =
                Action::from_json(serde_json::json!({"type":"select_motion","motion":name}))
                    .unwrap();
            let (reply, response) = std::sync::mpsc::channel();
            e.automation(
                Request::Dispatch {
                    action,
                    expected_revision: Some(revision),
                    reply,
                },
                cx,
            );
            response.recv().unwrap().unwrap();
            assert_eq!(e.document.backdrop.unwrap().motion, Motion::Nebula);
            assert_eq!(e.document.backdrop.unwrap().seed, 42);
            assert_eq!(e.preview.revision, revision + 1);
            let (reply, response) = std::sync::mpsc::channel();
            e.automation(Request::Snapshot(reply), cx);
            let snapshot = response.recv().unwrap().unwrap();
            assert_eq!(
                crate::automation::state(&snapshot)["backdrop"]["motion"],
                "nebula"
            );
            e.dispatch(Action::Undo, cx).unwrap();
            assert_eq!(e.document.backdrop, Some(original));
        });
    }
}

#[gpui::test]
fn randomize_motion_uses_bridge_state_revisions_and_undo(cx: &mut TestAppContext) {
    use crate::{animation::Motion, automation::Request, backdrop::Backdrop};
    let entity = cx.new(|cx| Editor::with_native(cx, false));
    let original = entity.update(cx, |e, cx| {
        let revision = e.preview.revision;
        assert!(
            e.dispatch(Action::RandomizeMotion { seed: None }, cx)
                .is_err()
        );
        assert_eq!(e.preview.revision, revision);
        let original = Backdrop {
            motion: Motion::Lava,
            ..Default::default()
        };
        e.dispatch(
            Action::SetBackdrop {
                backdrop: Some(original),
            },
            cx,
        )
        .unwrap();
        let base = e.document.base.clone();
        e.playback.paused = true;
        e.playback.position = 1.25;
        let bridge = |e: &mut Editor, cx: &mut gpui::Context<Editor>, seed, revision| {
            let (reply, response) = std::sync::mpsc::channel();
            e.automation(
                Request::Dispatch {
                    action: Action::RandomizeMotion { seed },
                    expected_revision: Some(revision),
                    reply,
                },
                cx,
            );
            response.recv().unwrap()
        };
        let revision = e.preview.revision;
        assert!(bridge(e, cx, Some(42), revision - 1).is_err());
        assert_eq!(e.document.backdrop, Some(original));
        bridge(e, cx, Some(42), revision).unwrap();
        assert_eq!(
            e.document.backdrop,
            Some(Backdrop {
                seed: 42,
                ..original
            })
        );
        assert_eq!(e.preview.revision, revision + 1);
        assert!(e.playback.paused);
        assert_eq!(e.playback.position, 1.25);
        assert!(std::sync::Arc::ptr_eq(&base, &e.document.base));
        let (reply, response) = std::sync::mpsc::channel();
        e.automation(Request::Snapshot(reply), cx);
        let snapshot = response.recv().unwrap().unwrap();
        assert_eq!(crate::automation::state(&snapshot)["backdrop"]["seed"], 42);
        bridge(e, cx, None, e.preview.revision).unwrap();
        let fresh = e.document.backdrop.unwrap().seed;
        assert_ne!(fresh, 0);
        assert_ne!(fresh, 42);
        bridge(e, cx, Some(0), e.preview.revision).unwrap();
        assert_eq!(e.document.backdrop, Some(original));
        bridge(e, cx, Some(42), e.preview.revision).unwrap();
        e.dispatch(Action::Undo, cx).unwrap();
        assert_eq!(e.document.backdrop, Some(original));
        original
    });
    entity.update(cx, |e, cx| {
        // Undo queues a foreground refresh; finish it before the next command.
        e.receive(
            super::Message::Preview(
                e.preview.revision,
                e.document.marks.len(),
                original.inside_padding,
                super::render_image(super::preview_base(&e.document)),
            ),
            cx,
        );
        e.dispatch(Action::Redo, cx).unwrap();
        assert_eq!(
            e.document.backdrop,
            Some(Backdrop {
                seed: 42,
                ..original
            })
        );
    });
}

#[gpui::test]
fn randomize_button_fits_and_changes_only_motion_seed(cx: &mut TestAppContext) {
    let view = cx.add_window(|window, cx| {
        let mut editor = Editor::with_native(cx, false);
        editor.focus.focus(window);
        editor.panels.backdrop = true;
        editor
            .dispatch(
                Action::SelectMotion {
                    motion: crate::animation::Motion::Prism,
                },
                cx,
            )
            .unwrap();
        editor
    });
    let root = view.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(*view, cx);
    visual.simulate_window_resize(*view, gpui::size(gpui::px(1050.), gpui::px(600.)));
    visual.run_until_parked();
    let button = visual.debug_bounds("backdrop-randomize").unwrap();
    let panel = visual.debug_bounds("backdrop-panel").unwrap();
    assert!(
        panel.contains(&button.origin) && panel.contains(&button.bottom_right()),
        "panel {panel:?}, button {button:?}"
    );
    let original = root.read_with(&visual, |e, _| e.document.backdrop.unwrap());
    visual.simulate_click(button.center(), Default::default());
    root.read_with(&visual, |e, _| {
        let randomized = e.document.backdrop.unwrap();
        assert_ne!(randomized.seed, original.seed);
        assert_eq!(
            randomized,
            crate::backdrop::Backdrop {
                seed: randomized.seed,
                ..original
            }
        );
    });
    root.update(&mut visual, |e, cx| e.dispatch(Action::Undo, cx).unwrap());
    root.read_with(&visual, |e, _| {
        assert_eq!(e.document.backdrop, Some(original))
    });
}

#[gpui::test]
fn incompatible_backdrop_duration_rejects_action_before_mutation(cx: &mut TestAppContext) {
    let entity = cx.new(|cx| Editor::with_native(cx, false));
    entity.update(cx, |e, cx| {
        e.dispatch(
            Action::SetImageAnimation {
                animation: crate::animation::ImageAnimation {
                    effect: crate::animation::Entrance::Pop,
                    duration_ms: 2000,
                    delay_ms: 1000,
                    seconds: 5,
                    exit: true,
                },
            },
            cx,
        )
        .unwrap();
        let revision = e.preview.revision;
        assert!(
            e.dispatch(
                Action::SetBackdrop {
                    backdrop: Some(crate::backdrop::Backdrop {
                        seconds: 2,
                        ..Default::default()
                    }),
                },
                cx
            )
            .is_err()
        );
        assert_eq!(e.preview.revision, revision);
        assert!(e.document.backdrop.is_none());
        assert_eq!(e.document.image_animation.seconds, 5);
    });
}

#[gpui::test]
fn entrance_actions_preserve_pixels_group_slider_undo_and_seek_without_history(
    cx: &mut TestAppContext,
) {
    use crate::animation::{AnimationControl, Entrance, ImageAnimation};
    let entity = cx.new(|cx| Editor::with_native(cx, false));
    entity.update(cx, |e, cx| {
        let base = e.document.base.clone();
        e.dispatch(
            Action::SelectEntrance {
                effect: Entrance::Tilt,
            },
            cx,
        )
        .unwrap();
        assert!(e.panels.animation && !e.panels.backdrop && !e.panels.enhance);
        assert!(
            e.document.backdrop.is_none(),
            "image animation does not force backdrop motion"
        );
        assert!(std::sync::Arc::ptr_eq(&base, &e.document.base));
        e.dispatch(
            Action::BeginAnimationAdjustment {
                control: AnimationControl::Duration,
                track: (0., 0., 180., 24.),
                position: (40., 12.),
            },
            cx,
        )
        .unwrap();
        e.dispatch(
            Action::SetAnimationControl {
                control: AnimationControl::Duration,
                value: 800,
            },
            cx,
        )
        .unwrap();
        e.dispatch(
            Action::SetAnimationControl {
                control: AnimationControl::Duration,
                value: 1500,
            },
            cx,
        )
        .unwrap();
        e.cancel_gesture();
        e.dispatch(Action::SeekAnimation { seconds: 1.5 }, cx)
            .unwrap();
        assert!(e.playback.paused);
        assert_eq!(e.clip_time(), 1.5);
        let revision = e.preview.revision;
        assert!(
            e.dispatch(
                Action::SetImageAnimation {
                    animation: ImageAnimation {
                        seconds: 2,
                        duration_ms: 2000,
                        delay_ms: 1000,
                        ..e.document.image_animation
                    }
                },
                cx
            )
            .is_err()
        );
        assert_eq!(e.preview.revision, revision);
        assert!(
            e.dispatch(Action::SeekAnimation { seconds: f32::NAN }, cx)
                .is_err()
        );
        e.document.undo();
        assert_eq!(
            e.document.image_animation.duration_ms, 1000,
            "one undo restores the entire slider drag; seeking adds none"
        );
        assert_eq!(e.document.image_animation.effect, Entrance::Tilt);
        e.document.undo();
        assert_eq!(e.document.image_animation.effect, Entrance::None);
        e.document.redo();
        assert_eq!(e.document.image_animation.effect, Entrance::Tilt);
        e.dispatch(Action::ReplayAnimation, cx).unwrap();
        assert!(!e.playback.paused);
        assert!(e.clip_time() < 0.1);
        e.dispatch(Action::SelectTool { tool: Tool::Pen }, cx)
            .unwrap();
        assert!(!e.panels.animation);
    });
}

#[gpui::test]
fn animation_sidebar_buttons_and_timeline_dispatch_at_minimum_window(cx: &mut TestAppContext) {
    use crate::animation::Entrance;
    let view = cx.add_window(|window, cx| {
        let editor = Editor::with_native(cx, false);
        editor.focus.focus(window);
        editor
    });
    let root = view.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(*view, cx);
    visual.simulate_window_resize(*view, gpui::size(gpui::px(1050.), gpui::px(600.)));
    root.update(&mut visual, |e, cx| {
        e.dispatch(Action::ToggleAnimationPanel, cx).unwrap()
    });
    visual.run_until_parked();
    let panel = visual.debug_bounds("animation-panel").unwrap();
    for selector in ["entrance-Diagonal", "entrance-Pop", "entrance-Tilt"] {
        let b = visual.debug_bounds(selector).unwrap();
        assert!(panel.contains(&b.origin) && panel.contains(&b.bottom_right()));
    }
    let b = visual.debug_bounds("entrance-Diagonal").unwrap();
    visual.simulate_click(b.center(), Default::default());
    root.read_with(&visual, |e, _| {
        assert_eq!(e.document.image_animation.effect, Entrance::Diagonal)
    });
    let timeline = visual.debug_bounds("animation-Preview time").unwrap();
    visual.simulate_click(timeline.center(), Default::default());
    root.read_with(&visual, |e, _| {
        assert!(e.playback.paused);
        assert!((e.clip_time() - 2.5).abs() < 0.05);
        assert!(e.document.marks.is_empty());
    });
    visual.simulate_event(gpui::ScrollWheelEvent {
        position: panel.center(),
        delta: gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.), gpui::px(-180.))),
        ..Default::default()
    });
    root.read_with(&visual, |e, _| {
        assert_eq!(
            e.viewport.pan,
            (0., 0.),
            "sidebar scrolling must not pan the image"
        );
    });
    let canvas = root.read_with(&visual, |e, _| e.viewport.canvas_bounds.get());
    visual.simulate_event(gpui::ScrollWheelEvent {
        position: canvas.center(),
        delta: gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.), gpui::px(-20.))),
        ..Default::default()
    });
    root.read_with(&visual, |e, _| assert_eq!(e.viewport.pan, (0., -20.)));
}

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
        let formatted = crate::automation::dispatch(
            &sender,
            "dispatch_action",
            serde_json::json!({
                "action":{"type":"set_backdrop_format","format":"shorts"},
                "expected_revision":0
            }),
        )
        .unwrap();
        assert_eq!(formatted["revision"], 1);
        let document =
            crate::automation::dispatch(&sender, "get_document", serde_json::json!({})).unwrap();
        assert_eq!(document["backdrop"]["format"], "shorts");
        let toggled = crate::automation::dispatch(
            &sender,
            "dispatch_action",
            serde_json::json!({
                "action":{"type":"toggle_backdrop_enabled"},"expected_revision":1
            }),
        )
        .unwrap();
        assert_eq!(toggled["revision"], 2);
        let document =
            crate::automation::dispatch(&sender, "get_document", serde_json::json!({})).unwrap();
        assert!(document["backdrop"].is_null());
        let undone = crate::automation::dispatch(
            &sender,
            "dispatch_action",
            serde_json::json!({"action":{"type":"undo"},"expected_revision":2}),
        )
        .unwrap();
        assert_eq!(undone["revision"], 3);
        // Undo starts a preview refresh. Its receipt accepts the work; document
        // snapshot tools correctly reject requests until that work finishes.
    });
    for _ in 0..8 {
        let message = incoming
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        entity.update(cx, |e, cx| e.receive(message, cx));
    }
    worker.join().unwrap();
    proxy.join().unwrap();
    entity.read_with(cx, |e, _| {
        assert_eq!(e.interaction.tool, Tool::Arrow);
        assert_eq!(
            e.document.backdrop.unwrap().format,
            crate::backdrop::Format::Shorts
        );
    });
}

#[gpui::test]
fn native_and_mcp_edits_produce_the_same_document_and_undo(cx: &mut TestAppContext) {
    use crate::{automation::Snapshot, document::actions::DocumentAction};
    let entity = cx.new(|cx| Editor::with_native(cx, false));
    entity.update(cx, |e, cx| {
        let mark = crate::document::Mark {
            style: Default::default(),
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

#[test]
fn malformed_mcp_actions_are_rejected_before_entering_the_editor_queue() {
    let (sender, receiver) = async_channel::unbounded();
    for action in [
        serde_json::json!({"type":"fit","scale":2}),
        serde_json::json!({"type":"select_tool","tool":"unknown"}),
        serde_json::json!({"type":"set_backdrop","backdrop":{"paddding":20}}),
        serde_json::json!({"type":"apply_prepared_document","revision":0}),
    ] {
        assert!(
            crate::automation::dispatch(
                &sender,
                "dispatch_action",
                serde_json::json!({"action":action})
            )
            .is_err()
        );
        assert!(receiver.try_recv().is_err());
    }
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
            style: Default::default(),
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

#[test]
fn sidebar_actions_are_accepted_by_the_mcp_schema() {
    for action in [
        serde_json::json!({"type":"set_appearance","style":{"dash":"dashed","start":"dot","end":"arrow"}}),
        serde_json::json!({"type":"set_magnifier_zoom","zoom":3}),
        serde_json::json!({"type":"set_counter_number","number":7}),
        serde_json::json!({"type":"set_crop_ratio","ratio":null}),
        serde_json::json!({"type":"add_line_point"}),
        serde_json::json!({"type":"toggle_animation_panel"}),
        serde_json::json!({"type":"select_entrance","effect":"diagonal"}),
        serde_json::json!({"type":"set_image_animation","animation":{"effect":"tilt","exit":true}}),
        serde_json::json!({"type":"set_animation_control","control":"duration","value":800}),
        serde_json::json!({"type":"seek_animation","seconds":1.5}),
        serde_json::json!({"type":"replay_animation"}),
        serde_json::json!({"type":"straighten_line"}),
    ] {
        crate::mcp::validate_tool("dispatch_action", &serde_json::json!({"action":action}))
            .unwrap();
        Action::from_json(action).unwrap();
    }
}

#[gpui::test]
fn inside_padding_preserves_capture_and_annotations_and_groups_slider_history(
    cx: &mut TestAppContext,
) {
    use crate::backdrop::{Backdrop, Control};
    use std::sync::Arc;
    let entity = cx.new(|cx| Editor::with_native(cx, false));
    entity.update(cx, |e, cx| {
        e.dispatch(
            Action::SetBackdrop {
                backdrop: Some(Backdrop::default()),
            },
            cx,
        )
        .unwrap();
        let base = e.document.base.clone();
        let marks = e.document.marks.clone();
        let dimensions = base.dimensions();
        e.dispatch(
            Action::BeginBackdropAdjustment {
                control: Control::InsidePadding,
                track: (0., 0., 200., 24.),
                position: (20., 12.),
            },
            cx,
        )
        .unwrap();
        e.dispatch(
            Action::SetBackdropControl {
                control: Control::InsidePadding,
                value: 48,
            },
            cx,
        )
        .unwrap();
        e.cancel_gesture();
        assert!(Arc::ptr_eq(&base, &e.document.base));
        assert_eq!(e.document.marks, marks);
        assert_eq!(
            e.document.export().dimensions(),
            (dimensions.0 + 224, dimensions.1 + 224)
        );
        let preview = super::preview_base(&e.document);
        let expected_ratio = (dimensions.0 + 96) as f32 / (dimensions.1 + 96) as f32;
        assert!((preview.width() as f32 / preview.height() as f32 - expected_ratio).abs() < 0.002);
        let revision = e.preview.revision;
        e.dispatch(Action::Undo, cx).unwrap();
        assert_eq!(e.document.backdrop.unwrap().inside_padding, 0);
        assert!(e.preview.revision > revision);
        e.receive(
            super::Message::Preview(
                e.preview.revision,
                marks.len(),
                0,
                super::render_image(super::preview_base(&e.document)),
            ),
            cx,
        );
        e.dispatch(Action::Redo, cx).unwrap();
        assert_eq!(e.document.backdrop.unwrap().inside_padding, 48);
        e.receive(
            super::Message::Preview(
                e.preview.revision,
                marks.len(),
                48,
                super::render_image(super::preview_base(&e.document)),
            ),
            cx,
        );
        e.dispatch(Action::ToggleBackdropEnabled, cx).unwrap();
        assert_eq!(e.document.export().dimensions(), dimensions);
        let revision = e.preview.revision;
        // A different slider can re-enable a saved style with inside padding.
        e.dispatch(
            Action::BeginBackdropAdjustment {
                control: Control::Shadow,
                track: (0., 0., 60., 24.),
                position: (24., 12.),
            },
            cx,
        )
        .unwrap();
        e.cancel_gesture();
        assert_eq!(e.document.backdrop.unwrap().inside_padding, 48);
        assert!(e.preview.revision > revision);
        assert!(e.preview.rendering);
        assert!(
            e.dispatch(
                Action::SetBackdropControl {
                    control: Control::InsidePadding,
                    value: 201
                },
                cx
            )
            .is_err()
        );
        assert!(
            e.dispatch(
                Action::SetBackdrop {
                    backdrop: Some(Backdrop {
                        inside_padding: 513,
                        ..Default::default()
                    })
                },
                cx
            )
            .is_err()
        );
    });
}
