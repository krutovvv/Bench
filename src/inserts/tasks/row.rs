//! One task in a list: its checkbox, title, tags, day and timer; a click opens its card.

use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::inserts::tasks::model::{self as tasks, List, Task, today};
use bench_ui::icons::Ico;

impl BenchApp {
    pub(crate) fn render_task_row(&mut self, n: usize, t: &Task, cx: &mut Context<Self>) -> AnyElement {
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
            .pl(px(bench_ui::TASK_INSET - bench_ui::ROW_STRIPE))
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
                    .size(px(bench_ui::CHECKBOX))
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
}
