mod app;
mod icons;
mod lang;
mod state;
mod theme;
mod ui;

use gpui_kit::*;

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            theme::apply(theme::Appearance::load(), None, cx);
            open_main_window(cx);
            cx.activate(true);
        });
}

/// A window is a whole app state: more windows rather than tabs.
pub fn open_main_window(cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1240.), px(800.)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some("Text View".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(14.), px(14.))),
        }),
        ..Default::default()
    };
    if let Err(e) = gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| app::MainApp::new(window, cx))
    }) {
        state::log_error("open window", &format!("{e:#}"));
    }
}
