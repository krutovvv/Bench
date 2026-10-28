//! The /tasks insert's views, after Things: the lists (Today, Upcoming, Anytime, Logbook); the
//! open one with its tags as a filter strip, Today with "This Evening", Upcoming as the next seven
//! days then months, the Logbook by day then month; the open task's card.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, Textarea};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::inserts::tasks::model::{self as tasks, List, Task, today};
use bench_ui::icons::Ico;

impl BenchApp {
    /// A task list as a thread's live block: the same rows, no scroll of its own (the thread
    /// scrolls), the open task's card right under the list, the text at the thread's column.
    pub(crate) fn render_tasks_block(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.tasks_view(true, cx)
    }

    fn tasks_view(&mut self, inline: bool, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let day = today();
        let list = self.inserts.tasks.list.clone();
        let shown = self.inserts.tasks.shown(&list);
        // Sections: (title, icon, tasks); an Upcoming day header is drawn big (number + name).
        let mut sections: Vec<(Option<String>, Option<Ico>, Vec<Task>)> = vec![];
        let month = |d: chrono::NaiveDate| {
            if chrono::Datelike::year(&d) == chrono::Datelike::year(&day) {
                d.format("%B").to_string()
            } else {
                d.format("%B %Y").to_string()
            }
        };
        if !self.inserts.tasks.query.is_empty() {
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
                    // The next seven days that have something ("Tomorrow · 26 Oct"), then by month.
                    for k in 1..=7 {
                        let d = day + chrono::Duration::days(k);
                        let name = if k == 1 {
                            "Tomorrow".to_string()
                        } else {
                            d.format("%A").to_string()
                        };
                        let label = format!("{name} · {}", d.format("%-d %b"));
                        let on: Vec<Task> =
                            shown.iter().filter(|t| t.day == Some(d)).cloned().collect();
                        if !on.is_empty() {
                            sections.push((Some(label), None, on));
                        }
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
        let empty = sections.iter().all(|(_, _, v)| v.is_empty());
        let mut rows: Vec<AnyElement> = vec![];
        let mut n = 0;
        for (title, icon, list_tasks) in sections {
            // A section's name: one quiet line over its tasks, no rule (the first one closer).
            if let Some(title) = title {
                rows.push(
                    h_flex()
                        .flex_none()
                        .px_6()
                        .pt(px(if rows.is_empty() { 4. } else { 14. }))
                        .pb_1()
                        .gap_1p5()
                        .items_center()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(muted)
                        .when_some(icon, |d, i| d.child(i.icon().xsmall()))
                        .child(title)
                        .into_any_element(),
                );
            }
            for t in list_tasks {
                rows.push(self.render_task_row(n, &t, cx));
                n += 1;
                // The open task's card right under its row, whatever the list (the Logbook's
                // thousand rows would push a card at the end out of sight).
                if !inline && self.inserts.tasks.selected.as_deref() == Some(t.id.as_str()) {
                    if let Some(open) = self.inserts.tasks.selected_task().cloned() {
                        let card = self.render_task_card(&open, cx);
                        rows.push(
                            div()
                                .flex_none()
                                .mx_6()
                                .my_1()
                                .border_1()
                                .rounded_md()
                                .border_color(theme.border)
                                .child(card)
                                .into_any_element(),
                        );
                    }
                }
            }
        }
        let note = if let Some(e) = &self.inserts.tasks.error {
            Some((e.clone(), theme.danger))
        } else if !self.inserts.tasks.loaded {
            Some(("Loading tasks…".to_string(), muted))
        } else if empty {
            Some((
                if self.inserts.tasks.query.is_empty() {
                    "Nothing here".to_string()
                } else {
                    "No task matches".to_string()
                },
                muted,
            ))
        } else {
            None
        };
        // The list's tags, the most used first: "All · work · client-b…". A project's thread
        // opens on its project's tag (`sync_task_project`); "All" drops it.
        let mut tags = if self.inserts.tasks.query.is_empty() {
            self.inserts.tasks.tags_in(&list)
        } else {
            vec![]
        };
        let picked = self.inserts.tasks.tag_filter.clone();
        if let Some(p) = &picked {
            if !tags.iter().any(|g| g.eq_ignore_ascii_case(p)) {
                tags.insert(0, p.clone());
            }
        }
        let strip = (!tags.is_empty()).then(|| {
            let chip = |id: SharedString,
                        label: String,
                        on: bool,
                        tag: Option<String>,
                        cx: &mut Context<Self>| {
                div()
                    .id(id)
                    .flex_none()
                    .h(px(22.))
                    .px_2()
                    .flex()
                    .items_center()
                    .rounded_md()
                    .border_1()
                    .text_sm()
                    .cursor_pointer()
                    .when(on, |d| {
                        d.border_color(theme.primary).text_color(theme.primary)
                    })
                    .when(!on, |d| {
                        d.border_color(theme.border)
                            .text_color(muted)
                            .hover(|s| s.text_color(theme.foreground))
                    })
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.click_claimed = true;
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
                .pl(px(24.))
                .pr_6()
                .py_1p5()
                .flex_wrap()
                .gap_2()
                .items_center()
                .children(chips)
                .into_any_element()
        });
        if inline {
            // In an insert: its head switches the lists (no heading here); the rows are pulled
            // close to the insert's edges but keep a small gutter (4 + 12 wrapper = 16 px).
            // No w_full: stretched, the negative margins widen it toward the insert's edges.
            return v_flex()
                .mx(px(-4.))
                .children(strip)
                .child(v_flex().children(rows).when_some(note, |d, (text, color)| {
                    // One line, as an empty terminal is.
                    d.child(div().px_6().text_base().text_color(color).child(text))
                }))
                .into_any_element();
        }
        let list_pane = v_flex().flex_1().min_w_0().h_full().children(strip).child(
            crate::inserts::scroll_pane(
                "tasks-rows",
                &self.inserts.tasks.rows_scroll,
                v_flex()
                    .pt_1()
                    .pb_4()
                    .children(rows)
                    .when_some(note, |d, (text, color)| {
                        d.child(
                            div()
                                .px_6()
                                .py_3()
                                .text_base()
                                .text_color(color)
                                .child(text),
                        )
                    }),
            ),
        );
        h_flex()
            .size_full()
            .items_stretch()
            .child(list_pane)
            .into_any_element()
    }

    fn render_task_row(&mut self, n: usize, t: &Task, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let day = today();
        let selected = self.inserts.tasks.selected.as_deref() == Some(t.id.as_str());
        let done = t.done();
        let (id_check, id_open) = (t.id.clone(), t.id.clone());
        let overdue = !done && t.day.is_some_and(|d| d < day);
        // The day, where the list doesn't say it already.
        let logbook = self.inserts.tasks.list == List::Logbook && self.inserts.tasks.query.is_empty();
        let when = match (&self.inserts.tasks.list, t.day, t.completed) {
            (List::Logbook, _, _) if logbook => None,
            (_, _, Some(d)) => Some(tasks::show_date(d, day)),
            (List::Today, Some(d), _) if overdue => Some(tasks::show_date(d, day)),
            (List::Today | List::Upcoming, _, _) if self.inserts.tasks.query.is_empty() => None,
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
                    .rounded_md()
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
            // Its tags (a project's is one of them).
            .children(t.tags.iter().take(3).map(|g| {
                div()
                    .flex_none()
                    .px_1p5()
                    .text_sm()
                    .text_color(muted)
                    .border_1()
                    .rounded_md()
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

    /// The open task, Things-style: the title, the notes, and one thin strip of quick controls
    /// under them — not a stack of labelled form rows.
    pub(crate) fn render_task_card(&mut self, t: &Task, cx: &mut Context<Self>) -> AnyElement {
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
        let id = t.id.clone();
        let view = cx.entity();
        let (i1, i2, i3, i4, i5) = (id.clone(), id.clone(), id.clone(), id.clone(), id.clone());
        let armed = self.armed.is(&format!("task:{id}"));
        let running = t.timer.is_some();
        let spent_now = t.spent_now();
        // In a thread: the thread's width, as tall as its content (the thread scrolls).
        v_flex()
            .id("task-card")
            .w_full()
            .px_4()
            .py_3()
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.inserts.tasks.title)),
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
            // The notes are the body of the card: no frame, the field grows with them.
            .child(
                div()
                    .px_1()
                    .child(Textarea::new(&self.inserts.tasks.notes).appearance(false)),
            )
            .when_some(self.inserts.tasks.field_error.clone(), |d, e| {
                d.child(div().px_1().text_sm().text_color(theme.danger).child(e))
            })
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .items_center()
                    .border_t_1()
                    .border_color(theme.border)
                    .pt_2()
                    // "When": one button that says it with the four answers in its menu —
                    // the strip of four labels beside a date field took half the row before,
                    // and pushed everything after it onto a second line.
                    .child({
                        let label = if when == usize::MAX {
                            t.day
                                .map(|d| tasks::show_date(d, day))
                                .unwrap_or_else(|| "Anytime".into())
                        } else {
                            ["Today", "This evening", "Tomorrow", "Anytime"][when.min(3)]
                                .to_string()
                        };
                        let (app, when_id) = (view.clone(), i2.clone());
                        Button::new("task-when")
                            .ghost()
                            .small()
                            .icon(Ico::CalendarDays.icon())
                            .label(format!("{label}  ▾"))
                            .dropdown_menu(move |mut menu, _, _| {
                                for (k, name) in [
                                    (0, "Today"),
                                    (1, "This evening"),
                                    (2, "Tomorrow"),
                                    (3, "Anytime"),
                                ] {
                                    let (app, id) = (app.clone(), when_id.clone());
                                    menu = menu.item(
                                        PopupMenuItem::new(name).checked(when == k).on_click(
                                            move |_, window, cx| {
                                                app.update(cx, |this, cx| {
                                                    this.set_task_when(id.clone(), k, window, cx)
                                                });
                                            },
                                        ),
                                    );
                                }
                                menu
                            })
                    })
                    .child(
                        div()
                            .w(px(110.))
                            .flex_none()
                            .child(Input::new(&self.inserts.tasks.day)),
                    )
                    .when(t.done(), |d| {
                        d.child(
                            div()
                                .w(px(120.))
                                .flex_none()
                                .child(Input::new(&self.inserts.tasks.completed)),
                        )
                    })
                    // The tags: the projects in a menu, ticked what is on the task; the field
                    // next to it adds a tag no project has. Chips for every project ate the row.
                    .child({
                        let lower: Vec<String> = t.tags.iter().map(|g| g.to_lowercase()).collect();
                        let mut items: Vec<(String, bool)> = self
                            .places.projects
                            .iter()
                            .map(|p| tasks::slug(&p.name))
                            .filter(|s| !s.is_empty())
                            .map(|tag| {
                                let chosen = lower.contains(&tag.to_lowercase());
                                (tag, chosen)
                            })
                            .collect();
                        // Tags of the task that are no project's ride the menu too, ticked.
                        let own: Vec<String> = t
                            .tags
                            .iter()
                            .filter(|g| !items.iter().any(|(s, _)| s.eq_ignore_ascii_case(g)))
                            .cloned()
                            .collect();
                        items.extend(own.into_iter().map(|g| (g, true)));
                        let (app, empty) = (view.clone(), lower.is_empty());
                        Button::new("task-tags")
                            .ghost()
                            .small()
                            .icon(Ico::Tag.icon())
                            .label(if empty {
                                "Tags  ▾".to_string()
                            } else {
                                format!("Tags · {}  ▾", lower.len())
                            })
                            .dropdown_menu(move |mut menu, _, _| {
                                if items.is_empty() {
                                    menu = menu
                                        .label("No tags yet — type one in the field")
                                        .separator();
                                }
                                for (tag, chosen) in items.iter() {
                                    let app = app.clone();
                                    let tag = tag.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(tag.clone()).checked(*chosen).on_click(
                                            move |_, window, cx| {
                                                app.update(cx, |this, cx| {
                                                    this.toggle_task_tag(tag.clone(), window, cx)
                                                });
                                            },
                                        ),
                                    );
                                }
                                let app = app.clone();
                                menu.separator()
                                    .item(PopupMenuItem::new("Clear tags").on_click(
                                        move |_, window, cx| {
                                            app.update(cx, |this, cx| {
                                                this.clear_task_tags(window, cx)
                                            });
                                        },
                                    ))
                            })
                    })
                    .child(
                        div()
                            .w(px(100.))
                            .flex_none()
                            .child(Input::new(&self.inserts.tasks.tags)),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .gap_1()
                            .items_center()
                            .child(div().w(px(64.)).child(Input::new(&self.inserts.tasks.spent)))
                            .child(
                                div()
                                    .id("task-timer")
                                    .flex_none()
                                    .h(px(bench_ui::FIELD_H))
                                    .px_2()
                                    .gap_1()
                                    .flex()
                                    .items_center()
                                    .text_xs()
                                    .border_1()
                                    .rounded_md()
                                    .border_color(if running {
                                        theme.primary
                                    } else {
                                        theme.border
                                    })
                                    .text_color(if running { theme.primary } else { muted })
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.list_hover))
                                    .child(
                                        if running { Ico::Square } else { Ico::Play }
                                            .icon()
                                            .xsmall(),
                                    )
                                    .when(running, |d| d.child(tasks::show_spent(spent_now)))
                                    .tooltip(|window, cx| {
                                        Tooltip::new("Start / stop the timer").build(window, cx)
                                    })
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.toggle_task_timer(i3.clone(), window, cx)
                                    })),
                            ),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("task-done")
                            .h(px(bench_ui::FIELD_H))
                            .px_2()
                            .flex()
                            .items_center()
                            .text_sm()
                            .border_1()
                            .rounded_md()
                            .border_color(theme.border)
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.list_hover))
                            .child(if t.done() { "Not done" } else { "Done  ⌘K" })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.toggle_task_done(i4.clone(), window, cx)
                            })),
                    )
                    .child(
                        div()
                            .id("task-delete")
                            .h(px(bench_ui::FIELD_H))
                            .px_2()
                            .flex()
                            .items_center()
                            .text_xs()
                            .rounded_md()
                            .cursor_pointer()
                            .text_color(if armed { theme.danger } else { muted })
                            .hover(|s| s.bg(theme.list_hover).text_color(theme.danger))
                            .when(armed, |d| d.child("Confirm delete"))
                            .when(!armed, |d| {
                                d.child(Ico::Trash.icon().small())
                                    .tooltip(|window, cx| Tooltip::new("Delete").build(window, cx))
                            })
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.delete_task(i5.clone(), cx)),
                            ),
                    ),
            )
            .into_any_element()
    }
}
