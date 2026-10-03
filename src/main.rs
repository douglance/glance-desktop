mod animation;
mod arrow;
mod automation;
mod backdrop;
mod document;
mod drawing;
mod effects;
mod enhance;
mod gestures;
mod gif_export;
mod glance;
mod icons;
mod mcp;
mod menus;
mod motion_shader;
mod navigation;
#[cfg(test)]
mod performance;
mod platform;
mod selection;
#[cfg(test)]
mod stress_tests;
mod text;
mod video;
actions!(glance, [Quit]);
mod editor;
use editor::Editor;
pub(crate) use editor::{Layout, Message};
use gpui::*;
fn main() {
    if std::env::args().any(|arg| arg == "--mcp") {
        if let Err(error) = mcp::run() {
            eprintln!("Glance MCP: {error}");
            std::process::exit(1);
        }
        return;
    }
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
                    title: Some("Glance".into()),
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
