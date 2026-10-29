//! A message waiting for the agent: the person's own row, lighter, with its controls — fix the
//! words, send now (into the running work), take it back — and what those controls do.

use gpui_kit::component::input::Textarea;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::thread::ThreadView;
use bench_ui::icons::Ico;
use gpui_kit::component::input::{InputEvent, TextareaState};

impl ThreadView {
    pub(crate) fn render_queued(
        &mut self,
        key: &str,
        sub: &str,
        mode: protocol::QueueMode,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let steer = mode == protocol::QueueMode::Steer;
        // Words still on their way to the queue (no `sub` yet): the row shows, controls wait
        // for the id the agent will give them.
        let echo = sub.is_empty();
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
            .text_size(px(bench_ui::THREAD_META))
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
                        "Keep these words (Enter)",
                        false,
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.claim_click(cx);
                        this.queue_edit_save(window, cx);
                    })),
                )
            })
            .when(!echo && field.is_none(), |d| {
                d.child(
                    bench_ui::icon_button(
                        (id.clone(), 1),
                        Ico::Pencil,
                        "Fix the words before it goes out",
                        false,
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.claim_click(cx);
                        this.queue_edit_open(edit_id.clone(), edit_text.clone(), window, cx);
                    })),
                )
                .when(!echo && !steer, |d| {
                    d.child(
                        bench_ui::icon_button(
                            (id.clone(), 2),
                            Ico::Play,
                            "Send now: into the running work",
                            false,
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.claim_click(cx);
                            this.queued(send_id.clone(), true, cx);
                        })),
                    )
                })
            })
            .when(!echo, |d| {
                d.child(
                    bench_ui::icon_button((id.clone(), 3), Ico::X, "Take it back", false, cx)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.claim_click(cx);
                            if this.queue_edit.as_ref().is_some_and(|(e, _)| e == &rm_id) {
                                this.queue_edit_cancel(window, cx);
                            } else {
                                this.queued(rm_id.clone(), false, cx);
                            }
                        })),
                )
            })
            .into_any_element();
        let editor = field.map(|st| {
            // The composer's field, at its size and leading: the row is as tall as the text.
            bench_ui::flush_field()
                .id((id.clone(), 4))
                .cursor_text()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseDownEvent, _, cx| this.claim_click(cx)),
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

impl ThreadView {
    /// A waiting message (the thread's queue): sent now, into the running work, or taken back.
    pub fn queued(&mut self, id: String, send_now: bool, cx: &mut Context<Self>) {
        let Some(thread) = self.thread.id.clone() else {
            return;
        };
        if send_now {
            self.tell(protocol::SendQueuedNow { thread, sub: id }, cx);
        } else {
            self.tell(protocol::WithdrawQueued { thread, sub: id }, cx);
        }
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
                // As in the composer: Enter keeps the words, Shift+Enter (or ⌘Enter) is a new
                // line.
                .submit_on_enter(true)
        });
        field.update(cx, |s, cx| s.set_value(text, window, cx));
        cx.subscribe_in(&field, window, |this, _, ev: &InputEvent, window, cx| {
            // Enter keeps the change, as it sends in the composer; Shift+Enter and ⌘Enter are a
            // new line.
            if matches!(
                ev,
                InputEvent::PressEnter {
                    secondary: false,
                    shift: false,
                }
            ) {
                this.queue_edit_save(window, cx);
            }
        })
        .detach();
        self.queue_edit = Some((id, field.clone()));
        // The click that opened this field is its own: the thread's bare-click rule (the composer
        // takes the focus) must not undo the focus this field just took.
        self.claim_click(cx);
        field.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    pub fn queue_edit_save(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some((id, field)) = self.queue_edit.take() else {
            return;
        };
        let text = field.read(cx).value().trim().to_string();
        if let Some(thread) = self.thread.id.clone()
            && !text.is_empty()
        {
            self.tell(protocol::EditQueued { thread, sub: id, text }, cx);
        }
        cx.notify();
    }

    pub fn queue_edit_cancel(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.queue_edit = None;
        cx.notify();
    }
}
