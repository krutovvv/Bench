//! The conversation: one row per turn — user message, folded steps, answer,
//! sources (always) and actions (on hover).

use std::rc::Rc;

use gpui_kit::component::message_scroller::MessageScroller;
use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::text::TextView;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{clip, clock, plural};
use crate::app::{HermesApp, tool_label};
use crate::history::host_of;
use crate::icons::Ico;
use crate::model::{Step, Turn};

/// What a turn row needs besides the turn itself.
#[derive(Clone)]
struct RowState {
    live: Option<(SharedString, u64)>,
    playing: bool,
    speaking: bool,
    last: bool,
}

impl HermesApp {
    pub(super) fn render_chat(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        if self.turns.is_empty() && self.session.is_some() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    ShimmerText::new("Загружаю историю…")
                        .text_sm()
                        .text_color(theme.muted_foreground),
                )
                .into_any_element();
        }
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
            let last = i + 1 == turns.len();
            let state = RowState {
                live: live.clone().filter(|_| last),
                playing: playing == Some(i),
                speaking: speaking == Some(i),
                last,
            };
            render_turn(i, turn, state, &view, cx)
        })
        .size_full()
        .into_any_element()
    }
}

fn icon_button(id: impl Into<ElementId>, icon: Ico, tip: &'static str, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    div()
        .id(id)
        .p_1()
        .cursor_pointer()
        .text_color(theme.muted_foreground)
        .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
        .child(icon.icon().xsmall())
        .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
}

fn render_turn(
    i: usize,
    turn: &Turn,
    st: RowState,
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
    let group: SharedString = format!("turn{i}").into();

    // One-line summary of the steps; only the error count is red.
    let summary = (has_steps || st.live.is_some()).then(|| {
        let toggle = view.clone();
        let mut tail: Vec<String> = vec![];
        if tools > 0 {
            tail.push(plural(tools, "действие", "действия", "действий"));
        }
        if let Some((_, s)) = &st.live {
            tail.push(clock(*s));
        }
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
                (if turn.expanded {
                    Ico::ChevronDown
                } else {
                    Ico::ChevronRight
                })
                .icon()
                .xsmall()
                .when(!has_steps, |i| i.text_color(theme.transparent)),
            )
            .map(|d| match &st.live {
                Some((label, _)) => d.child(
                    ShimmerText::new(label.clone())
                        .text_color(muted)
                        .max_w(px(480.)),
                ),
                None => d.child(div().flex_none().child("Готово")),
            })
            .when(!tail.is_empty(), |d| {
                d.child(div().flex_none().child(format!("· {}", tail.join(" · "))))
            })
            .when(errors > 0, |d| {
                d.child(div().flex_none().text_color(theme.danger).child(format!(
                    "· {}",
                    plural(errors, "ошибка", "ошибки", "ошибок")
                )))
            })
            .on_click(move |_, _, cx| toggle.update(cx, |this, cx| this.toggle_turn(i, cx)))
    });

    let steps = turn.expanded.then(|| {
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
                        let color = if *failed { theme.danger } else { muted };
                        h_flex()
                            .gap_2()
                            .items_start()
                            .child(
                                (if *failed {
                                    Ico::CircleAlert
                                } else {
                                    Ico::Terminal
                                })
                                .icon()
                                .xsmall()
                                .mt(px(3.))
                                .text_color(color),
                            )
                            .child(
                                div()
                                    .text_color(color)
                                    .child(clip(&tool_label(name, detail), 220)),
                            )
                            .into_any_element()
                    }
                    Step::Note(text) => div()
                        .text_color(theme.foreground.opacity(0.75))
                        .child(clip(text, 400))
                        .into_any_element(),
                    Step::Thought(text) => h_flex()
                        .gap_2()
                        .items_start()
                        .child(Ico::Brain.icon().xsmall().mt(px(3.)).text_color(muted))
                        .child(div().text_xs().text_color(muted).child(clip(text, 600)))
                        .into_any_element(),
                }
            }))
    });

    let answer = (!turn.answer.trim().is_empty()).then(|| {
        TextView::markdown(
            SharedString::from(format!("answer-{i}")),
            turn.answer.clone(),
        )
    });

    // Sources (left, always) and actions (right, on hover).
    let sources = render_sources(i, &turn.sources, cx);
    let actions = (st.live.is_none() && !turn.answer.trim().is_empty()).then(|| {
        let (copy, speak, regen) = (view.clone(), view.clone(), view.clone());
        let time = match (turn.started, turn.finished) {
            (Some(a), Some(b)) if b > a => clock((b - a) as u64),
            _ => String::new(),
        };
        let when = turn.finished.or(turn.started).unwrap_or_default();
        h_flex()
            .gap_0p5()
            .items_center()
            .opacity(0.)
            .group_hover(group.clone(), |s| s.opacity(1.))
            .when(!time.is_empty(), |d| {
                d.child(
                    div()
                        .id(("time", i))
                        .px_1()
                        .text_xs()
                        .text_color(muted)
                        .child(time)
                        .tooltip(move |window, cx| {
                            Tooltip::new(format_when(when)).build(window, cx)
                        }),
                )
            })
            .child(
                icon_button(("copy", i), Ico::Copy, "Копировать", cx).on_click(
                    move |_, window, cx| {
                        copy.update(cx, |this, cx| this.copy_answer(i, window, cx))
                    },
                ),
            )
            .child(
                icon_button(
                    ("speak", i),
                    if st.playing {
                        Ico::Square
                    } else if st.speaking {
                        Ico::Loader
                    } else {
                        Ico::Volume
                    },
                    if st.playing {
                        "Остановить"
                    } else {
                        "Озвучить"
                    },
                    cx,
                )
                .on_click(move |_, window, cx| {
                    speak.update(cx, |this, cx| this.toggle_speak(i, window, cx))
                }),
            )
            .when(st.last, |d| {
                d.child(
                    icon_button(("regen", i), Ico::RotateCcw, "Ответить заново", cx)
                        .on_click(move |_, _, cx| regen.update(cx, |this, cx| this.regenerate(cx))),
                )
            })
    });

    h_flex()
        .id(("turn", i))
        .group(group)
        .w_full()
        .px_4()
        .py_3()
        .child(
            v_flex()
                .w_full()
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
                .children(answer)
                .child(
                    h_flex()
                        .min_h(px(22.))
                        .gap_2()
                        .items_start()
                        .child(div().flex_1().min_w_0().children(sources))
                        .children(actions),
                ),
        )
        .into_any_element()
}

/// One chip per site; click opens its first page, the tooltip lists them all.
fn render_sources(i: usize, urls: &[String], cx: &App) -> Option<AnyElement> {
    if urls.is_empty() {
        return None;
    }
    let theme = cx.theme().clone();
    let mut hosts: Vec<(String, Vec<String>)> = vec![];
    for u in urls {
        let h = host_of(u);
        match hosts.iter_mut().find(|(x, _)| *x == h) {
            Some((_, pages)) => pages.push(u.clone()),
            None => hosts.push((h, vec![u.clone()])),
        }
    }
    hosts.sort_by_key(|(_, p)| std::cmp::Reverse(p.len()));
    let chips = hosts
        .into_iter()
        .take(10)
        .enumerate()
        .map(|(k, (host, pages))| {
            let first = pages[0].clone();
            let list: SharedString = pages.join("\n").into();
            h_flex()
                .id(SharedString::from(format!("src-{i}-{k}")))
                .gap_1()
                .px_1p5()
                .py_0p5()
                .border_1()
                .border_color(theme.border)
                .text_xs()
                .text_color(theme.muted_foreground)
                .cursor_pointer()
                .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                .child(Ico::Globe.icon().xsmall())
                .child(host)
                .when(pages.len() > 1, |d| {
                    d.child(div().opacity(0.6).child(pages.len().to_string()))
                })
                .tooltip(move |window, cx| Tooltip::new(list.clone()).build(window, cx))
                .on_click(move |_, _, cx| cx.open_url(&first))
        });
    Some(
        h_flex()
            .flex_wrap()
            .gap_1()
            .children(chips)
            .into_any_element(),
    )
}

/// "21 окт 22:15" for a Unix timestamp, in local time.
fn format_when(ts: f64) -> String {
    let out = std::process::Command::new("date")
        .args(["-r", &(ts as i64).to_string(), "+%d.%m.%Y %H:%M"])
        .output();
    out.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}
