//! Tasks mode (⌘4, the list icon in the sidebar's top-right corner): tasks in Bench's own store (`crate::task_store`, the agent's bench.sqlite; moved once from Apple Reminders) shown the way Things shows
//! to-dos — Today (with This Evening), Upcoming, Anytime, Logbook and tags in the
//! sidebar, the tasks in the middle, the open task's card on the right. Every change goes to
//! Reminders (`crate::reminders`); what another device or the agent changes there shows up here.

use web_time::Instant;

use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::*;

use super::BenchApp;
use crate::reminders::{Cmd, Evt};
use crate::task_store::TaskStore;
use crate::tasks::{self, List, Task, today};

/// How far back the Logbook (and a tag's done tasks) reaches.
pub const RANGES: [(&str, i64); 5] = [
    ("7 days", 7),
    ("30 days", 30),
    ("90 days", 90),
    ("1 year", 365),
    ("All", 36500),
];

pub struct TasksMode {
    pub on: bool,
    pub list: List,
    /// The open thread's project ("": Host). Its tag (`tasks::slug` of its name) is the list's
    /// filter when the thread opens, one click to drop; new tasks get the filter's tag.
    pub project: String,
    pub items: Vec<Task>,
    pub loaded: bool,
    pub error: Option<String>,
    pub selected: Option<String>,
    pub range: usize,
    /// The tag picked in the strip under the header ("All" = none).
    pub tag_filter: Option<String>,
    pub search: Entity<InputState>,
    pub query: String,
    // The open task's card.
    pub title: Entity<InputState>,
    pub notes: Entity<TextareaState>,
    pub tags: Entity<InputState>,
    pub day: Entity<InputState>,
    pub spent: Entity<InputState>,
    pub completed: Entity<InputState>,
    /// A field that did not parse ("Date: next blursday"), shown under the card's fields.
    pub field_error: Option<String>,
    /// The card was edited: saved shortly after the typing stops.
    dirty: Option<Instant>,
    /// Delete was pressed once for this task.
    pub delete_armed: Option<String>,
    pub(crate) reminders: Option<TaskStore>,
    last_load: Option<Instant>,
    next_token: u64,
    _subscriptions: Vec<Subscription>,
}

impl TasksMode {
    pub fn new(window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let input =
            |placeholder: &'static str, window: &mut Window, cx: &mut Context<BenchApp>| {
                cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
            };
        let search = input("Search tasks", window, cx);
        let title = input("Task", window, cx);
        let tags = input("work, client-x", window, cx);
        let day = input("today, fri, +3, 25.10", window, cx);
        let spent = input("1h 30m", window, cx);
        let completed = input("today, yesterday, 21.10", window, cx);
        let notes = cx.new(|cx| {
            TextareaState::new(window, cx)
                // Grows with the notes: the thread scrolls, not the field.
                .auto_grow(4, 100_000)
                .placeholder("Notes")
        });
        let mut subscriptions =
            vec![
                cx.subscribe_in(&search, window, |this, state, ev: &InputEvent, _, cx| {
                    if let InputEvent::Change = ev {
                        this.tasks.query = state.read(cx).value().trim().to_lowercase();
                        cx.notify();
                    }
                }),
            ];
        for field in [&title, &tags, &day, &spent, &completed] {
            subscriptions.push(cx.subscribe_in(
                field,
                window,
                |this, _, ev: &InputEvent, _, cx| {
                    if let InputEvent::Change = ev {
                        this.task_fields_changed(cx);
                    }
                },
            ));
        }
        subscriptions.push(
            cx.subscribe_in(&notes, window, |this, _, ev: &InputEvent, _, cx| {
                if let InputEvent::Change = ev {
                    this.task_fields_changed(cx);
                }
            }),
        );
        Self {
            on: false,
            list: List::Today,
            items: vec![],
            loaded: false,
            error: None,
            selected: None,
            range: 1,
            tag_filter: None,
            project: String::new(),
            search,
            query: String::new(),
            title,
            notes,
            tags,
            day,
            spent,
            completed,
            field_error: None,
            dirty: None,
            delete_armed: None,
            reminders: None,
            last_load: None,
            next_token: 1,
            _subscriptions: subscriptions,
        }
    }

    pub fn selected_task(&self) -> Option<&Task> {
        let id = self.selected.as_ref()?;
        self.items.iter().find(|t| &t.id == id)
    }

    fn selected_mut(&mut self) -> Option<&mut Task> {
        let id = self.selected.clone()?;
        self.items.iter_mut().find(|t| t.id == id)
    }

    fn done_since(&self) -> chrono::NaiveDate {
        today() - chrono::Duration::days(RANGES[self.range].1)
    }

    /// The tasks a list shows, in its order (Logbook: newest done first).
    pub fn shown(&self, list: &List) -> Vec<Task> {
        let day = today();
        let q = &self.query;
        let mut out: Vec<Task> = self
            .items
            .iter()
            .filter(|t| {
                if q.is_empty() {
                    t.in_list(list, day)
                        && self
                            .tag_filter
                            .as_ref()
                            .is_none_or(|f| t.tags.iter().any(|g| g.eq_ignore_ascii_case(f)))
                } else {
                    t.title.to_lowercase().contains(q)
                        || t.notes.to_lowercase().contains(q)
                        || t.tags.iter().any(|g| g.to_lowercase().contains(q))
                }
            })
            .cloned()
            .collect();
        match list {
            List::Logbook => out.sort_by(|a, b| b.completed.cmp(&a.completed)),
            List::Upcoming => out.sort_by_key(|t| (t.day, t.created)),
            _ => out.sort_by_key(|t| (t.done(), t.created)),
        }
        out
    }

    /// The tags of a list's tasks, the most used first (the strip over the list).
    pub fn tags_in(&self, list: &List) -> Vec<String> {
        let day = today();
        let mut counts: Vec<(String, usize)> = vec![];
        for t in self.items.iter().filter(|t| t.in_list(list, day)) {
            for g in &t.tags {
                match counts.iter_mut().find(|(n, _)| n.eq_ignore_ascii_case(g)) {
                    Some((_, c)) => *c += 1,
                    None => counts.push((g.clone(), 1)),
                }
            }
        }
        counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.to_lowercase().cmp(&b.0.to_lowercase())));
        counts.into_iter().map(|(n, _)| n).collect()
    }

    fn send(&self, cmd: Cmd) {
        if let Some(r) = &self.reminders {
            r.send(cmd);
        }
    }
}

impl BenchApp {
    /// The Reminders thread, started the first time Tasks opens (macOS asks for access once).
    pub(crate) fn start_reminders(&mut self, cx: &mut Context<Self>) {
        // The demo's tasks are made up; Reminders stays untouched.
        if self.tasks.reminders.is_some() || self.demo {
            return;
        }
        // The tasks live in the agent's store: wait for the connection.
        let Some(conn) = self.conn.clone() else {
            return;
        };
        let (tx, rx) = async_channel::unbounded();
        self.tasks.reminders = Some(TaskStore::start(conn, self.current_profile(), tx));
        cx.spawn(async move |this, cx| {
            while let Ok(evt) = rx.recv().await {
                if this
                    .update(cx, |this, cx| this.on_reminders(evt, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    fn on_reminders(&mut self, evt: Evt, cx: &mut Context<Self>) {
        match evt {
            Evt::Loaded(items) => {
                // A card being typed into keeps its own copy until it is saved.
                let editing = self
                    .tasks
                    .dirty
                    .is_some()
                    .then(|| self.tasks.selected_task().cloned())
                    .flatten();
                // A new task not yet in Reminders (nothing typed, or its save still on the way)
                // only exists here: a re-read must not drop it, or its open card closes.
                let pending: Vec<Task> = self
                    .tasks
                    .items
                    .iter()
                    .filter(|t| t.id.starts_with("new:"))
                    .cloned()
                    .collect();
                self.tasks.items = items;
                self.tasks.items.extend(pending);
                if let Some(t) = editing {
                    if let Some(slot) = self.tasks.items.iter_mut().find(|x| x.id == t.id) {
                        *slot = t;
                    }
                }
                self.tasks.loaded = true;
                self.tasks.error = None;
            }
            Evt::Created { token, id } => {
                let temp = format!("new:{token}");
                if let Some(t) = self.tasks.items.iter_mut().find(|t| t.id == temp) {
                    t.id = id.clone();
                }
                if self.tasks.selected.as_deref() == Some(temp.as_str()) {
                    self.tasks.selected = Some(id);
                }
                self.load_tasks();
            }
            Evt::Error(e) => self.tasks.error = Some(e),
        }
        cx.notify();
    }

    pub fn load_tasks(&mut self) {
        self.tasks.last_load = Some(Instant::now());
        let since = self.tasks.done_since();
        self.tasks.send(Cmd::Load { done_since: since });
    }

    /// From the window's ticker: save a card whose typing stopped, re-read Reminders now and
    /// then (another device, the agent), keep a running timer's figure moving.
    pub fn tick_tasks(&mut self, cx: &mut Context<Self>) {
        if self
            .tasks
            .dirty
            .is_some_and(|t| t.elapsed().as_millis() > 700)
        {
            self.tasks.dirty = None;
            self.save_selected();
        }
        if !self.tasks.on || self.tasks.reminders.is_none() {
            return;
        }
        if self.tasks.dirty.is_none()
            && self
                .tasks
                .last_load
                .is_none_or(|t| t.elapsed().as_secs() >= 4)
        {
            self.load_tasks();
        }
        if self.tasks.items.iter().any(|t| t.timer.is_some()) {
            cx.notify();
        }
    }

    fn save_selected(&mut self) {
        if let Some(t) = self.tasks.selected_task().cloned() {
            self.save_task(t);
        }
    }

    fn save_task(&mut self, task: Task) {
        let token = if task.id.starts_with("new:") {
            task.id[4..].parse().unwrap_or(0)
        } else {
            0
        };
        let mut t = task;
        if token != 0 {
            t.id = String::new();
        }
        self.tasks.send(Cmd::Save { task: t, token });
        self.tasks.last_load = Some(Instant::now());
    }

    /// Change one task (in place, then in Reminders).
    fn change_task(&mut self, id: &str, f: impl FnOnce(&mut Task), cx: &mut Context<Self>) {
        let Some(t) = self.tasks.items.iter_mut().find(|t| t.id == id) else {
            return;
        };
        f(t);
        let t = t.clone();
        if self.tasks.selected.as_deref() == Some(id) {
            self.tasks.dirty = None;
        }
        self.save_task(t);
        cx.notify();
    }

    /// A new connection (another machine, or the agent restarted): read its tasks.
    pub fn restart_tasks(&mut self, cx: &mut Context<Self>) {
        self.tasks.reminders = None;
        self.tasks.items.clear();
        self.tasks.loaded = false;
        self.start_reminders(cx);
        self.load_tasks();
    }

    /// What the open thread is about: its session's subject, or the new chat's.
    pub fn current_subject(&self) -> String {
        match &self.session {
            Some(id) => self
                .sessions
                .iter()
                .find(|s| &s.id == id)
                .map(|s| s.subject.clone())
                .unwrap_or_default(),
            None => self.new_subject.clone(),
        }
    }

    /// The task lists in a new thread, on `list`.
    pub fn open_task_list(&mut self, list: List, window: &mut Window, cx: &mut Context<Self>) {
        self.new_thread_with(format!("tasks:{}", list.name().to_lowercase()), window, cx);
    }

    pub fn select_task_list(&mut self, list: List, window: &mut Window, cx: &mut Context<Self>) {
        self.flush_task(cx);
        self.tasks.list = list;
        self.tasks.tag_filter = None;
        self.tasks.selected = None;
        self.drop_blank_new();
        self.tasks
            .search
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.tasks.query.clear();
        cx.notify();
    }

    pub fn set_task_range(&mut self, k: usize, cx: &mut Context<Self>) {
        self.tasks.range = k.min(RANGES.len() - 1);
        self.load_tasks();
        cx.notify();
    }

    /// Save the open card now, if it was edited.
    fn flush_task(&mut self, _cx: &mut Context<Self>) {
        if self.tasks.dirty.take().is_some() {
            self.save_selected();
        }
    }

    /// A new task left without a title was never written anywhere: leaving it drops it.
    fn drop_blank_new(&mut self) {
        let open = self.tasks.selected.clone();
        self.tasks.items.retain(|t| {
            !(t.id.starts_with("new:") && t.title.trim().is_empty() && open.as_ref() != Some(&t.id))
        });
    }

    /// The open card closes (saved first).
    pub fn close_task(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.flush_task(cx);
        self.tasks.selected = None;
        self.drop_blank_new();
        cx.notify();
    }

    pub fn select_task(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.flush_task(cx);
        self.tasks.selected = Some(id);
        self.drop_blank_new();
        self.tasks.delete_armed = None;
        self.fill_task_card(window, cx);
        cx.notify();
    }

    /// The card's fields from the open task (setting them does not count as editing).
    fn fill_task_card(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(t) = self.tasks.selected_task().cloned() else {
            return;
        };
        let day = today();
        let m = &self.tasks;
        m.title
            .update(cx, |s, cx| s.set_value(t.title.clone(), window, cx));
        m.notes
            .update(cx, |s, cx| s.set_value(t.notes.clone(), window, cx));
        m.tags
            .update(cx, |s, cx| s.set_value(t.tags.join(", "), window, cx));
        let d = t
            .day
            .map(|d| tasks::show_date(d, day).to_lowercase())
            .unwrap_or_default();
        m.day.update(cx, |s, cx| s.set_value(d, window, cx));
        let spent = if t.spent > 0 {
            tasks::show_spent(t.spent)
        } else {
            String::new()
        };
        m.spent.update(cx, |s, cx| s.set_value(spent, window, cx));
        let c = t
            .completed
            .map(|d| d.format("%d.%m.%Y").to_string())
            .unwrap_or_default();
        m.completed.update(cx, |s, cx| s.set_value(c, window, cx));
        self.tasks.field_error = None;
    }

    /// The card was typed into: take its fields into the task; saved once the typing stops.
    fn task_fields_changed(&mut self, cx: &mut Context<Self>) {
        let day = today();
        let m = &self.tasks;
        let title = m.title.read(cx).value().to_string();
        let notes = m.notes.read(cx).value().to_string();
        let tags = tasks::parse_tags(&m.tags.read(cx).value());
        let day_text = m.day.read(cx).value().to_string();
        let spent_text = m.spent.read(cx).value().to_string();
        let done_text = m.completed.read(cx).value().to_string();
        let mut error = None;
        let Some(t) = self.tasks.selected_mut() else {
            return;
        };
        t.title = title;
        t.notes = notes;
        t.tags = tags;
        match tasks::parse_date(&day_text, day) {
            Some(d) => t.day = d,
            None => error = Some(format!("Date: “{}” is not a day", day_text.trim())),
        }
        match tasks::parse_spent(&spent_text) {
            Some(s) => t.spent = s,
            None => {
                error = Some(format!(
                    "Time: “{}” — try 1h 30m, 90m or 1:30",
                    spent_text.trim()
                ))
            }
        }
        if t.done() {
            match tasks::parse_date(&done_text, day) {
                Some(Some(d)) => t.completed = Some(d),
                Some(None) => {}
                None => error = Some(format!("Done on: “{}” is not a day", done_text.trim())),
            }
        }
        self.tasks.field_error = error;
        self.tasks.dirty = Some(Instant::now());
        cx.notify();
    }

    /// A new task in the open list, its card open with the title field focused.
    /// A project's tag (`tasks::slug` of its name); None for Host.
    pub fn project_tag(&self, project: &str) -> Option<String> {
        self.projects.iter().find(|p| p.id == project).map(|p| tasks::slug(&p.name)).filter(|s| !s.is_empty())
    }

    /// The tasks follow the open thread: in a project its tag filters the list (dropped with a
    /// click on "All"); in Host every task shows.
    pub fn sync_task_project(&mut self) {
        if self.tasks.project != self.thread_project {
            self.tasks.project = self.thread_project.clone();
            self.tasks.tag_filter = self.project_tag(&self.thread_project);
        }
    }

    pub fn set_task_tag_filter(&mut self, tag: Option<String>, cx: &mut Context<Self>) {
        self.tasks.tag_filter = tag;
        cx.notify();
    }

    /// The card's project chips: the project's tag on the open task, or off it again.
    pub fn toggle_task_tag(&mut self, tag: String, window: &mut Window, cx: &mut Context<Self>) {
        let mut tags = tasks::parse_tags(&self.tasks.tags.read(cx).value());
        match tags.iter().position(|g| g.eq_ignore_ascii_case(&tag)) {
            Some(i) => {
                tags.remove(i);
            }
            None => tags.push(tag),
        }
        self.tasks.tags.update(cx, |s, cx| s.set_value(tags.join(", "), window, cx));
        self.task_fields_changed(cx);
        cx.notify();
    }

    pub fn new_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.flush_task(cx);
        let token = self.tasks.next_token;
        self.tasks.next_token += 1;
        let mut t = Task {
            id: format!("new:{token}"),
            created: tasks::now(),
            ..Default::default()
        };
        let list = if self.tasks.list == List::Logbook {
            List::Anytime
        } else {
            self.tasks.list.clone()
        };
        list.place(&mut t, today());
        if let Some(tag) = self.tasks.tag_filter.clone() {
            t.tags = vec![tag];
        }
        self.tasks.items.push(t.clone());
        self.tasks.selected = Some(t.id.clone());
        self.drop_blank_new();
        self.fill_task_card(window, cx);
        self.tasks.title.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    /// Done ↔ not done (done today; the card can move the day back).
    pub fn toggle_task_done(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.change_task(
            &id,
            |t| {
                t.completed = if t.done() { None } else { Some(today()) };
                if let Some(start) = t.timer.take() {
                    t.spent += (tasks::now() - start).max(0);
                }
            },
            cx,
        );
        if self.tasks.selected.as_deref() == Some(id.as_str()) {
            self.fill_task_card(window, cx);
        }
    }

    pub fn toggle_task_important(&mut self, id: String, cx: &mut Context<Self>) {
        self.change_task(&id, |t| t.important = !t.important, cx);
    }

    /// "When": 0 Today, 1 This Evening, 2 Tomorrow, 3 Anytime.
    pub fn set_task_when(
        &mut self,
        id: String,
        when: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let day = today();
        self.change_task(
            &id,
            |t| {
                t.someday = false;
                t.evening = when == 1;
                t.day = match when {
                    0 | 1 => Some(day),
                    2 => Some(day + chrono::Duration::days(1)),
                    _ => None,
                };
            },
            cx,
        );
        if self.tasks.selected.as_deref() == Some(id.as_str()) {
            self.fill_task_card(window, cx);
        }
    }

    /// One timer at a time: starting one stops the others (their time is kept).
    pub fn toggle_task_timer(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let running: Vec<String> = self
            .tasks
            .items
            .iter()
            .filter(|t| t.timer.is_some() && t.id != id)
            .map(|t| t.id.clone())
            .collect();
        for other in running {
            self.change_task(
                &other,
                |t| {
                    if let Some(start) = t.timer.take() {
                        t.spent += (tasks::now() - start).max(0);
                    }
                },
                cx,
            );
        }
        self.change_task(
            &id,
            |t| match t.timer.take() {
                Some(start) => t.spent += (tasks::now() - start).max(0),
                None => t.timer = Some(tasks::now()),
            },
            cx,
        );
        if self.tasks.selected.as_deref() == Some(id.as_str()) {
            self.fill_task_card(window, cx);
        }
    }

    /// First press arms, the second removes the task from Reminders.
    pub fn delete_task(&mut self, id: String, cx: &mut Context<Self>) {
        if self.tasks.delete_armed.as_deref() != Some(id.as_str()) {
            self.tasks.delete_armed = Some(id);
            cx.notify();
            return;
        }
        self.tasks.delete_armed = None;
        self.tasks.items.retain(|t| t.id != id);
        if self.tasks.selected.as_deref() == Some(id.as_str()) {
            self.tasks.selected = None;
            self.tasks.dirty = None;
        }
        if !id.starts_with("new:") {
            self.tasks.send(Cmd::Delete(id));
        }
        cx.notify();
    }

}
