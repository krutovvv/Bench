//! A new thread before its first message: Bench's mark, where the thread will work, and what can
//! be done first — an insert put in with one click — as Claude's empty session
//! offers its first steps. With no model set, the one thing that matters instead: nothing can
//! answer yet, and the gear (Settings → Agents) is where a model is added.

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::*;
use bench_ui::Gap;

use crate::thread::ThreadView;

/// How far down the thread's area the welcome starts, as a share of the window's height: over
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

        let head = v_flex()
            .items_center()
            .gap(Gap::M)
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
                .gap(Gap::L)
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
            // Each kind the thread offers, with its chip (`InsertKind::welcome`).
            let kinds: Vec<_> = crate::inserts::kinds()
                .iter()
                .filter(|k| k.offered(app, &at))
                .filter_map(|k| Some((k.word(), k.welcome()?)))
                .collect();
            let chips = h_flex().flex_wrap().justify_center().gap(Gap::M).children(kinds.into_iter().map(
                |(word, chip)| {
                    let slash = format!("/{}", word.word);
                    let (key, typed) = (word.key.to_string(), format!("{slash} "));
                    bench_ui::suggestion_chip(
                        SharedString::from(format!("welcome-{}", word.word)),
                        chip.icon,
                        chip.label,
                        Some(slash.into()),
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if chip.compose {
                            this.compose(typed.clone(), window, cx)
                        } else {
                            this.put_insert(key.clone(), window, cx)
                        }
                    }))
                },
            ));
            v_flex()
                .items_center()
                .gap(Gap::L)
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
            .pt(window.viewport_size().height * bench_ui::WELCOME_TOP)
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
