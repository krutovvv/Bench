//! Real input for a picture: `BENCH_DEMO_DROP=<where>` carries a file into the window (the same
//! `FileDropEvent`s macOS sends for a drag from the desktop) and drops it, so a screenshot shows
//! whether the thread under it took it as an attachment. `where`: `chat` — the middle, `right` —
//! the right dock's width at the middle's height, `between` — between the docks, `foot` — the
//! bottom edge, over the composer's meters, `side` — the sidebar. `tint` — only carries the file in
//! and holds it there (no drop): the picture shows the window's accept-as-it-is wash.
//! `BENCH_DEMO_CLICK=<x>,<y>`: the left button pressed and let go there (points), two seconds in:
//! a picture shows who took the click and who has the focus.
//! `BENCH_DEMO_DRAG=<x>,<y>:<x>,<y>[:hold]`: the left button pressed at the first place, carried to
//! the second and let go there (`hold`: kept down): what a person's hand does, so a picture shows
//! what a drag really lands on.

use std::path::PathBuf;

use gpui_kit::*;

use crate::app::BenchApp;

/// The file the simulated drop carries (it must be a real file: an attachment is one).
const PROBE_FILE: &str = "/tmp/bench-drop-probe.txt";

pub(super) fn click(at: &str, window: &mut Window, cx: &mut Context<BenchApp>) {
    let Some((x, y)) = at.split_once(',').and_then(|(x, y)| Some((x.parse::<f32>().ok()?, y.parse::<f32>().ok()?))) else {
        return;
    };
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
}

/// `<x>,<y>` in points.
fn place(at: &str) -> Option<Point<Pixels>> {
    let (x, y) = at.split_once(',')?;
    Some(point(px(x.parse().ok()?), px(y.parse().ok()?)))
}

pub(super) fn drag(spec: &str, window: &mut Window, cx: &mut Context<BenchApp>) {
    let mut parts = spec.split(':');
    let (Some(from), Some(to)) = (parts.next().and_then(place), parts.next().and_then(place)) else {
        return;
    };
    let hold = parts.next() == Some("hold");
    let handle = window.window_handle();
    let wait = cx.background_executor().clone();
    cx.spawn(async move |_, cx| {
        wait.timer(std::time::Duration::from_secs(2)).await;
        let modifiers = Modifiers::default();
        let moved = |position| PlatformInput::MouseMove(MouseMoveEvent { position, pressed_button: Some(MouseButton::Left), modifiers });
        let press = MouseDownEvent { button: MouseButton::Left, position: from, modifiers, click_count: 1, first_mouse: false };
        let _ = handle.update(cx, |_, window, cx| window.dispatch_event(PlatformInput::MouseDown(press), cx));
        const STEPS: usize = 12;
        for step in 1..=STEPS {
            let done = step as f32 / STEPS as f32;
            let position = point(from.x + (to.x - from.x) * done, from.y + (to.y - from.y) * done);
            let _ = handle.update(cx, |_, window, cx| window.dispatch_event(moved(position), cx));
            wait.timer(std::time::Duration::from_millis(40)).await;
        }
        if hold {
            // Held there: a real pointer moving over the copy would otherwise say where it is.
            loop {
                wait.timer(std::time::Duration::from_millis(100)).await;
                if handle.update(cx, |_, window, cx| window.dispatch_event(moved(to), cx)).is_err() {
                    return;
                }
            }
        }
        let release = MouseUpEvent { button: MouseButton::Left, position: to, modifiers, click_count: 1 };
        let _ = handle.update(cx, |_, window, cx| window.dispatch_event(PlatformInput::MouseUp(release), cx));
    })
    .detach();
}

/// The simulated drop, about a second after the window stood up.
pub(super) fn drop_file(where_: String, window: &mut Window, cx: &mut Context<BenchApp>) {
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
        // Held there: a real pointer moving over the copy would otherwise end the carry.
        while where_ == "tint" {
            wait.timer(std::time::Duration::from_millis(100)).await;
            let at = handle.update(cx, |_, window, _| window.viewport_size());
            let Ok(size) = at else { return };
            let position = point(size.width * 0.35, size.height * 0.5);
            let paths = ExternalPaths([PathBuf::from(PROBE_FILE)].into_iter().collect());
            let _ = handle.update(cx, |_, window, cx| {
                for event in [FileDropEvent::Entered { position, paths }, FileDropEvent::Pending { position }] {
                    window.dispatch_event(event.to_platform_input(), cx);
                }
            });
        }
    })
    .detach();
}
