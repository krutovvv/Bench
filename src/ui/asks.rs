//! What the agent puts to the user beyond its answer: a question with choices (the `clarify`
//! tool) as a card at the end of the chat, and its plan (the `todo` tool) as a foldable strip
//! over the input, steps ticked off as it goes.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::icons::Ico;

impl BenchApp {
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
                                .border_1()
                                .rounded_md()
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
                .border_1()
                .rounded_md()
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

    /// The plan as a row of the thread, the agent's: a block like an insert's (its avatar, its
    /// `block_head`) with the chevron, "PLAN · 3/7" and, folded, the step in progress; the steps
    /// under the head when open. It goes when no step is left to do (`Evt::Todos`), so an ended
    /// plan never lingers.
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
                        .items_center()
                        .text_base()
                        .child(icon.icon().xsmall().text_color(color))
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
        let fold = if open {
            Ico::ChevronDown
        } else {
            Ico::ChevronRight
        };
        let head = super::block_head(&theme, open)
            .id("plan-head")
            .gap_1p5()
            .cursor_pointer()
            .hover(|s| s.text_color(theme.foreground))
            .child(fold.icon().xsmall())
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
            .when(!open, |d| d.child(div().flex_1()))
            .on_click(cx.listener(|this, _, _, cx| {
                // Folding is the thread's own click: it must not pull the focus to the composer.
                this.click_claimed = true;
                this.plan_open = !this.plan_open;
                cx.notify();
            }));
        let frame = v_flex()
            .id("plan-block")
            .w_full()
            .min_w_0()
            .flex_none()
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .overflow_hidden()
            .child(head)
            .when(open, |d| {
                d.child(
                    v_flex()
                        .id("plan-steps")
                        .px_3()
                        .py_2()
                        .gap_0p5()
                        .children(steps),
                )
            });
        // The agent's row, as an insert's or an answer's: its avatar, the block in the text column.
        Some(
            h_flex()
                .w_full()
                .flex_none()
                .id("plan-row")
                .pl(px(super::CHAT_LEFT))
                .pr(px(super::CHAT_RIGHT))
                .py(px(super::TURN_Y))
                .gap(px(super::CHAT_TEXT - super::CHAT_LEFT - super::AVATAR))
                .items_start()
                .child(super::avatar(true, &theme))
                .child(div().flex_1().min_w_0().child(frame))
                .into_any_element(),
        )
    }
}
