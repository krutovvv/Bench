//! What `BenchApp` listens to and keeps running while it lives, set up in `BenchApp::new`: its
//! fields' events (the composer, the question card's field, the search, the pickers) and its
//! timers (the agent's events, the clock, the background processes, the agent's state).

use std::time::Duration;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::SelectEvent;
use gpui_kit::*;

use super::{BenchApp, Choice};
use crate::client::connection;
use crate::client::rest;
use crate::client::rpc::Evt;

impl BenchApp {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn subscriptions(
        search: &Entity<InputState>,
        project_path: &Entity<InputState>,
        effort_choice: &Entity<Choice>,
        target_choice: &Entity<Choice>,
        profile_choice: &Entity<Choice>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Subscription> {
        vec![
            // "System" follows macOS light/dark as it changes.
            cx.observe_window_appearance(window, |this, window, cx| {
                if this.appearance.mode == bench_ui::theme::Mode::System {
                    bench_ui::theme::apply(this.appearance, Some(window), cx);
                    this.repaint_terminals(cx);
                }
            }),
            cx.subscribe_in(
                search,
                window,
                |this, state, ev: &InputEvent, window, cx| {
                    if matches!(ev, InputEvent::Change) {
                        let q = state.read(cx).value().trim().to_string();
                        this.search_sessions(q, window, cx);
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
            cx.subscribe_in(
                target_choice,
                window,
                |this, _, ev: &SelectEvent<Vec<SharedString>>, window, cx| {
                    let SelectEvent::Confirm(Some(label)) = ev else {
                        return;
                    };
                    if let Some(t) = this
                        .targets
                        .iter()
                        .find(|t| t.label() == label.as_ref())
                        .cloned()
                    {
                        if t != this.target || this.local_conn().is_none() {
                            this.connect(t, window, cx);
                        }
                    }
                },
            ),
            cx.subscribe_in(
                profile_choice,
                window,
                |this, _, ev: &SelectEvent<Vec<SharedString>>, window, cx| {
                    let SelectEvent::Confirm(Some(label)) = ev else {
                        return;
                    };
                    if let Some(p) = this.profiles.iter().find(|p| p.label == label.as_ref()) {
                        let name = p.name.clone();
                        this.select_profile(name, window, cx);
                    }
                },
            ),
        ]
    }

    pub(super) fn timers(
        evt_rx: async_channel::Receiver<Evt>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Task<()>> {
        let events = cx.spawn_in(window, async move |this, cx| {
            while let Ok(evt) = evt_rx.recv().await {
                if this
                    .update_in(cx, |this, window, cx| this.on_event(evt, window, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
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
                    if live {
                        this.speed.tick();
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
        // The open session's background processes, every 3 s (the gateway has no event for them).
        let processes = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(3)).await;
                if this.update(cx, |this, cx| this.poll_processes(cx)).is_err() {
                    break;
                }
            }
        });
        // The gateway item in the status bar: its state and the last lines of its log, every 10 s
        // while connected. Both come from where this project's agent runs, so a project on a
        // server shows that server's gateway.
        let backend = cx.spawn(async move |this, cx| {
            loop {
                let Ok(job) = this.update(cx, |this, cx| {
                    let at = this.thread_ref(cx);
                    this.thread_conn(&at)
                        .map(|c| (c, this.current_profile(), this.thread_target(&at)))
                }) else {
                    break;
                };
                if let Some((conn, profile, target)) = job {
                    let r = cx
                        .background_spawn(async move {
                            (
                                rest::gateway_status(&conn, &profile),
                                connection::gateway_log(&target, 12),
                            )
                        })
                        .await;
                    if this
                        .update(cx, |this, cx| {
                            this.gw_status = Some(r.0.map_err(|e| format!("{e:#}")));
                            this.gw_log = r.1;
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                cx.background_executor()
                    .timer(Duration::from_secs(10))
                    .await;
            }
        });
        vec![events, ticker, processes, backend]
    }
}
