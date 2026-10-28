//! Bench — one window for its own agent (bench-agent, `agent/`), its sessions, the user's servers
//! (terminal over SSH), their files (a mini IDE), everyday tools and tasks. Crate, binary and
//! bundle id (local.bench.app) are "bench"; its state lives in `~/.bench/ui` (copied over from
//! `~/Library/Application Support/bench` or `…/hermes-gpui` on the first start,
//! `client::connection::migrate_state`).

// The browser build (web/) leaves out what needs a real system (terminals, probes, the state
// folder); what only the desktop calls is unused there.
#![cfg_attr(target_family = "wasm", allow(dead_code, unused_imports))]

mod app;
mod client;
mod demo;
mod env;
mod explorer;
mod inserts;
mod media;
mod settings;
mod sidebar;
mod statusbar;
mod system;
mod thread;
mod titlebar;
mod workspace;

use gpui_kit::*;

use crate::client::connection;

#[cfg(not(target_family = "wasm"))]
fn main() {
    connection::migrate_state();
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--probe") {
        if let Err(e) = client::probe::run(args.get(2).map(String::as_str).unwrap_or("local")) {
            eprintln!("probe failed: {e:#}");
            std::process::exit(1);
        }
        return;
    }
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            bench_ui::theme::apply(&settings::appearance::load(), None, cx);
            app::menu::install(cx);

            open_main_window(cx);
            cx.activate(true);
        });
}

/// The browser build (web/): always the demo — nothing to connect to there. gpui-kit's own
/// icons come over HTTP from `./assets/icons/` next to the page; the browser has no system
/// fonts, so Inter, Fira Mono and Noto Sans (OFL, assets/fonts/) are built in.
#[cfg(target_family = "wasm")]
fn main() {
    console_error_panic_hook::set_once();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets::new("."))
        .run(|cx| {
            gpui_kit::init(cx);
            let fonts = vec![
                std::borrow::Cow::Borrowed(
                    include_bytes!("../assets/fonts/InterVariable.ttf").as_slice(),
                ),
                std::borrow::Cow::Borrowed(
                    include_bytes!("../assets/fonts/FiraMono-Medium.ttf").as_slice(),
                ),
                // GPUI's default text style asks for the system font; Noto Sans is on its
                // fallback list, so whatever isn't styled by the theme still has a font.
                std::borrow::Cow::Borrowed(
                    include_bytes!("../assets/fonts/NotoSans-Regular.ttf").as_slice(),
                ),
            ];
            if let Err(e) = cx.text_system().add_fonts(fonts) {
                log_error(&format!("fonts: {e:#}"));
            }
            bench_ui::theme::apply(&settings::appearance::load(), None, cx);
            open_main_window(cx);
        });
}

#[cfg(target_family = "wasm")]
fn log_error(text: &str) {
    eprintln!("{text}");
}

/// A Bench window: its own connection to the agent, profile and chat.
pub fn open_main_window(cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1240.), px(800.)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some("Bench".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(14.), px(14.))),
        }),
        ..Default::default()
    };
    if let Err(e) = gpui_kit::open_window(options, cx, |window, cx| {
        let app = cx.new(|cx| app::BenchApp::new(window, cx));
        // The `/` list answers ↑ ↓ Tab Enter. It has to be an interceptor: gpui matches key
        // bindings BEFORE any key listener (Window::dispatch_key_event runs the bindings, and
        // only after that the capture and bubble key listeners), and the composer is a text
        // input that binds ↑ ↓ to moving the caret — so a handler on the root or on the
        // composer's own element sees those arrows only after the caret already moved.
        let watch = app.downgrade();
        let keystrokes = cx.intercept_keystrokes(move |ev, window, cx| {
            let ks = &ev.keystroke;
            if ks.modifiers.modified() {
                return;
            }
            let used = match ks.key.as_str() {
                // The thread whose composer has the focus (the open one, or the one beside).
                "up" | "down" | "tab" | "enter" => watch
                    .upgrade()
                    .and_then(|app| {
                        app.read(cx).thread_views().into_iter().find(|v| {
                            v.read(cx)
                                .composer
                                .read(cx)
                                .focus_handle(cx)
                                .is_focused(window)
                        })
                    })
                    .map(|view| view.update(cx, |v, cx| v.slash_key(ks.key.as_str(), window, cx)))
                    .unwrap_or(false),
                _ => false,
            };
            if used {
                cx.stop_propagation();
            }
        });
        // The interceptor lives as long as the app.
        app.update(cx, |this, _| this._slash_keys = Some(keystrokes));
        app
    }) {
        eprintln!("open window: {e:#}");
    }
}
