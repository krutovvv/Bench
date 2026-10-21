//! The conversation: one row per turn — user bubble, folded steps, answer.

use std::rc::Rc;

use gpui_kit::component::message_scroller::MessageScroller;
use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{clip, clock, plural};
use crate::app::{HermesApp, tool_label};
use crate::model::{Step, Turn};

pub const COLUMN: f32 = 900.;

impl HermesApp {
    pub(super) fn render_chat(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        if self.turns.is_empty() {
            let profile = self.current_profile();
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_2()
                .child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Чем займёмся?"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(format!("{} · {profile}", self.target.label())),
                )
                .into_any_element();
        }

        let view = cx.entity();
        let turns: Rc<Vec<Turn>> = Rc::new(self.turns.clone());
        let live = self
            .live
            .as_ref()
            .map(|l| (l.label.clone(), l.started.elapsed().as_secs()));
        let playing = self.playing.as_ref().map(|(i, _)| *i);
        let speaking = self.speaking;

        MessageScroller::new("chat", self.scroller.clone(), move |i, _, cx| {
            let Some(turn) = turns.get(i) else {
                return div().into_any_element();
            };
            let is_last = i + 1 == turns.len();
            render_turn(
                i,
                turn,
                live.clone().filter(|_| is_last),
                playing == Some(i),
                speaking == Some(i),
                &view,
                cx,
            )
        })
        .size_full()
        .into_any_element()
    }
}

fn render_turn(
    i: usize,
    turn: &Turn,
    live: Option<(SharedString, u64)>,
    playing: bool,
    speaking: bool,
    view: &Entity<HermesApp>,
    cx: &mut App,
) -> AnyElement {
    let theme = cx.theme().clone();
    let muted = theme.muted_foreground;
    let tools = turn.tool_count();
    let errors = turn
        .steps
        .iter()
        .filter(|s| matches!(s, Step::Tool { failed: true, .. }))
        .count();
    let has_steps = !turn.steps.is_empty();

    // The one-line summary, clickable when there is something to unfold.
    let summary = (has_steps || live.is_some()).then(|| {
        let tail = [
            (tools > 0).then(|| plural(tools, "действие", "действия", "действий")),
            (errors > 0).then(|| plural(errors, "ошибка", "ошибки", "ошибок")),
            live.as_ref().map(|(_, s)| clock(*s)),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        let toggle = view.clone();
        h_flex()
            .id(("steps", i))
            .gap_1p5()
            .items_center()
            .text_sm()
            .text_color(muted)
            .min_w_0()
            .when(has_steps, |d| {
                d.cursor_pointer().hover(|s| s.text_color(theme.foreground))
            })
            .child(
                Icon::new(if turn.expanded {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .xsmall()
                .when(!has_steps, |i| i.text_color(theme.transparent)),
            )
            .map(|d| match &live {
                Some((label, _)) => d.child(
                    ShimmerText::new(label.clone())
                        .text_color(muted)
                        .max_w(px(480.)),
                ),
                None => d.child(div().flex_none().child("Готово")),
            })
            .when(!tail.is_empty(), |d| {
                d.child(
                    div()
                        .flex_none()
                        .text_color(if errors > 0 { theme.danger } else { muted })
                        .child(tail),
                )
            })
            .on_click(move |_, _, cx| view_toggle(&toggle, i, cx))
    });

    let steps =
        turn.expanded.then(|| {
            v_flex()
                .ml(px(6.))
                .pl_4()
                .gap_1p5()
                .border_l_1()
                .border_color(theme.border)
                .text_sm()
                .children(turn.steps.iter().map(|step| {
                    match step {
                        Step::Tool {
                            name,
                            detail,
                            failed,
                        } => {
                            h_flex()
                                .gap_2()
                                .items_start()
                                .child(
                                    Icon::new(if *failed {
                                        IconName::CircleAlert
                                    } else {
                                        IconName::SquareTerminal
                                    })
                                    .xsmall()
                                    .mt(px(3.))
                                    .text_color(if *failed { theme.danger } else { muted }),
                                )
                                .child(
                                    div()
                                        .text_color(if *failed { theme.danger } else { muted })
                                        .child(clip(&tool_label(name, detail), 220)),
                                )
                                .into_any_element()
                        }
                        Step::Note(text) => div()
                            .text_color(theme.foreground.opacity(0.75))
                            .child(clip(text, 400))
                            .into_any_element(),
                        Step::Thought(text) => v_flex()
                            .gap_0p5()
                            .child(div().text_xs().text_color(muted).child("Размышление"))
                            .child(div().text_xs().text_color(muted).child(clip(text, 600)))
                            .into_any_element(),
                    }
                }))
        });

    let answer = (!turn.answer.trim().is_empty()).then(|| {
        let speak = view.clone();
        v_flex()
            .gap_1()
            .child(TextView::markdown(
                SharedString::from(format!("answer-{i}")),
                turn.answer.clone(),
            ))
            .when(live.is_none(), |d| {
                d.child(
                    h_flex().child(
                        div()
                            .id(("speak", i))
                            .p_1()
                            .cursor_pointer()
                            .text_color(muted)
                            .hover(|s| s.bg(theme.secondary).text_color(theme.foreground))
                            .child(
                                Icon::new(if playing {
                                    IconName::Pause
                                } else if speaking {
                                    IconName::LoaderCircle
                                } else {
                                    IconName::Play
                                })
                                .xsmall(),
                            )
                            .on_click(move |_, window, cx| {
                                speak.update(cx, |this, cx| this.toggle_speak(i, window, cx));
                            }),
                    ),
                )
            })
    });

    h_flex()
        .id(("turn", i))
        .w_full()
        .px_4()
        .py_3()
        .child(
            v_flex()
                .w_full()
                .max_w(px(COLUMN))
                .gap_3()
                .child(
                    div()
                        .w_full()
                        .px_3()
                        .py_2()
                        .bg(theme.secondary)
                        .border_1()
                        .border_color(theme.border)
                        .whitespace_normal()
                        .child(turn.user.clone()),
                )
                .children(summary)
                .children(steps)
                .children(answer),
        )
        .into_any_element()
}

fn view_toggle(view: &Entity<HermesApp>, i: usize, cx: &mut App) {
    view.update(cx, |this, cx| this.toggle_turn(i, cx));
}
