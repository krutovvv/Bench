//! What `BenchApp` listens to and keeps running while it lives, set up in `BenchApp::new`: its
//! fields' events (the composer, the question card's field, the search, the pickers) and its
//! timers (the agent's events, the clock, the background processes, the agent's state).

use std::time::Duration;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::SelectEvent;
use gpui_kit::*;

use super::{BenchApp, Choice};

impl BenchApp {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn subscriptions(
        search: &Entity<InputState>,
        project_path: &Entity<InputState>,
        effort_choice: &Entity<Choice>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Subscription> {
        vec![
            // "System" follows macOS light/dark as it changes.
            cx.observe_window_appearance(window, |this, window, cx| {
                if this.appearance.mode == bench_ui::theme::Mode::System {
                    bench_ui::theme::apply(&this.appearance, Some(window), cx);
                    this.repaint_terminals(cx);
                }
            }),
            cx.subscribe_in(
                search,
                window,
                |this, state, ev: &InputEvent, window, cx| {
                    if matches!(ev, InputEvent::Change) {
                        let q = state.read(cx).value().trim().to_string();
                        this.search_threads(q, window, cx);
                    }
                },
            ),
            cx.subscribe_in(
                project_path,
                window,
                |this, state, ev: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter { .. } = ev {
                        let path = state.read(cx).value().trim().to_string();
                        if !path.is_empty() {
                            this.path_entry = false;
                            this.set_new_project(path, window, cx);
                        }
                    }
                },
            ),
            cx.subscribe_in(
                effort_choice,
                window,
                |this, _, ev: &SelectEvent<Vec<SharedString>>, _, cx| {
                    let SelectEvent::Confirm(Some(label)) = ev else {
                        return;
                    };
                    if let Some(i) = this.efforts.iter().position(|e| e == label.as_ref()) {
                        this.set_effort(i, cx);
                    }
                },
            ),
        ]
    }

    pub(super) fn timers(_window: &mut Window, cx: &mut Context<Self>) -> Vec<Task<()>> {
        // Live clock + playback end detection.
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                let alive = this.update(cx, |this, cx| {
                    if this.playing.as_ref().is_some_and(|(_, p)| p.is_done()) {
                        this.playing = None;
                        cx.notify();
                    }
                    if this
                        .media_playing
                        .as_ref()
                        .is_some_and(|(_, p)| p.is_done())
                    {
                        this.media_playing = None;
                    }
                    if this.media_playing.is_some() {
                        cx.notify(); // the elapsed time
                    }
                    let live = this.thread(cx).live.is_some();
                    // Each thread's speed meter counts its own seconds.
                    for view in this.thread_views() {
                        view.update(cx, |v, _| {
                            if v.thread.live.is_some() {
                                v.thread.speed.tick();
                            }
                        });
                    }
                    if live || this.recorder.is_some() {
                        cx.notify();
                    }
                    this.tick_tasks(cx);
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        vec![ticker]
    }
}
