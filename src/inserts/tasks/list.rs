//! The /tasks insert's lists, after Things: Today (with "This Evening"), Upcoming as the next
//! seven days then months, Anytime, the Logbook by day then month; the open list's tags as a
//! filter strip. A row is `row.rs`, the open task's card `card.rs`.

use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::inserts::tasks::model::{List, Task, today};
use bench_ui::icons::Ico;

impl BenchApp {
    /// A task list as a thread's live block: the same rows, no scroll of its own (the thread
    /// scrolls), the open task's card right under the list, the text at the thread's column.
    pub(crate) fn render_tasks_block(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.tasks_view(true, cx)
    }

    pub(crate) fn tasks_view(&mut self, inline: bool, cx: &mut Context<Self>) -> AnyElement {
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
                    .h(px(bench_ui::TASK_HEAD_H))
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
                .pl(px(bench_ui::TASK_INSET))
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
}
