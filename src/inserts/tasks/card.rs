//! The open task's card: its title and notes, one strip of quick controls (day, list, tags,
//! time spent), Delete.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, Textarea};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::inserts::tasks::model::{self as tasks, Task, today};
use bench_ui::icons::Ico;

impl BenchApp {
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
                            .w(px(bench_ui::TASK_DAY_W))
                            .flex_none()
                            .child(Input::new(&self.inserts.tasks.day)),
                    )
                    .when(t.done(), |d| {
                        d.child(
                            div()
                                .w(px(bench_ui::TASK_LIST_W))
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
                            .w(px(bench_ui::TASK_TIME_W))
                            .flex_none()
                            .child(Input::new(&self.inserts.tasks.tags)),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .gap_1()
                            .items_center()
                            .child(div().w(px(bench_ui::TASK_SPENT_W)).child(Input::new(&self.inserts.tasks.spent)))
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
