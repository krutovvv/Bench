//! The person's rows and the agent's: who speaks is an avatar in the left gutter.

use ::thread::Row;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
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

impl BenchApp {
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
    /// (queued); `actions` sit at its right; `field` replaces the text while it is being fixed.
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
        let said = (field.is_none() && !text.is_empty()).then(|| self.kept_plain(&key, text, cx));
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
        speaker_row(Some(false), body, cx)
            .children(actions)
            .into_any_element()
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
        // put in since (a terminal, a tool) and what waits in the queue leave it the last.
        let last = !self.thread.running()
            && self.tv.rows[k + 1..]
                .iter()
                .all(|r| matches!(r, Row::Queued { .. } | Row::Insert { .. }));
        let sources = self.render_sources(k, cx);
        let files = last.then(|| self.render_session_files(cx)).flatten();
        let group: SharedString = format!("answer-{key}").into();
        let (playing, speaking) = (
            self.playing.as_ref().is_some_and(|(j, _)| j == key),
            self.speaking.as_deref() == Some(key),
        );
        let (copy_key, speak_key) = (key.to_string(), key.to_string());
        let actions = h_flex()
            .absolute()
            .top(px(TURN_Y + 3.))
            .right_6()
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
                .on_click(
                    cx.listener(move |this, _, window, cx| {
                        this.toggle_speak(&speak_key, window, cx)
                    }),
                ),
            )
            .when(last, |d| {
                d.child(
                    bench_ui::icon_button(("regen", k), Ico::RotateCcw, "Regenerate", false, cx)
                        .on_click(cx.listener(|this, _, _, cx| this.regenerate(cx))),
                )
            });
        let body = v_flex()
            .gap_3()
            .children(self.render_answer(key, text, cx))
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
            .when(!text.trim().is_empty(), |d| d.child(actions))
            .into_any_element()
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
            .children(self.render_answer("writing", text, cx));
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
