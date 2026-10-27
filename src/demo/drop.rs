//! A probe for the file drop: `BENCH_DEMO_DROP=<where>` makes Bench itself carry a file into the
//! window (the same `FileDropEvent`s macOS sends for a drag from the desktop) and drop it, so a
//! screenshot shows whether the thread under it took it as an attachment. `where`: `chat` — the
//! middle, `right` — the right dock's width at the middle's height, `between` — between the docks.

use std::path::PathBuf;

use gpui_kit::*;

use crate::app::BenchApp;

/// The file the simulated drop carries (it must be a real file: an attachment is one).
const PROBE_FILE: &str = "/tmp/bench-drop-probe.txt";

impl BenchApp {
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
                        _ => point(size.width * 0.35, size.height * 0.5),
                    };
                    let paths = ExternalPaths([PathBuf::from(PROBE_FILE)].into_iter().collect());
                    for event in [
                        FileDropEvent::Entered {
                            position: at,
                            paths,
                        },
                        FileDropEvent::Pending { position: at },
                        FileDropEvent::Submit { position: at },
                    ] {
                        window.dispatch_event(event.to_platform_input(), cx);
                    }
                });
            })
            .detach();
        });
    }
}