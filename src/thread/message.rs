//! The person's rows and the agent's: who speaks is an avatar in the left gutter.

use ::thread::Row;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::ThreadView;
use bench_ui::icons::Ico;
use bench_ui::{AVATAR, CHAT_LEFT, CHAT_RIGHT, CHAT_TEXT, TURN_Y, avatar};

/// The row frame: the avatar (or its empty place) in the gutter, `body` at the text column.
pub(crate) fn speaker_row(agent: Option<bool>, body: impl IntoElement, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .w_full()
        .pl(px(CHAT_LEFT))
        .pr(px(CHAT_RIGHT))
        .py(px(TURN_Y))
        .gap(px(CHAT_TEXT - CHAT_LEFT - AVATAR))
        .items_start()
        .text_base()
        .child(match agent {
            Some(agent) => avatar(agent, theme).into_any_element(),
            None => div().flex_none().w(px(AVATAR)).into_any_element(),
        })
        .child(div().flex_1().min_w_0().child(body))
}

impl ThreadView {
    pub(crate) fn render_user(
        &mut self,
        key: &str,
        text: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (said, paths) = crate::media::attach::split(text);
        self.user_row(key.to_string(), &said, &paths, None, false, None, cx)
    }

    /// The person's own row: their text, the files that went with it. `muted`: not gone out yet
    /// (queued); `actions` go under the text, at its indent (the thread's width is precious in the
    /// Explorer, so nothing takes it from the side); `field` replaces the text while it is fixed.
    pub(crate) fn user_row(
        &mut self,
        key: String,
        text: &str,
        paths: &[String],
        actions: Option<AnyElement>,
        muted: bool,
        field: Option<AnyElement>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let attached: Vec<AnyElement> = paths
            .iter()
            .enumerate()
            .map(|(k, p)| self.attachment_chip(format!("{key}-{k}").into(), p, None, cx))
            .collect();
        let said =
            (field.is_none() && !text.is_empty()).then(|| self.thread.texts.plain(&key, text, cx));
        let body = div()
            .whitespace_normal()
            .when(muted, |d| d.text_color(theme.muted_foreground))
            .children(said)
            .when_some(field, |d, f| d.child(f))
            .when(!attached.is_empty(), |d| {
                d.child(
                    h_flex()
                        .flex_wrap()
                        .gap_2()
                        .when(!text.is_empty(), |d| d.mt(px(10.)))
                        .pb(px(4.))
                        .children(attached),
                )
            });
        let body = body.when_some(actions, |d, a| d.child(div().pt_2().child(a)));
        speaker_row(Some(false), body, cx).into_any_element()
    }

    /// What the agent said. `k` is the row's place (the last answer gets Regenerate);
    /// `after_work`: it continues the work row above it, so no avatar of its own.
    pub(crate) fn render_agent_answer(
        &mut self,
        k: usize,
        key: &str,
        text: &str,
        error: Option<&str>,
        after_work: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        // The last answer: nothing the person or the agent said or did after it. What the person
        // put in since (a terminal, a tool), the plan of the work in progress and what waits in
        // the queue leave it the last.
        let last = !self.thread.journal.running()
            && self.thread.tv.rows[k + 1..].iter().all(|r| {
                matches!(
                    r,
                    Row::Queued { .. } | Row::Insert { .. }
                )
            });
        let sources = self.render_sources(k, cx);
        let files = last.then(|| self.render_session_files(cx)).flatten();
        let group: SharedString = format!("answer-{key}").into();
        let (playing, speaking) = self.app(cx).map_or((false, false), |a| {
            (
                a.playing.as_ref().is_some_and(|(j, _)| j == key),
                a.speaking.as_deref() == Some(key),
            )
        });
        let (copy_key, speak_key) = (key.to_string(), key.to_string());
        // Hovering anywhere over the answer reveals its two actions, at the same edge the
        // live run's Pause and Stop sat at: read aloud, and copy — which copies the agent's
        // own markdown, what selecting the text on screen cannot. No regenerate: an agent
        // run is not a chat reply to roll again; it acted, checked itself, and this is it.
        let actions = h_flex()
            .absolute()
            .top(px(TURN_Y + 3.))
            .right_0()
            .pl_1()
            .gap_0p5()
            .items_center()
            .bg(theme.background)
            .when(!(playing || speaking), |d| {
                d.opacity(0.).group_hover(group.clone(), |s| s.opacity(1.))
            })
            .child(
                bench_ui::icon_button(("copy", k), Ico::Copy, "Copy", false, cx).on_click(
                    cx.listener(move |this, _, window, cx| this.copy_answer(&copy_key, window, cx)),
                ),
            )
            .child(
                bench_ui::icon_button(
                    ("speak", k),
                    if playing {
                        Ico::Square
                    } else if speaking {
                        Ico::Loader
                    } else {
                        Ico::Volume
                    },
                    if playing { "Stop" } else { "Read aloud" },
                    false,
                    cx,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    let text = this.answer_text(&speak_key).unwrap_or_default();
                    let key = speak_key.clone();
                    this.app
                        .update(cx, |app, cx| app.toggle_speak(&key, text, window, cx))
                        .ok();
                })),
            );
        let body = v_flex()
            .gap_3()
            .children(self.answer_body(key, text, cx))
            .when_some(error, |d, e| {
                d.child(
                    div()
                        .text_color(theme.danger)
                        .child(format!("The model stopped: {e}")),
                )
            })
            .children(sources)
            .children(files);
        div()
            .id(SharedString::from(format!("row-{key}")))
            .group(group)
            .relative()
            .w_full()
            .child(
                speaker_row(if after_work { None } else { Some(true) }, body, cx)
                    .when(after_work, |d| d.pt_0()),
            )
            // A pure chat answer carries its two where they are; an answer after work wears
            // them on the work line itself, in the transport's seat (`work.rs`).
            .when(!after_work && !text.trim().is_empty(), |d| d.child(actions))
            .into_any_element()
    }

    /// An answer's text, delivered files, diagrams and code (`media/view.rs`, the app's: its
    /// files and playback are the window's), parsed into this thread's texts.
    fn answer_body(&mut self, key: &str, text: &str, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let at = self.thread_ref();
        let Some(app) = self.app.upgrade() else {
            return vec![];
        };
        let texts = &mut self.thread.texts;
        app.update(cx, |app, cx| app.render_answer(&at, texts, key, text, cx))
    }

    /// The answer being written: its reasoning so far, muted, then its text.
    pub(crate) fn render_writing(
        &mut self,
        text: &str,
        after_work: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let body = v_flex()
            .gap_3()
            .children(self.answer_body("writing", text, cx));
        speaker_row(if after_work { None } else { Some(true) }, body, cx)
            .when(after_work, |d| d.pt_0())
            .into_any_element()
    }

    /// Why a run stopped, kept in the journal.
    pub(crate) fn render_fail(&mut self, text: &str, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        speaker_row(
            Some(true),
            div()
                .text_color(theme.danger)
                .child(format!("The run stopped: {text}")),
            cx,
        )
        .into_any_element()
    }
}
