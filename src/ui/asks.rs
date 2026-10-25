//! What the agent puts to the user beyond its answer: a question with choices (the `clarify`
//! tool) as a card at the end of the chat, and its plan (the `todo` tool) as a foldable strip
//! over the input, steps ticked off as it goes.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::HermesApp;
use crate::icons::Ico;

impl HermesApp {
    /// The question card: choices (one click answers; with multi-select, tick then Send), an
    /// answer of your own, or Decline.
    pub(super) fn render_clarify(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let req = self.clarify.clone()?;
        let theme = cx.theme().clone();
        let picked = self.clarify_picked.clone();
        let choices: Vec<AnyElement> = req
            .choices
            .iter()
            .enumerate()
            .map(|(k, choice)| {
                let label = choice.clone();
                if req.multi_select {
                    let on = picked.contains(&k);
                    h_flex()
                        .id(("clarify-pick", k))
                        .gap_2()
                        .py_1()
                        .items_center()
                        .text_base()
                        .cursor_pointer()
                        .hover(|s| s.text_color(theme.foreground))
                        .child(
                            div()
                                .size(px(14.))
                                .flex_none()
                                .border_1().rounded_md()
                                .border_color(if on { theme.primary } else { theme.input })
                                .when(on, |d| d.bg(theme.primary)),
                        )
                        .child(div().flex_1().min_w_0().child(label))
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.toggle_clarify_choice(k, cx)),
                        )
                        .into_any_element()
                } else {
                    Button::new(("clarify-choice", k))
                        .small()
                        .outline()
                        .label(label.clone())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.answer_clarify(label.clone(), window, cx)
                        }))
                        .into_any_element()
                }
            })
            .collect();
        let any_picked = !picked.is_empty();
        Some(
            v_flex()
                .ml(px(super::CHAT_TEXT))
                .mr(px(super::CHAT_RIGHT))
                .mt_2()
                .p_3()
                .gap_2()
                .border_1().rounded_md()
                .border_color(theme.primary)
                .child(
                    h_flex()
                        .gap_2()
                        .items_start()
                        .text_base()
                        .child(
                            Ico::CircleHelp
                                .icon()
                                .small()
                                .text_color(theme.primary)
                                .mt(px(2.)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .font_weight(FontWeight::MEDIUM)
                                .child(req.question.clone()),
                        ),
                )
                .when(!choices.is_empty(), |d| {
                    if req.multi_select {
                        d.child(v_flex().pl_6().children(choices))
                    } else {
                        d.child(h_flex().pl_6().flex_wrap().gap_2().children(choices))
                    }
                })
                .child(
                    h_flex()
                        .pl_6()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(Input::new(&self.clarify_input).small()),
                        )
                        .when(req.multi_select, |d| {
                            d.child(
                                Button::new("clarify-send")
                                    .small()
                                    .primary()
                                    .label("Send selected")
                                    .when(!any_picked, |b| b.ghost())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.send_clarify_picks(window, cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("clarify-decline")
                                .small()
                                .ghost()
                                .label("Decline")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.answer_clarify(String::new(), window, cx)
                                })),
                        ),
                )
                .into_any_element(),
        )
    }

    /// The plan strip over the input: "Plan · 3/7 · <the step in progress>", unfolding into
    /// the steps with their state.
    pub(super) fn render_plan(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.todos.is_empty() {
            return None;
        }
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let open = self.plan_open;
        let done = self
            .todos
            .iter()
            .filter(|t| t.status == "completed")
            .count();
        let total = self
            .todos
            .iter()
            .filter(|t| t.status != "cancelled")
            .count();
        let current = self
            .todos
            .iter()
            .find(|t| t.status == "in_progress")
            .or_else(|| self.todos.iter().find(|t| t.status == "pending"))
            .map(|t| t.content.clone());
        let steps: Vec<AnyElement> = if open {
            self.todos
                .iter()
                .map(|t| {
                    let (icon, color, strike) = match t.status.as_str() {
                        "completed" => (Ico::CircleCheck, theme.success, true),
                        "in_progress" => (Ico::CircleDot, theme.primary, false),
                        "cancelled" => (Ico::X, muted, true),
                        _ => (Ico::CircleEmpty, muted, false),
                    };
                    h_flex()
                        .gap_2()
                        .py_0p5()
                        .pl(px(16. * t.depth as f32))
                        .items_start()
                        .text_base()
                        .child(icon.icon().xsmall().text_color(color).mt(px(3.)))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .when(strike, |d| d.line_through().text_color(muted))
                                .when(t.status == "in_progress", |d| {
                                    d.text_color(theme.foreground)
                                        .font_weight(FontWeight::MEDIUM)
                                })
                                .child(t.content.clone()),
                        )
                        .into_any_element()
                })
                .collect()
        } else {
            vec![]
        };
        Some(
            v_flex()
                .w_full()
                .flex_none()
                .border_t_1()
                .border_color(theme.border)
                .bg(theme.background)
                // The rule spans the window; the content starts at the thread's text.
                .child(
                    v_flex()
                        .w_full()
                        .child(
                            h_flex()
                                .id("plan-head")
                                .pl(px(super::CHAT_TEXT))
                                .pr(px(super::CHAT_RIGHT))
                                .py_1p5()
                                .gap_1p5()
                                .items_center()
                                .text_base()
                                .text_color(muted)
                                .cursor_pointer()
                                .hover(|s| s.text_color(theme.foreground))
                                .child(
                                    if open {
                                        Ico::ChevronDown
                                    } else {
                                        Ico::ChevronRight
                                    }
                                    .icon()
                                    .xsmall(),
                                )
                                .child(Ico::ListChecks.icon().xsmall())
                                .child(div().flex_none().child(format!("PLAN · {done}/{total}")))
                                .when_some(current.filter(|_| !open), |d, step| {
                                    d.child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_color(theme.foreground)
                                            .child(format!("· {step}")),
                                    )
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.plan_open = !this.plan_open;
                                    cx.notify();
                                })),
                        )
                        .when(open, |d| {
                            d.child(
                                v_flex()
                                    .id("plan-steps")
                                    .pl(px(super::CHAT_TEXT))
                                    .pr(px(super::CHAT_RIGHT))
                                    .pb_2()
                                    .max_h(px(240.))
                                    .overflow_y_scroll()
                                    .children(steps),
                            )
                        }),
                )
                .into_any_element(),
        )
    }
}
