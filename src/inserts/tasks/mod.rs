//! `/tasks`: the task lists in the thread, every list a click away (Today, Upcoming, Anytime,
//! Logbook…). In a project's thread the list opens filtered by the project's tag. Its state: {list}.

pub mod model;
pub mod state;
pub mod store;
pub mod view;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;
use serde_json::json;

use crate::app::BenchApp;
use crate::app::model::Insert;
use crate::inserts::InsertKind;
use crate::inserts::tasks::model::{LISTS, List};
use crate::thread::ThreadRef;

pub struct Tasks;

fn list_of(ins: &Insert) -> List {
    let name = ins.get("list");
    LISTS
        .iter()
        .find(|l| l.name().eq_ignore_ascii_case(&name))
        .cloned()
        .unwrap_or(LISTS[0].clone())
}

impl InsertKind for Tasks {
    fn owns(&self, key: &str) -> bool {
        key == "tasks"
    }

    fn word(&self) -> crate::inserts::Word {
        crate::inserts::Word {
            word: "tasks",
            aliases: &[],
            desc: "The task lists",
            key: "tasks",
        }
    }

    /// One row per list: the insert opens on it.
    fn choices(&self, _app: &BenchApp, _at: &ThreadRef) -> Vec<crate::app::model::SlashCommand> {
        LISTS
            .iter()
            .map(|l| crate::app::model::SlashCommand {
                name: format!("/tasks {}", l.name()),
                desc: "the task lists, on this one".into(),
                category: "Tool".into(),
                key: format!("tasks:{}", l.name().to_lowercase()),
            })
            .collect()
    }

    fn shared(&self) -> Option<&'static str> {
        Some("tasks")
    }

    fn title(&self, _app: &BenchApp, _at: &ThreadRef, _ins: &Insert) -> String {
        "Tasks".into()
    }

    fn controls(
        &self,
        app: &mut BenchApp,
        at: &ThreadRef,
        ins: &Insert,
        _window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Vec<AnyElement> {
        let now = list_of(ins);
        app.sync_task_project(at);
        // Only the lists that have something (and the one shown): an empty one needs no button.
        let lists: Vec<List> = LISTS
            .iter()
            .filter(|l| **l == now || !app.inserts.tasks.shown(l).is_empty())
            .cloned()
            .collect();
        let chosen = lists.iter().position(|l| *l == now).unwrap_or(0);
        let id = ins.id.clone();
        let theme = cx.theme().clone();
        let add = div()
            .id(SharedString::from(format!("{}-new-task", ins.id)))
            .px_1()
            .cursor_pointer()
            .text_color(theme.muted_foreground)
            .hover(|s| s.text_color(theme.foreground))
            .child("+ New task")
            .on_click(cx.listener(|this, _, window, cx| {
                this.click_claimed = true;
                this.new_task(window, cx)
            }))
            .into_any_element();
        let (picks, at) = (lists.clone(), at.clone());
        vec![
            add,
            crate::inserts::tools::view::segmented_owned(
                "insert-task-lists",
                lists.iter().map(|l| l.name()).collect(),
                chosen,
                cx,
                move |this, k, window, cx| {
                    let list = picks[k].clone();
                    this.patch_insert(
                        &at,
                        &id,
                        crate::inserts::state::new_state(json!({ "list": list.name().to_lowercase() })),
                        cx,
                    );
                    this.select_task_list(list, window, cx);
                },
            ),
        ]
    }

    fn body(
        &self,
        app: &mut BenchApp,
        at: &ThreadRef,
        ins: &Insert,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<AnyElement> {
        let list = list_of(ins);
        if app.inserts.tasks.list != list {
            app.select_task_list(list, window, cx);
        }
        app.sync_task_project(at);
        Some(app.render_tasks_block(cx))
    }

    /// The open task (a click on a row, or a new one), as the next row of the thread.
    fn follow(
        &self,
        app: &mut BenchApp,
        _at: &ThreadRef,
        ins: &Insert,
        _window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<AnyElement> {
        let task = app.inserts.tasks.selected_task().cloned()?;
        let theme = cx.theme().clone();
        let title = if task.title.trim().is_empty() {
            "New task".to_string()
        } else {
            task.title.clone()
        };
        let head = bench_ui::block_head(&theme, true)
            .child(div().flex_1().min_w_0().truncate().child(title))
            .child(
                bench_ui::head_icon_button(
                    &format!("{}-task-close", ins.id),
                    bench_ui::icons::Ico::X,
                    "Close",
                    cx,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.click_claimed = true;
                    this.close_task(window, cx);
                })),
            );
        let body = app.render_task_card(&task, cx);
        Some(crate::inserts::card(head, body, cx))
    }

    fn context(&self, _app: &BenchApp, _at: &ThreadRef, ins: &Insert, _cx: &App) -> String {
        format!("The tasks insert shows the {} list.", list_of(ins).name())
    }

    fn opened(
        &self,
        app: &mut BenchApp,
        _at: &ThreadRef,
        ins: &Insert,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) {
        app.start_task_store(cx);
        app.load_tasks();
        app.select_task_list(list_of(ins), window, cx);
    }
}
