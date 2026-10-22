//! Settings → Cron: pick a job, edit its prompt (Markdown) and schedule, save, run it now,
//! pause/resume. The model stays whatever the profile uses.

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use super::HermesApp;
use crate::rest;

impl HermesApp {
    pub(super) fn load_cron(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.rest(window, cx, rest::cron_jobs, |this, r, window, cx| match r {
            Ok(jobs) => {
                this.cron = jobs;
                // Keep the open job open (fresh values), or open the first one.
                let id = this
                    .cron_open
                    .clone()
                    .filter(|id| this.cron.iter().any(|j| &j.id == id))
                    .or_else(|| this.cron.first().map(|j| j.id.clone()));
                if let Some(id) = id {
                    this.open_cron(id, window, cx);
                }
                cx.notify();
            }
            Err(e) => this.toast(NotificationType::Error, format!("Cron: {e:#}"), window, cx),
        });
    }

    pub fn open_cron(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(job) = self.cron.iter().find(|j| j.id == id).cloned() else {
            return;
        };
        self.cron_open = Some(id);
        self.cron_editor
            .update(cx, |s, cx| s.set_value(job.prompt.clone(), window, cx));
        self.cron_schedule
            .update(cx, |s, cx| s.set_value(job.expr.clone(), window, cx));
        cx.notify();
    }

    pub fn save_cron(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.cron_open.clone() else {
            return;
        };
        let prompt = self.cron_editor.read(cx).value().to_string();
        let schedule = self.cron_schedule.read(cx).value().trim().to_string();
        if schedule.is_empty() {
            self.toast(
                NotificationType::Warning,
                "The schedule is empty",
                window,
                cx,
            );
            return;
        }
        self.rest(
            window,
            cx,
            move |c, p| rest::update_cron(c, p, &id, &prompt, &schedule),
            |this, r, window, cx| match r {
                Ok(()) => {
                    this.toast(NotificationType::Success, "Saved", window, cx);
                    this.load_cron(window, cx);
                }
                Err(e) => this.toast(NotificationType::Error, format!("Cron: {e:#}"), window, cx),
            },
        );
    }

    /// "trigger" runs the job now (its output lands in a cron session); "pause" / "resume".
    pub fn cron_act(&mut self, action: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.cron_open.clone() else {
            return;
        };
        self.rest(
            window,
            cx,
            move |c, p| rest::cron_action(c, p, &id, action),
            move |this, r, window, cx| match r {
                Ok(()) => {
                    let msg = match action {
                        "trigger" => "Started — its run appears under Cron in the session list",
                        "pause" => "Paused",
                        _ => "Resumed",
                    };
                    this.toast(NotificationType::Success, msg, window, cx);
                    this.load_cron(window, cx);
                    if action == "trigger" {
                        this.refresh_sessions();
                    }
                }
                Err(e) => this.toast(NotificationType::Error, format!("Cron: {e:#}"), window, cx),
            },
        );
    }
}
