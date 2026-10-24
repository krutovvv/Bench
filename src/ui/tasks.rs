//! Tasks mode's views, after Things: the lists (Today, Upcoming, Anytime, Logbook) in the
//! sidebar; the open list in the middle with its tags as a filter strip, Today with "This
//! Evening", Upcoming as the next seven days then months, the Logbook by day then month; the open
//! task's card on the right.

use gpui_kit::component::input::{Input, Textarea};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::tools::segmented;
use crate::app::HermesApp;
use crate::app::tasks::RANGES;
use crate::icons::Ico;
use crate::tasks::{self, List, Task, today};

fn list_icon(list: &List) -> Ico {
    match list {
        List::Today => Ico::Star,
        List::Upcoming => Ico::CalendarDays,
        List::Anytime => Ico::Layers,
        List::Logbook => Ico::BookCheck,
    }
}

use crate::tasks::LISTS;

impl HermesApp {
    pub(super) fn render_tasks_sidebar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let compact = self.appearance.compact_sidebar;
        let row = |id: SharedString,
                   list: List,
                   label: String,
                   count: Option<usize>,
                   cx: &mut Context<Self>| {
            let selected = self.tasks.list == list && self.tasks.query.is_empty();
            let icon = list_icon(&list);
            h_flex()
                .id(id)
                .gap_2()
                .pl(px(14.))
                .pr_3()
                .map(|d| if compact { d.py_1() } else { d.py_2() })
                .items_center()
                .border_l_2()
                .border_color(if selected {
                    theme.primary
                } else {
                    theme.transparent
                })
                .when(selected, |d| d.bg(theme.sidebar_accent))
                .hover(|s| s.bg(theme.sidebar_accent))
                .cursor_pointer()
                .child(icon.icon().small().text_color(if list == List::Today {
                    theme.warning
                } else {
                    muted
                }))
                .child(div().flex_1().min_w_0().text_base().truncate().child(label))
                .when_some(count.filter(|c| *c > 0), |d, c| {
                    d.child(div().text_sm().text_color(muted).child(c.to_string()))
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.select_task_list(list.clone(), window, cx)
                }))
                .into_any_element()
        };
        let mut items: Vec<AnyElement> = vec![];
        for (k, list) in LISTS.iter().enumerate() {
            let count = match list {
                List::Today | List::Upcoming | List::Anytime => Some(self.tasks.count(list)),
                _ => None,
            };
            items.push(row(
                SharedString::from(format!("task-list-{k}")),
                list.clone(),
                list.name(),
                count,
                cx,
            ));
        }
        v_flex()
            .size_full()
            .bg(theme.sidebar)
            .child(self.render_sidebar_top(cx))
            .child(super::search_band(&self.tasks.search, cx))
            .child(
                v_flex()
                    .id("tasks-lists")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .py_1()
                    .children(items),
            )
            .into_any_element()
    }

    pub(super) fn render_tasks_header(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let list = self.tasks.list.clone();
        let searching = !self.tasks.query.is_empty();
        let title = if searching {
            "Search".to_string()
        } else {
            list.name()
        };
        let hint = if searching {
            format!("“{}” in all tasks", self.tasks.query)
        } else {
            match &list {
                List::Today => "Planned for today and earlier".into(),
                List::Upcoming => "Planned for later days".into(),
                List::Anytime => "No day yet".into(),
                List::Logbook => "Done, newest first".into(),
            }
        };
        let ranged = list == List::Logbook && !searching;
        let labels: Vec<&'static str> = RANGES.iter().map(|(l, _)| *l).collect();
        h_flex()
            .id("header")
            .h(px(36.))
            .flex_none()
            .bg(theme.title_bar)
            .border_b_1()
            .border_color(theme.border)
            .pl_4()
            .pr_2()
            .gap_3()
            .items_center()
            .child(
                h_flex()
                    .flex_none()
                    .gap_1p5()
                    .items_center()
                    .text_base()
                    .font_weight(FontWeight::MEDIUM)
                    .child(
                        list_icon(&list)
                            .icon()
                            .small()
                            .text_color(theme.muted_foreground),
                    )
                    .child(title),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(hint),
            )
            .when(ranged, |d| {
                d.child(segmented(
                    "task-range",
                    &labels,
                    self.tasks.range,
                    cx,
                    |this, k, _, cx| this.set_task_range(k, cx),
                ))
            })
            .child(self.render_header_tail(cx))
            .into_any_element()
    }

    /// A task list as a thread's live block: the same rows, no scroll of its own (the thread
    /// scrolls), the open task's card right under the list, the text at the thread's column.
    pub(super) fn render_tasks_block(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.tasks_view(true, cx)
    }

    pub(super) fn render_tasks_body(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.tasks_view(false, cx)
    }

    fn tasks_view(&mut self, inline: bool, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let day = today();
        let list = self.tasks.list.clone();
        let shown = self.tasks.shown(&list);
        // Sections: (title, icon, tasks); an Upcoming day header is drawn big (number + name).
        let mut sections: Vec<(Option<String>, Option<Ico>, Vec<Task>)> = vec![];
        let month = |d: chrono::NaiveDate| {
            if chrono::Datelike::year(&d) == chrono::Datelike::year(&day) {
                d.format("%B").to_string()
            } else {
                d.format("%B %Y").to_string()
            }
        };
        if !self.tasks.query.is_empty() {
            sections.push((None, None, shown));
        } else {
            match list {
                List::Today => {
                    let (evening, day_tasks): (Vec<Task>, Vec<Task>) =
                        shown.into_iter().partition(|t| t.evening);
                    sections.push((None, None, day_tasks));
                    if !evening.is_empty() {
                        sections.push((Some("This Evening".into()), Some(Ico::Moon), evening));
                    }
                }
                List::Upcoming => {
                    // The next seven days, empty or not, then by month.
                    for k in 1..=7 {
                        let d = day + chrono::Duration::days(k);
                        let label = format!(
                            "{}\u{0}{}",
                            d.format("%-d"),
                            if k == 1 {
                                "Tomorrow".to_string()
                            } else {
                                d.format("%A").to_string()
                            }
                        );
                        let on: Vec<Task> =
                            shown.iter().filter(|t| t.day == Some(d)).cloned().collect();
                        sections.push((Some(label), None, on));
                    }
                    for t in shown
                        .into_iter()
                        .filter(|t| t.day.is_some_and(|d| d > day + chrono::Duration::days(7)))
                    {
                        let label = t.day.map(month);
                        match sections.last_mut() {
                            Some((l, _, v)) if *l == label => v.push(t),
                            _ => sections.push((label, None, vec![t])),
                        }
                    }
                }
                List::Logbook => {
                    for t in shown {
                        let label = t.completed.map(|d| match (day - d).num_days() {
                            0 => "Today".to_string(),
                            1 => "Yesterday".to_string(),
                            _ => month(d),
                        });
                        match sections.last_mut() {
                            Some((l, _, v)) if *l == label => v.push(t),
                            _ => sections.push((label, None, vec![t])),
                        }
                    }
                }
                _ => sections.push((None, None, shown)),
            }
        }
        let empty = sections.iter().all(|(_, _, v)| v.is_empty()) && list != List::Upcoming;
        let mut rows: Vec<AnyElement> = vec![];
        let mut n = 0;
        for (title, icon, list_tasks) in sections {
            let has_header = title.is_some();
            // An Upcoming day: "24  Tomorrow" over a rule, like Things' plan.
            if let Some((num, name)) = title.as_deref().and_then(|t| t.split_once('\u{0}')) {
                rows.push(
                    h_flex()
                        .flex_none()
                        .px_6()
                        .pt_4()
                        .pb_1()
                        .gap_2()
                        .items_baseline()
                        .border_b_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(num.to_string()),
                        )
                        .child(div().text_base().text_color(muted).child(name.to_string()))
                        .into_any_element(),
                );
            } else if let Some(title) = title {
                rows.push(
                    h_flex()
                        .flex_none()
                        .px_6()
                        .pt_4()
                        .pb_1()
                        .gap_1p5()
                        .items_center()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(muted)
                        .border_b_1()
                        .border_color(theme.border)
                        .when_some(icon, |d, i| d.child(i.icon().xsmall()))
                        .child(title)
                        .into_any_element(),
                );
            }
            // Some air between a section's rule and its first task (the rule belongs to the header).
            if has_header && !list_tasks.is_empty() {
                rows.push(div().h_1().flex_none().into_any_element());
            }
            for t in list_tasks {
                rows.push(self.render_task_row(n, &t, cx));
                n += 1;
                // The open task's card right under its row, whatever the list (the Logbook's
                // thousand rows would push a card at the end out of sight).
                if self.tasks.selected.as_deref() == Some(t.id.as_str()) {
                    if let Some(open) = self.tasks.selected_task().cloned() {
                        let card = self.render_task_card(&open, cx);
                        rows.push(
                            div()
                                .flex_none()
                                .mx_6()
                                .my_1()
                                .border_1()
                                .border_color(theme.border)
                                .child(card)
                                .into_any_element(),
                        );
                    }
                }
            }
        }
        let note = if let Some(e) = &self.tasks.error {
            Some((e.clone(), theme.danger))
        } else if !self.tasks.loaded {
            Some(("Loading tasks…".to_string(), muted))
        } else if empty {
            Some((
                if self.tasks.query.is_empty() {
                    "Nothing here".to_string()
                } else {
                    "No task matches".to_string()
                },
                muted,
            ))
        } else {
            None
        };
        // The list's tags, the most used first: "All · client-a · client-b…".
        let tags = if self.tasks.query.is_empty() {
            self.tasks.tags_in(&list)
        } else {
            vec![]
        };
        let picked = self.tasks.tag_filter.clone();
        let strip = (!tags.is_empty()).then(|| {
            let chip =
                |id: SharedString,
                 label: String,
                 on: bool,
                 tag: Option<String>,
                 cx: &mut Context<Self>| {
                    div()
                        .id(id)
                        .flex_none()
                        .px_2()
                        .py_0p5()
                        .text_sm()
                        .cursor_pointer()
                        .when(on, |d| {
                            d.bg(theme.primary).text_color(theme.primary_foreground)
                        })
                        .when(!on, |d| {
                            d.text_color(muted)
                                .hover(|s| s.text_color(theme.foreground))
                        })
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_task_tag_filter(tag.clone(), cx)
                        }))
                        .into_any_element()
                };
            let mut chips = vec![chip(
                "task-tag-all".into(),
                "All".into(),
                picked.is_none(),
                None,
                cx,
            )];
            for (k, tag) in tags.into_iter().enumerate() {
                let on = picked
                    .as_deref()
                    .is_some_and(|p| p.eq_ignore_ascii_case(&tag));
                chips.push(chip(
                    SharedString::from(format!("task-tag-{k}")),
                    tag.clone(),
                    on,
                    Some(tag),
                    cx,
                ));
            }
            h_flex()
                .id("task-tags")
                .flex_none()
                // Under the header's rule, down to the sidebar search band's bottom rule.
                .min_h(super::bar_h(cx))
                .px_6()
                .py_1()
                .gap_1()
                .flex_wrap()
                .items_center()
                .border_b_1()
                .border_color(theme.border)
                .child(Ico::Tag.icon().xsmall().text_color(muted).mr_1())
                .children(chips)
        });
        if inline {
            // The rows' own px_6 is pulled back so their text starts at the thread's column.
            // In the agent's row: the rows' own px_6 pulled back, so their text starts at the
            // row's text column.
            return v_flex()
                .w_full()
                .ml(px(-24.))
                .mr(px(-24.))
                .child(
                    div()
                        .px_6()
                        .pb_1()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(list.name()),
                )
                .children(strip)
                .child(v_flex().pt_1().children(rows).when_some(note, |d, (text, color)| {
                    d.child(div().px_6().py_3().text_base().text_color(color).child(text))
                }))
                .into_any_element();
        }
        let list_pane = v_flex().flex_1().min_w_0().h_full().children(strip).child(
            v_flex()
                .id("tasks-rows")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .pt_1()
                .pb_4()
                .children(rows)
                .when_some(note, |d, (text, color)| {
                    d.child(div().px_6().py_3().text_base().text_color(color).child(text))
                }),
        );
        h_flex().size_full().items_stretch().child(list_pane).into_any_element()
    }

    fn render_task_row(&mut self, n: usize, t: &Task, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let day = today();
        let selected = self.tasks.selected.as_deref() == Some(t.id.as_str());
        let done = t.done();
        let (id_check, id_open) = (t.id.clone(), t.id.clone());
        let overdue = !done && t.day.is_some_and(|d| d < day);
        // The day, where the list doesn't say it already.
        let logbook = self.tasks.list == List::Logbook && self.tasks.query.is_empty();
        let when = match (&self.tasks.list, t.day, t.completed) {
            (List::Logbook, _, _) if logbook => None,
            (_, _, Some(d)) => Some(tasks::show_date(d, day)),
            (List::Today, Some(d), _) if overdue => Some(tasks::show_date(d, day)),
            (List::Today | List::Upcoming, _, _) if self.tasks.query.is_empty() => None,
            (_, Some(d), _) => Some(tasks::show_date(d, day)),
            _ => None,
        };
        // The Logbook says when each was done, in the accent, before the title.
        let done_on = t
            .completed
            .filter(|_| logbook)
            .map(|d| d.format("%-d %b").to_string());
        let spent = t.spent_now();
        let running = t.timer.is_some();
        h_flex()
            .id(("task-row", n))
            // A long list scrolls instead of squeezing its rows.
            .flex_none()
            // 22 + the 2px selection stripe = the 24 of the headers and the day numbers.
            .pl(px(22.))
            .pr_6()
            .py_1p5()
            .gap_2()
            .items_center()
            .border_l_2()
            .border_color(if selected {
                theme.primary
            } else {
                theme.transparent
            })
            .when(selected, |d| d.bg(theme.list_active))
            .hover(|s| s.bg(theme.list_hover))
            .cursor_pointer()
            .child(
                // The checkbox: a click marks it done (or not), without opening the card.
                div()
                    .id(("task-check", n))
                    .size(px(14.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_1()
                    .border_color(if done { theme.primary } else { theme.input })
                    .when(done, |d| {
                        d.bg(theme.primary)
                            .text_color(theme.primary_foreground)
                            .child(Ico::Check.icon().xsmall())
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.toggle_task_done(id_check.clone(), window, cx)
                    })),
            )
            .when_some(done_on, |d, on| {
                d.child(
                    div()
                        .flex_none()
                        .text_base()
                        .text_color(theme.primary)
                        .child(on),
                )
            })
            .child(
                div()
                    .flex_shrink(1.)
                    .min_w_0()
                    .truncate()
                    .text_base()
                    .when(done && !logbook, |d| d.text_color(muted))
                    .child(if t.title.trim().is_empty() {
                        "New task".to_string()
                    } else {
                        t.title.clone()
                    }),
            )
            .when(!t.notes.trim().is_empty(), |d| {
                d.child(Ico::FileText.icon().xsmall().text_color(muted))
            })
            .children(t.tags.iter().take(3).map(|g| {
                div()
                    .flex_none()
                    .px_1p5()
                    .text_sm()
                    .text_color(muted)
                    .border_1()
                    .border_color(theme.border)
                    .child(g.clone())
            }))
            .when(t.important, |d| {
                d.child(Ico::Flag.icon().xsmall().text_color(theme.danger))
            })
            .child(div().flex_1())
            .when(spent > 0 || running, |d| {
                d.child(
                    h_flex()
                        .flex_none()
                        .gap_1()
                        .items_center()
                        .text_sm()
                        .text_color(if running { theme.primary } else { muted })
                        .child(Ico::Clock.icon().xsmall())
                        .child(tasks::show_spent(spent)),
                )
            })
            .when_some(when, |d, w| {
                d.child(
                    div()
                        .flex_none()
                        .text_sm()
                        .text_color(if overdue { theme.danger } else { muted })
                        .child(w),
                )
            })
            .on_click(
                cx.listener(move |this, _, window, cx| {
                    this.select_task(id_open.clone(), window, cx)
                }),
            )
            .into_any_element()
    }

    /// The open task: title, when, important, tags, time (with the timer), the day it was done
    /// (back-dating), notes; Delete at the bottom.
    fn render_task_card(&mut self, t: &Task, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let day = today();
        let when = if t.evening && t.day == Some(day) {
            1
        } else if t.day == Some(day) {
            0
        } else if t.day == Some(day + chrono::Duration::days(1)) {
            2
        } else if t.day.is_none() {
            3
        } else {
            usize::MAX
        };
        let label = |text: &'static str| {
            div()
                .w(px(64.))
                .flex_none()
                .text_sm()
                .text_color(muted)
                .child(text)
        };
        let field = |name: &'static str, control: AnyElement| {
            h_flex()
                .gap_2()
                .items_center()
                .child(label(name))
                .child(div().flex_1().min_w_0().child(control))
                .into_any_element()
        };
        let id = t.id.clone();
        let (i1, i2, i3, i4, i5) = (id.clone(), id.clone(), id.clone(), id.clone(), id.clone());
        let armed = self.tasks.delete_armed.as_deref() == Some(id.as_str());
        let running = t.timer.is_some();
        let spent_now = t.spent_now();
        // In a thread: the thread's width, as tall as its content (the thread scrolls).
        v_flex()
            .id("task-card")
            .w_full()
            .px_6()
            .py_5()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.tasks.title)),
                    )
                    .child(
                        div()
                            .id("task-flag")
                            .p_1()
                            .cursor_pointer()
                            .text_color(if t.important { theme.danger } else { muted })
                            .hover(|s| s.bg(theme.list_hover))
                            .child(Ico::Flag.icon().small())
                            .tooltip(|window, cx| Tooltip::new("Important").build(window, cx))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_task_important(i1.clone(), cx)
                            })),
                    ),
            )
            .child(field(
                "When",
                segmented(
                    "task-when",
                    &["Today", "Evening", "Tomorrow", "Anytime"],
                    when,
                    cx,
                    move |this, k, window, cx| this.set_task_when(i2.clone(), k, window, cx),
                ),
            ))
            .child(field(
                "Day",
                Input::new(&self.tasks.day).small().into_any_element(),
            ))
            .child(field(
                "Tags",
                Input::new(&self.tasks.tags).small().into_any_element(),
            ))
            .child(field(
                "Time",
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.tasks.spent).small()),
                    )
                    .child(
                        div()
                            .id("task-timer")
                            .flex_none()
                            .px_2()
                            .py_0p5()
                            .gap_1()
                            .flex()
                            .items_center()
                            .text_sm()
                            .border_1()
                            .border_color(if running { theme.primary } else { theme.border })
                            .text_color(if running {
                                theme.primary
                            } else {
                                theme.foreground
                            })
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.list_hover))
                            .child(
                                if running { Ico::Square } else { Ico::Play }
                                    .icon()
                                    .xsmall(),
                            )
                            .child(if running {
                                tasks::show_spent(spent_now)
                            } else {
                                "Start".to_string()
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.toggle_task_timer(i3.clone(), window, cx)
                            })),
                    )
                    .into_any_element(),
            ))
            .when(t.done(), |d| {
                d.child(field(
                    "Done on",
                    Input::new(&self.tasks.completed).small().into_any_element(),
                ))
            })
            .when_some(self.tasks.field_error.clone(), |d, e| {
                d.child(div().text_sm().text_color(theme.danger).child(e))
            })
            .child(
                div()
                    .border_1()
                    .border_color(theme.border)
                    .px_1()
                    .child(Textarea::new(&self.tasks.notes).appearance(false)),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .id("task-done")
                            .px_2()
                            .py_0p5()
                            .text_sm()
                            .border_1()
                            .border_color(theme.border)
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.list_hover))
                            .child(if t.done() {
                                "Mark not done"
                            } else {
                                "Mark done  ⌘K"
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.toggle_task_done(i4.clone(), window, cx)
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("task-delete")
                            .px_2()
                            .py_0p5()
                            .text_sm()
                            .border_1()
                            .border_color(if armed { theme.danger } else { theme.border })
                            .text_color(if armed { theme.danger } else { muted })
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.list_hover))
                            .child(if armed { "Confirm delete" } else { "Delete" })
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.delete_task(i5.clone(), cx)),
                            ),
                    ),
            )
            .into_any_element()
    }
}
