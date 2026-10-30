//! The project's schedule: jobs that run a prompt in a new thread of the project (the agent's
//! `cron_jobs`). A row each, "+ New job" in the head; a card edits the job, runs or pauses it.

use gpui_kit::component::input::{EditorState, Input, InputState};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::*;
use serde_json::{Value, json};

use super::section::{
    Card, ExplorerParts, PartRow, SaveCall, Section, card_act, code, head_act, pad, remove, revert,
    save, when,
};
use super::{ProjectEnv, editor, enc, on_change};
use crate::app::BenchApp;
use crate::client::rest;

/// A scheduled job of the project (the agent's `cron_jobs`): it runs `prompt` in a new thread of
/// the project on `schedule` ("every 1d", "0 9 * * 1-5", an ISO time for once).
#[derive(Clone, Debug, Default)]
pub struct CronJob {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub schedule: String,
    pub display: String,
    pub enabled: bool,
    pub next_run: Option<String>,
    pub last_run: Option<String>,
    pub last_status: Option<String>,
    pub last_output: String,
}

pub struct ScheduleEnv {
    pub jobs: Vec<CronJob>,
    /// The job opened under its row ("new": a new one's form) and its fields.
    pub open: Option<String>,
    pub name: Entity<InputState>,
    pub schedule: Entity<InputState>,
    pub prompt: Entity<EditorState>,
    pub dirty: bool,
}

impl ScheduleEnv {
    pub(super) fn new(id: &str, window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let prompt = editor("markdown", window, cx);
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Its name: Weekly digest"));
        let schedule = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("every 1d · every 30m · 0 9 * * 1-5 · 2026-11-01T09:00")
        });
        on_change(&prompt, id, cx, |e| e.cron.dirty = true);
        on_change(&name, id, cx, |e| e.cron.dirty = true);
        on_change(&schedule, id, cx, |e| e.cron.dirty = true);
        Self {
            jobs: vec![],
            open: None,
            name,
            schedule,
            prompt,
            dirty: false,
        }
    }
}

pub struct Schedule;

impl Section for Schedule {
    fn key(&self) -> &'static str {
        "cron"
    }

    fn apply(&self, e: &mut ProjectEnv, v: &Value, _window: &mut Window, _cx: &mut App) {
        e.cron.jobs = v["cron"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|j| {
                let s = |k: &str| j[k].as_str().unwrap_or_default().to_string();
                let o = |k: &str| j[k].as_str().filter(|s| !s.is_empty()).map(str::to_string);
                CronJob {
                    id: s("id"),
                    name: s("name"),
                    prompt: s("prompt"),
                    schedule: j["schedule"]["expr"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    display: s("schedule_display"),
                    enabled: j["enabled"].as_bool().unwrap_or(true),
                    next_run: o("next_run_at"),
                    last_run: o("last_run_at"),
                    last_status: o("last_status"),
                    last_output: s("last_output"),
                }
            })
            .collect();
    }

    /// A new job is POSTed to the project; a job's change is PUT to it.
    fn save(
        &self,
        app: &mut BenchApp,
        id: &str,
        eid: &str,
        _window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<SaveCall> {
        let e = app.envs.get(id)?;
        let job = e.cron.open.clone()?;
        let fields = json!({
            "name": e.cron.name.read(cx).value().trim(),
            "schedule": e.cron.schedule.read(cx).value().trim(),
            "prompt": e.cron.prompt.read(cx).value().trim(),
        });
        Some(if job == "new" {
            ("POST", format!("/api/projects/{eid}/cron"), fields)
        } else {
            (
                "PUT",
                format!("/api/cron/jobs/{}", enc(&job)),
                json!({ "updates": fields }),
            )
        })
    }

    fn saved(&self, e: &mut ProjectEnv) {
        e.cron.dirty = false;
        // A new job: its form closes, the job is in the list.
        if e.cron.open.as_deref() == Some("new") {
            e.cron.open = None;
            e.card = None;
        }
    }

    fn revert(
        &self,
        app: &mut BenchApp,
        id: &str,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> bool {
        let Some(e) = app.envs.get_mut(id) else {
            return true;
        };
        e.cron.dirty = false;
        if let Some(job) = e.cron.open.take() {
            if job != "new" {
                app.env_open_cron(id, job, window, cx);
                return true;
            }
        }
        false
    }
}

impl ExplorerParts for Schedule {
    /// What the project does by itself sits with what it is set to: the tree's Project group
    /// (the agent's own texts and tools are the groups above it).
    fn group(&self) -> &'static str {
        "Project"
    }

    fn block(&self, _general: bool) -> &'static str {
        "Schedule"
    }

    fn head(
        &self,
        _app: &BenchApp,
        e: &ProjectEnv,
        cx: &mut Context<BenchApp>,
    ) -> Option<AnyElement> {
        let id = e.id.clone();
        Some(
            head_act("env-cron-add".into(), "+ New job", cx)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.click_claimed = true;
                    cx.stop_propagation();
                    this.open_part(id.clone(), "cron", "new".into(), window, cx)
                }))
                .into_any_element(),
        )
    }

    /// What runs on its own, each run a thread of the project.
    fn items(&self, _app: &BenchApp, e: &ProjectEnv, cx: &mut Context<BenchApp>) -> Vec<PartRow> {
        let theme = cx.theme().clone();
        e.cron
            .jobs
            .iter()
            .map(|j| {
                let next = j
                    .next_run
                    .as_deref()
                    .map(when)
                    .unwrap_or_else(|| "no next run".into());
                let mut note = if j.enabled {
                    format!("{} · next {next}", j.display)
                } else {
                    format!("{} · paused", j.display)
                };
                if let Some(st) = &j.last_status {
                    note.push_str(&format!(" · last: {st}"));
                }
                let failed = j.last_status.as_deref().is_some_and(|s| s != "ok");
                PartRow {
                    name: j.id.clone(),
                    label: j.name.clone(),
                    note,
                    icon: (
                        bench_ui::icons::Ico::Clock,
                        if failed {
                            theme.danger
                        } else if j.enabled {
                            theme.foreground
                        } else {
                            theme.muted_foreground
                        },
                    ),
                }
            })
            .collect()
    }

    fn card(
        &self,
        app: &BenchApp,
        e: &ProjectEnv,
        name: &str,
        cx: &mut Context<BenchApp>,
    ) -> Option<Card> {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let job = e.cron.jobs.iter().find(|j| j.id == name).cloned();
        if name != "new" && job.is_none() {
            return None;
        }
        let mut actions = vec![];
        if let Some(j) = &job {
            let (run, pause, enabled) = (j.id.clone(), j.id.clone(), j.enabled);
            let (p1, p2) = (e.id.clone(), e.id.clone());
            actions.push(
                card_act("env-card-run".into(), "Run now".into(), muted)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        this.env_cron(&p1, run.clone(), "trigger", window, cx)
                    }))
                    .into_any_element(),
            );
            actions.push(
                card_act(
                    "env-card-pause".into(),
                    if enabled {
                        "Pause".into()
                    } else {
                        "Resume".into()
                    },
                    muted,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.click_claimed = true;
                    this.env_cron(
                        &p2,
                        pause.clone(),
                        if enabled { "pause" } else { "resume" },
                        window,
                        cx,
                    )
                }))
                .into_any_element(),
            );
        }
        actions.push(save(&e.id, "cron", e.cron.dirty, cx));
        actions.extend(revert(&e.id, "cron", e.cron.dirty, cx));
        if job.is_some() {
            actions.push(remove(app, e, "cron", name.to_string(), "Remove", cx));
        }
        let last = job
            .as_ref()
            .filter(|j| !j.last_output.trim().is_empty())
            .map(|j| {
                let at = j.last_run.as_deref().map(when).unwrap_or_default();
                div()
                    .text_sm()
                    .text_color(muted)
                    .child(format!("Last run {at}: {}", j.last_output.trim()))
            });
        Some(Card::new(
            job.as_ref()
                .map(|j| format!("Job · {}", j.name))
                .unwrap_or_else(|| "New job".into()),
            pad(v_flex()
                .child(Input::new(&e.cron.name))
                .child(Input::new(&e.cron.schedule))
                .child(bench_ui::editor_frame(cx).child(code(&e.cron.prompt, cx)))
                .children(last)),
            actions,
        ))
    }

    fn dirty(&self, e: &ProjectEnv) -> bool {
        e.cron.open.is_some() && e.cron.dirty
    }

    fn close(&self, e: &mut ProjectEnv) {
        e.cron.open = None;
        e.cron.dirty = false;
    }

    fn open(
        &self,
        app: &mut BenchApp,
        id: &str,
        job: String,
        w: &mut Window,
        cx: &mut Context<BenchApp>,
    ) {
        app.env_open_cron(id, job, w, cx);
    }

    fn remove_path(&self, _eid: &str, name: &str) -> Option<String> {
        Some(format!("/api/cron/jobs/{}", enc(name)))
    }
}

impl BenchApp {
    /// A job's row: its name, schedule and prompt open under it to edit (again: closes); "new":
    /// an empty form for a new job.
    pub fn env_open_cron(
        &mut self,
        id: &str,
        job: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(e) = self.envs.get_mut(id) else {
            return;
        };
        let c = &mut e.cron;
        if c.open.as_deref() == Some(&job) && !c.dirty {
            c.open = None;
            cx.notify();
            return;
        }
        let j = c
            .jobs
            .iter()
            .find(|j| j.id == job)
            .cloned()
            .unwrap_or_default();
        c.name
            .update(cx, |s, cx| s.set_value(j.name.clone(), window, cx));
        c.schedule
            .update(cx, |s, cx| s.set_value(j.schedule.clone(), window, cx));
        c.prompt
            .update(cx, |s, cx| s.set_value(j.prompt.clone(), window, cx));
        c.open = Some(job.clone());
        c.dirty = false;
        if job == "new" {
            c.name.update(cx, |s, cx| s.focus(window, cx));
        }
        cx.notify();
    }

    /// A job's "Run now", "Pause", "Resume".
    pub fn env_cron(
        &mut self,
        id: &str,
        job: String,
        action: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = id.to_string();
        let Some((conn, _)) = self.env_target(&id) else {
            return;
        };
        let path = format!("/api/cron/jobs/{}/{action}", enc(&job));
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "POST", &path, Value::Null),
            move |this, r, window, cx| match r {
                Ok(_) => {
                    if action == "trigger" {
                        this.toast(
                            NotificationType::Info,
                            "Started: its thread shows in the project",
                            window,
                            cx,
                        );
                    }
                    this.load_env(&id, window, cx);
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }
}
