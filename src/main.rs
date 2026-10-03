mod animation;
mod arrow;
mod backdrop;
mod document;
mod drawing;
mod enhance;
mod gestures;
mod glance;
mod icons;
mod menus;
mod navigation;
#[cfg(test)]
mod performance;
mod platform;
mod selection;
#[cfg(test)]
mod stress_tests;
mod text;
mod video;
actions!(pachiri, [Quit]);
mod editor;
use editor::Editor;
pub(crate) use editor::{Layout, Message};
use gpui::*;
fn main() {
    let application = Application::new().with_assets(icons::Icons);
    application.on_reopen(|cx| cx.activate(true));
    application.run(|cx: &mut App| {
        cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        menus::install(cx);
        let bounds = Bounds::centered(None, size(px(1220.), px(860.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(1050.), px(600.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Pachiri".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    window.on_window_should_close(cx, |_, cx| {
                        cx.hide();
                        false
                    });
                    let editor = Editor::new(cx);
                    editor.focus.focus(window);
                    editor
                })
            },
        )
        .expect("Unable to open the editor");
        cx.activate(true);
    });
}
