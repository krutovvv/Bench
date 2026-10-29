//! A new thread before its first message: Bench's mark, where the thread will work, and what can
//! be done first — an insert put in with one click, a saved prompt — as Claude's empty session
//! offers its first steps. With no model set, the one thing that matters instead: nothing can
//! answer yet, and the gear (Settings → Agents) is where a model is added.

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::thread::ThreadView;
use bench_ui::Ico;

/// How far down the thread's area the welcome starts, as a share of the window's height: over
/// the middle, as an empty window's message sits on a Mac.
const WELCOME_TOP: f32 = 0.22;
/// Saved prompts offered as chips; the rest are a `/` away.
const PROMPTS_SHOWN: usize = 3;

impl ThreadView {
    pub(crate) fn render_welcome(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let at = self.thread_ref();
        let Some(app) = self.app(cx) else {
            return div().into_any_element();
        };
        let name = app.project_name(&self.thread.project);
        // Read once the agent has said: before that, a warning would only flash.
        let no_model = app.defaults.ready && app.defaults.model.is_none();
        let browser = app.browser_offered(&at);
        let prompts: Vec<(String, String)> = app
            .prompts
            .iter()
            .take(PROMPTS_SHOWN)
            .map(|p| (p.title.clone(), p.body.clone()))
            .collect();

        let head = v_flex()
            .items_center()
            .gap_2()
            .child(bench_ui::logo_mark(bench_ui::WELCOME_MARK, cx))
            .child(
                div()
                    .pt(px(bench_ui::WELCOME_GAP))
                    .text_size(px(bench_ui::WELCOME_TITLE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.foreground)
                    .child(if no_model { "Welcome to Bench".to_string() } else { name.to_string() }),
            );

        let below = if no_model {
            v_flex()
                .items_center()
                .gap_3()
                .max_w(px(bench_ui::WELCOME_NOTE_W))
                .text_center()
                .text_size(px(bench_ui::THREAD_META))
                .text_color(theme.muted_foreground)
                .child(
                    "No model is connected yet, so nothing can answer here. Bench works with a \
                     model you choose: a local server (Ollama, LM Studio, llama.cpp) or a \
                     provider's API key.",
                )
                .child(
                    bench_ui::primary_button("welcome-model", "Connect a Model…").on_click(
                        cx.listener(|this, _, window, cx| {
                            this.app
                                .update(cx, |app, cx| {
                                    app.gear.tab = 0;
                                    app.open_center(crate::workspace::Center::Settings, window, cx);
                                })
                                .ok();
                        }),
                    ),
                )
                .into_any_element()
        } else {
            let chips = h_flex()
                .flex_wrap()
                .justify_center()
                .gap_2()
                .child(
                    bench_ui::suggestion_chip(
                        "welcome-terminal",
                        Ico::Terminal,
                        "Terminal",
                        Some("/terminal".into()),
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.put_insert("ssh:local".into(), window, cx)
                    })),
                )
                .when(browser, |d| {
                    d.child(
                        bench_ui::suggestion_chip(
                            "welcome-browser",
                            Ico::Globe,
                            "Browser",
                            Some("/browser".into()),
                            cx,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.put_insert("browser".into(), window, cx)
                        })),
                    )
                })
                .child(
                    bench_ui::suggestion_chip(
                        "welcome-tools",
                        Ico::Wrench,
                        "Utilities",
                        Some("/tools".into()),
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.compose("/tools ".into(), window, cx)
                    })),
                )
                .children(prompts.into_iter().enumerate().map(|(i, (title, body))| {
                    bench_ui::suggestion_chip(("welcome-prompt", i), Ico::Sparkles, title, None, cx)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.compose(body.clone(), window, cx)
                        }))
                }));
            v_flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .text_size(px(bench_ui::THREAD_META))
                        .text_color(theme.muted_foreground)
                        .child("Write below, or start with a tool."),
                )
                .child(chips)
                .into_any_element()
        };

        v_flex()
            .w_full()
            .items_center()
            .px(px(bench_ui::CHAT_LEFT))
            .pt(window.viewport_size().height * WELCOME_TOP)
            .gap(px(bench_ui::WELCOME_GAP))
            .child(head)
            .child(below)
            .into_any_element()
    }

    /// The composer filled with `text`, the caret in it: the person goes on from there.
    fn compose(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        self.composer.update(cx, |s, cx| {
            s.set_value(text, window, cx);
            s.focus(window, cx);
        });
        cx.notify();
    }
}
