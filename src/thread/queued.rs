//! A message waiting for the agent: the person's own row, lighter, with its controls — fix the
//! words, send now (into the running work), take it back — and what those controls do.

use gpui_kit::component::input::Textarea;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use bench_ui::icons::Ico;
use gpui_kit::component::input::{InputEvent, TextareaState};
use crate::client::rpc::Cmd;

impl BenchApp {
    pub(crate) fn render_queued(
        &mut self,
        key: &str,
        sub: &str,
        mode: &str,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let steer = mode == "steer";
        let (said, paths) = crate::media::attach::split(text);
        let field = self
            .queue_edit
            .as_ref()
            .filter(|(e, _)| e == sub)
            .map(|(_, st)| st.clone());
        let (send_id, rm_id, edit_id, edit_text) = (
            sub.to_string(),
            sub.to_string(),
            sub.to_string(),
            text.to_string(),
        );
        let id = SharedString::from(key.to_string());
        let actions = h_flex()
            .flex_none()
            .gap_1()
            .items_center()
            .text_sm()
            .text_color(theme.muted_foreground)
            .child(if field.is_some() {
                "Editing"
            } else if steer {
                "Next"
            } else {
                "Queued"
            })
            .when(field.is_some(), |d| {
                d.child(
                    bench_ui::icon_button(
                        (id.clone(), 0),
                        Ico::Check,
                        "Keep these words (⌘Enter)",
                        false,
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.click_claimed = true;
                        this.queue_edit_save(window, cx);
                    })),
                )
            })
            .when(field.is_none(), |d| {
                d.child(
                    bench_ui::icon_button(
                        (id.clone(), 1),
                        Ico::Pencil,
                        "Fix the words before it goes out",
                        false,
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        this.queue_edit_open(edit_id.clone(), edit_text.clone(), window, cx);
                    })),
                )
                .when(!steer, |d| {
                    d.child(
                        bench_ui::icon_button(
                            (id.clone(), 2),
                            Ico::Play,
                            "Send now: into the running work",
                            false,
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.click_claimed = true;
                            this.queued(send_id.clone(), true, cx);
                        })),
                    )
                })
            })
            .child(
                bench_ui::icon_button((id.clone(), 3), Ico::X, "Take it back", false, cx).on_click(
                    cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        if this.queue_edit.as_ref().is_some_and(|(e, _)| e == &rm_id) {
                            this.queue_edit_cancel(window, cx);
                        } else {
                            this.queued(rm_id.clone(), false, cx);
                        }
                    }),
                ),
            )
            .into_any_element();
        let editor = field.map(|st| {
            // The composer's field, at its size and leading: the row is as tall as the text.
            div()
                .id((id.clone(), 4))
                .ml(px(-12.))
                .my(px(-10.))
                .cursor_text()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseDownEvent, _, _| this.click_claimed = true),
                )
                .child(
                    Textarea::new(&st)
                        .appearance(false)
                        .large()
                        .line_height(rems(bench_ui::LEAD)),
                )
                .into_any_element()
        });
        div()
            .opacity(0.85)
            .child(self.user_row(
                key.to_string(),
                &said,
                &paths,
                Some(actions),
                true,
                editor,
                cx,
            ))
            .into_any_element()
    }
}

impl BenchApp {
    /// A waiting message (the thread's queue): sent now, into the running work, or taken back.
    pub fn queued(&mut self, id: String, send_now: bool, cx: &mut Context<Self>) {
        let Some(session) = self.runtime.clone() else {
            return;
        };
        self.cmd(if send_now {
            Cmd::QueueSend {
                session: session.clone(),
                id: id.clone(),
            }
        } else {
            Cmd::QueueWithdraw {
                session,
                id: id.clone(),
            }
        });
        cx.notify();
    }

    /// Fix the words of a waiting message. It has not gone anywhere, so a wrong word is cheap to
    /// change; one the agent has already taken up cannot (Pi settles it) — that row is gone by then.
    pub fn queue_edit_open(
        &mut self,
        id: String,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let field = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 100_000)
                .submit_on_enter(false)
        });
        field.update(cx, |s, cx| s.set_value(text, window, cx));
        cx.subscribe_in(&field, window, |this, _, ev: &InputEvent, window, cx| {
            // Enter is a new line, as in the composer; ⌘Enter keeps the change.
            if matches!(
                ev,
                InputEvent::PressEnter {
                    secondary: true,
                    ..
                }
            ) {
                this.queue_edit_save(window, cx);
            }
        })
        .detach();
        self.queue_edit = Some((id, field.clone()));
        // The click that opened this field is its own: the thread's bare-click rule (the composer
        // takes the focus) must not undo the focus this field just took.
        self.click_claimed = true;
        field.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    pub fn queue_edit_save(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some((id, field)) = self.queue_edit.take() else {
            return;
        };
        let text = field.read(cx).value().trim().to_string();
        if let Some(session) = self.runtime.clone() {
            if !text.is_empty() {
                self.cmd(Cmd::QueueEdit { session, id, text });
            }
        }
        cx.notify();
    }

    pub fn queue_edit_cancel(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.queue_edit = None;
        cx.notify();
    }
}
