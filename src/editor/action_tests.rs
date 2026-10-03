use super::{Editor, actions::Action, jobs::OperationKind};
use crate::document::{Document, Tool};
use gpui::{AppContext, TestAppContext, VisualTestContext};

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
