//! A probe for the file drop: `BENCH_DEMO_DROP=<where>` makes Bench itself carry a file into the
//! window (the same `FileDropEvent`s macOS sends for a drag from the desktop) and drop it, so a
//! screenshot shows whether the thread under it took it as an attachment. `where`: `chat` — the
//! middle, `right` — the right dock's width at the middle's height, `between` — between the docks,
//! `foot` — the bottom edge, over the composer's meters, `side` — the sidebar. `tint` — only
//! carries the file in and holds it there (no drop): the picture shows the window's accept-as-it-is
//! wash.

use std::path::PathBuf;

use gpui_kit::*;

use crate::app::BenchApp;

/// The file the simulated drop carries (it must be a real file: an attachment is one).
const PROBE_FILE: &str = "/tmp/bench-drop-probe.txt";

impl BenchApp {
    /// `BENCH_DEMO_CLICK=<x>,<y>`: the left button pressed and let go there (points), two seconds
    /// in, as macOS sends it: a picture shows who took the click and who has the focus.
    pub(crate) fn click_probe(&self, window: &mut Window, cx: &mut Context<Self>) {
        static DONE: std::sync::Once = std::sync::Once::new();
        let Some((x, y)) = std::env::var("BENCH_DEMO_CLICK").ok().and_then(|v| {
            let (x, y) = v.split_once(',')?;
            Some((x.parse::<f32>().ok()?, y.parse::<f32>().ok()?))
        }) else {
            return;
        };
        DONE.call_once(|| {
            let handle = window.window_handle();
            let wait = cx.background_executor().clone();
            cx.spawn(async move |_, cx| {
                wait.timer(std::time::Duration::from_secs(2)).await;
                let position = point(px(x), px(y));
                let press = MouseDownEvent {
                    button: MouseButton::Left,
                    position,
                    modifiers: Modifiers::default(),
                    click_count: 1,
                    first_mouse: false,
                };
                let release = MouseUpEvent {
                    button: MouseButton::Left,
                    position,
                    modifiers: Modifiers::default(),
                    click_count: 1,
                };
                for event in [PlatformInput::MouseDown(press), PlatformInput::MouseUp(release)] {
                    let _ = handle.update(cx, |_, window, cx| window.dispatch_event(event, cx));
                    wait.timer(std::time::Duration::from_millis(60)).await;
                }
            })
            .detach();
        });
    }

    /// The simulated drop, once, about a second after the window stood up.
    pub(crate) fn drop_probe(&self, window: &mut Window, cx: &mut Context<Self>) {
        static DONE: std::sync::Once = std::sync::Once::new();
        let where_ = match std::env::var("BENCH_DEMO_DROP") {
            Ok(w) => w,
            Err(_) => return,
        };
        DONE.call_once(|| {
            let _ = std::fs::write(PROBE_FILE, "dropped by the probe\n");
            let handle = window.window_handle();
            let wait = cx.background_executor().clone();
            cx.spawn(async move |_, cx| {
                wait.timer(std::time::Duration::from_millis(900)).await;
                let _ = handle.update(cx, |_, window, cx| {
                    let size = window.viewport_size();
                    let at = match where_.as_str() {
                        "chat" => point(size.width * 0.5, size.height * 0.5),
                        "right" => point(size.width * 0.88, size.height * 0.5),
                        "foot" => point(size.width * 0.5, size.height - px(12.)),
                        "side" => point(size.width * 0.10, size.height * 0.5),
                        _ => point(size.width * 0.35, size.height * 0.5),
                    };
                    let paths = ExternalPaths([PathBuf::from(PROBE_FILE)].into_iter().collect());
                    let mut events = vec![
                        FileDropEvent::Entered {
                            position: at,
                            paths,
                        },
                        FileDropEvent::Pending { position: at },
                    ];
                    if where_ != "tint" {
                        events.push(FileDropEvent::Submit { position: at });
                    }
                    for event in events {
                        window.dispatch_event(event.to_platform_input(), cx);
                    }
                });
            })
            .detach();
        });
    }
}
