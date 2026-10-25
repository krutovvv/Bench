//! `/tasks`: the task lists in the thread, every list a click away (Today, Upcoming, Anytime,
//! Logbook…). In a project's thread the project is the tag: its tasks only. Its state: {list}.

use gpui_kit::*;
use serde_json::json;

use super::InsertKind;
use crate::app::BenchApp;
use crate::model::Insert;
use crate::tasks::{LISTS, List};

pub struct Tasks;

fn list_of(ins: &Insert) -> List {
    let name = ins.get("list");
    LISTS.iter().find(|l| l.name().eq_ignore_ascii_case(&name)).cloned().unwrap_or(LISTS[0].clone())
}

impl InsertKind for Tasks {
    fn owns(&self, key: &str) -> bool {
        key == "tasks"
    }

    fn title(&self, _app: &BenchApp, _ins: &Insert) -> String {
        "Tasks".into()
    }

    fn controls(&self, _app: &mut BenchApp, ins: &Insert, _window: &mut Window, cx: &mut Context<BenchApp>) -> Vec<AnyElement> {
        let now = list_of(ins);
        let chosen = LISTS.iter().position(|l| *l == now).unwrap_or(0);
        let id = ins.id.clone();
        vec![super::super::tools::segmented_owned(
            "insert-task-lists",
            LISTS.iter().map(|l| l.name()).collect(),
            chosen,
            cx,
            move |this, k, window, cx| {
                let list = LISTS[k].clone();
                this.patch_insert(&id, json!({ "state": { "list": list.name().to_lowercase() } }), cx);
                this.select_task_list(list, window, cx);
            },
        )]
    }

    fn body(&self, app: &mut BenchApp, ins: &Insert, window: &mut Window, cx: &mut Context<BenchApp>) -> Option<AnyElement> {
        let list = list_of(ins);
        if app.tasks.list != list {
            app.select_task_list(list, window, cx);
        }
        if app.tasks.project != app.thread_project {
            app.tasks.project = app.thread_project.clone();
        }
        Some(app.render_tasks_block(cx))
    }

    fn context(&self, _app: &BenchApp, ins: &Insert, _cx: &App) -> String {
        format!("The tasks insert shows the {} list.", list_of(ins).name())
    }

    fn opened(&self, app: &mut BenchApp, ins: &Insert, window: &mut Window, cx: &mut Context<BenchApp>) {
        app.start_reminders(cx);
        app.load_tasks();
        app.select_task_list(list_of(ins), window, cx);
    }
}
