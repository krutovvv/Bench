//! How the project's threads run: how hard it thinks, what it may run without asking. Chosen
//! here once, not per chat — its threads start on these; the model is one for every project
//! (the gear). Read from and saved to the agent its threads run with (`env_agent`).

use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::json;

use super::env_agent;
use super::section::{Card, Section, row};
use crate::app::BenchApp;
use crate::rest;

#[derive(Default)]
pub struct RunEnv {
    pub effort: String,
    pub approvals: String,
}

pub struct Run;

impl Section for Run {
    fn key(&self) -> &'static str {
        "model"
    }
    fn block(&self, _general: bool) -> &'static str {
        "Model"
    }
    fn rows(&self, app: &BenchApp, cx: &mut Context<BenchApp>) -> Vec<AnyElement> {
        let Some(e) = app.project_env.as_ref() else {
            return vec![];
        };
        let approval = match e.run.approvals.as_str() {
            "off" => "off — it asks for nothing",
            "manual" => "manual — it asks for everything",
            "" => "—",
            m => m,
        };
        let dash = |v: &str| {
            if v.is_empty() {
                "—".to_string()
            } else {
                v.to_string()
            }
        };
        vec![
            row(
                e,
                "model",
                "reasoning".into(),
                "Reasoning".into(),
                dash(&e.run.effort),
                false,
                false,
                cx,
            ),
            row(
                e,
                "model",
                "approvals".into(),
                "Approvals".into(),
                approval.to_string(),
                false,
                false,
                cx,
            ),
        ]
    }
    /// A choice, not a text: what it can be, the current one marked; a click sets it for the
    /// project at once (a choice has no draft to save).
    fn card(&self, app: &BenchApp, name: &str, cx: &mut Context<BenchApp>) -> Option<Card> {
        let e = app.project_env.as_ref()?;
        let theme = cx.theme().clone();
        let kind: &'static str = match name {
            "reasoning" => "reasoning",
            _ => "approvals",
        };
        let (cur, opts): (String, &[&str]) = match kind {
            "reasoning" => (e.run.effort.clone(), crate::model::EFFORTS),
            _ => (e.run.approvals.clone(), crate::model::APPROVAL_MODES),
        };
        let mut items: Vec<AnyElement> = vec![];
        for value in opts.iter().map(|v| v.to_string()) {
            let on = value == cur;
            let (id, label) = (
                SharedString::from(format!("env-run-{name}-{value}")),
                value.clone(),
            );
            items.push(
                h_flex()
                    .id(id)
                    .px_3()
                    .py_1p5()
                    .gap_2()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.list_hover))
                    .child(div().flex_1().min_w_0().truncate().child(label))
                    .when(on, |d| {
                        d.child(
                            crate::icons::Ico::Check
                                .icon()
                                .xsmall()
                                .text_color(theme.foreground),
                        )
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        cx.stop_propagation();
                        this.env_set_run(kind, value.clone(), window, cx)
                    }))
                    .into_any_element(),
            );
        }
        let (title, note) = match kind {
            "reasoning" => (
                "Reasoning",
                "How hard it thinks. A new thread of the project starts on it.",
            ),
            _ => (
                "Approvals",
                "What it may run without asking: off — nothing, smart — what is safe, manual — everything.",
            ),
        };
        Some(Card::new(
            title,
            v_flex()
                .w_full()
                .py_1()
                .children(items)
                .child(
                    div()
                        .px_3()
                        .pt_2()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(note),
                )
                .into_any_element(),
            vec![],
        ))
    }
}

impl BenchApp {
    /// One of the two running choices — its reasoning level, its approvals — set for the
    /// project. A click applies it at once (a choice has no draft); the project's next threads
    /// start on it, a thread already begun keeps what it started with.
    pub fn env_set_run(
        &mut self,
        what: &'static str,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(e) = self.project_env.as_ref() else {
            return;
        };
        let id = e.id.clone();
        let Some((conn, eid)) = self.env_target(&id) else {
            return;
        };
        let agent = env_agent(&eid);
        let body = match what {
            "reasoning" => json!({"config": {"agent": {"reasoning_effort": value}}}),
            _ => json!({"config": {"approvals": {"mode": value}}}),
        };
        let asked = id.clone();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "PUT", "/api/config", &agent, body),
            move |this, r, window, cx| match r {
                Ok(_) => this.load_env(&asked, window, cx),
                Err(err) => {
                    if let Some(e) = this.project_env.as_mut() {
                        e.error = Some(format!("{err:#}"));
                    }
                    cx.notify();
                }
            },
        );
    }
}
