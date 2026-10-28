//! The agent's cards that grow inside its work: the plan it keeps (the `todo` tool) and the
//! questions it puts to the person (`clarify`, an approval). They are journal items of the turn
//! that wrote them — the cards stand in the work row, tick and settle there, and come back
//! exactly so on reopen.

use std::collections::HashSet;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use thread::{AskCard, PlanStep};

use crate::thread::ThreadView;
use bench_ui::icons::Ico;

impl ThreadView {
    /// The work row's cards, under its steps: its plan, then one per question.
    pub(crate) fn render_work_artifacts(
        &mut self,
        plan: Option<&(String, Vec<PlanStep>)>,
        asks: &[AskCard],
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut els: Vec<AnyElement> = Vec::new();
        if let Some((id, todos)) = plan {
            els.push(self.render_plan_frame(id, todos, cx));
        }
        for card in asks {
            els.push(self.render_ask_card(card, cx));
        }
        els
    }

    /// The plan: a block with the chevron, "PLAN · 3/7" and, folded, the step in progress; the
    /// steps under the head when open. Folded per plan, by its id.
    fn render_plan_frame(
        &mut self,
        id: &str,
        todos: &[PlanStep],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let open = self.thread.plan_open.contains(id);
        let done = todos.iter().filter(|t| t.status == "completed").count();
        let total = todos.iter().filter(|t| t.status != "cancelled").count();
        let current = todos
            .iter()
            .find(|t| t.status == "in_progress")
            .or_else(|| todos.iter().find(|t| t.status == "pending"))
            .map(|t| t.content.clone());
        let depth = |t: &PlanStep| {
            usize::from(
                t.parent
                    .as_deref()
                    .is_some_and(|p| todos.iter().any(|x| x.id == *p)),
            )
        };
        let steps: Vec<AnyElement> = if open {
            todos
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
                        .pl(px(16. * depth(t) as f32))
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
        let head = bench_ui::fold_head(
            SharedString::from(format!("plan-head-{id}")),
            open,
            Some(Ico::ListChecks),
            format!("PLAN · {done}/{total}"),
            cx,
        )
        // As short as the thread's other small rows, not as tall as a block's head.
        .h(px(bench_ui::CHIP_H))
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
            .on_click(cx.listener({
                let id = id.to_string();
                move |this, _, _, cx| {
                    // Folding is the thread's own click: it must not pull the focus to the composer.
                    this.claim_click(cx);
                    toggle(&mut this.thread.plan_open, &id);
                    cx.notify();
                }
            }));
        bench_ui::block_frame(cx)
            .id(SharedString::from(format!("plan-block-{id}")))
            .flex_none()
            .child(head)
            .when(open, |d| {
                d.child(v_flex().px_3().py_2().gap_0p5().children(steps))
            })
            .into_any_element()
    }

    /// The question, in the work that asked it. Waiting and live, it is a card one can answer
    /// (choices, one's own words, or its cross to decline; an approval offers its choices). Once settled —
    /// or waiting on an agent that no longer asks — it says what was asked and what was answered.
    fn render_ask_card(&mut self, card: &AskCard, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let approve = card.ask_kind == "approve";
        let accent = if approve {
            theme.warning
        } else {
            theme.primary
        };
        let settled = card.answer.is_some();
        // Only the question the agent is waiting for right now takes answers: the journal also
        // shows settled ones and, after a restart, ones nobody waits for.
        let live = !settled && self.thread.journal.asks().iter().any(|a| a.id == card.id);
        let head = h_flex()
            .gap_2()
            .items_start()
            .text_base()
            .child(
                if approve {
                    Ico::CircleAlert
                } else {
                    Ico::CircleHelp
                }
                .icon()
                .small()
                .text_color(if settled { muted } else { accent })
                .mt(px(2.)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .font_weight(FontWeight::MEDIUM)
                    .when(settled, |d| d.text_color(muted))
                    .child(card.question.clone()),
            )
            // Declining is closing the question: its cross in the card's corner, not a button
            // among the answers.
            .when(live && !approve, |d| {
                d.child(
                    bench_ui::head_icon_button("clarify-decline", Ico::X, "Decline", cx)
                        .flex_none()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.claim_click(cx);
                            this.answer_clarify(String::new(), window, cx)
                        })),
                )
            });
        let body =
            if settled {
                v_flex().pl_6().when_some(card.answer.clone(), |d, a| {
                    if a.is_empty() {
                        d.child(div().text_color(muted).child("No answer"))
                    } else {
                        d.child(
                            h_flex()
                                .gap_2()
                                .items_start()
                                .child(
                                    Ico::CircleCheck
                                        .icon()
                                        .xsmall()
                                        .text_color(theme.success)
                                        .mt(px(4.)),
                                )
                                .child(div().flex_1().min_w_0().child(a)),
                        )
                    }
                })
            } else if !live {
                v_flex().pl_6().child(div().text_color(muted).child(format!(
                    "Waiting for your answer…{}",
                    if card.choices.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", card.choices.join(", "))
                    }
                )))
            } else if approve {
                // An approval: its choices as buttons, the first primary, "deny" ghost.
                let label = |c: &str| match c {
                    "once" => "Allow once",
                    "session" => "Allow for this chat",
                    "always" => "Always allow",
                    "deny" => "Deny",
                    _ => "OK",
                };
                v_flex()
                    .gap_2()
                    .child(h_flex().flex_wrap().gap_2().children(
                        card.choices.iter().enumerate().map(|(k, c)| {
                            let choice = c.clone();
                            let id = card.id.clone();
                            let b = Button::new(("approve", k)).small().label(label(c));
                            let b = if k == 0 {
                                b.primary()
                            } else if c == "deny" {
                                b.ghost()
                            } else {
                                b.outline()
                            };
                            b.on_click(cx.listener(move |this, _, _, cx| {
                                this.answer_ask(id.clone(), choice.clone(), cx)
                            }))
                        }),
                    ))
            } else {
                // A clarify: choices (one click answers; with multi-select, tick then Send), an
                // answer of your own; the head's cross declines.
                let picked = self.thread.clarify_picked.clone();
                let choices: Vec<AnyElement> = card
                    .choices
                    .iter()
                    .enumerate()
                    .map(|(k, choice)| {
                        let label = choice.clone();
                        if card.multi {
                            let on = picked.contains(&k);
                            h_flex()
                                .id(("clarify-pick", k))
                                .gap_2()
                                .py_1()
                                .items_start()
                                .text_base()
                                .cursor_pointer()
                                .hover(|s| s.text_color(theme.foreground))
                                .child(
                                    div()
                                        .size(px(14.))
                                        .flex_none()
                                        .mt(px(2.))
                                        .border_1()
                                        .rounded_md()
                                        .border_color(if on { theme.primary } else { theme.input })
                                        .when(on, |d| d.bg(theme.primary)),
                                )
                                .child(div().flex_1().min_w_0().child(label))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.toggle_clarify_choice(k, cx)
                                }))
                                .into_any_element()
                        } else {
                            // A row as wide as the card, its text wrapped: a Button is one line as long
                            // as the choice and ran past the card.
                            bench_ui::choice_row(("clarify-choice", k), cx)
                                .child(label.clone())
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.answer_clarify(label.clone(), window, cx)
                                }))
                                .into_any_element()
                        }
                    })
                    .collect();
                let any_picked = !picked.is_empty();
                v_flex()
                    .gap_2()
                    .when(!choices.is_empty(), |d| {
                        if card.multi {
                            d.child(v_flex().pl_6().children(choices))
                        } else {
                            d.child(v_flex().pl_6().gap_2().children(choices))
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
                                    // The field types itself: the click is its, or the thread's
                                    // mouse-up hands the focus back to the composer the moment the
                                    // button is released.
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _: &MouseDownEvent, _, cx| {
                                            this.claim_click(cx)
                                        }),
                                    )
                                    // `medium` as high as a choice row: the answer's own line should
                                    // not read as the shabby cousin of the offered ones.
                                    .child(Input::new(&self.clarify_input)),
                            )
                            .when(card.multi, |d| {
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
                            }),
                    )
            };
        v_flex()
            .id(SharedString::from(format!("ask-card-{}", card.id)))
            .w_full()
            .min_w_0()
            .p_3()
            .gap_2()
            .border_1()
            .rounded_md()
            .border_color(if settled { theme.border } else { accent })
            .child(head)
            .when_some(card.detail.clone(), |d, cmd| {
                d.child(
                    div()
                        .p_2()
                        .bg(theme.secondary)
                        .font_family(bench_ui::theme::MONO)
                        .text_base()
                        .child(cmd),
                )
            })
            .child(body)
            .into_any_element()
    }
}

fn toggle(set: &mut HashSet<String>, id: &str) {
    if !set.remove(id) {
        set.insert(id.to_string());
    }
}
