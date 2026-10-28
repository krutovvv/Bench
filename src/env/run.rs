//! How the project's threads run: how hard it thinks, what it may run without asking, whether it
//! works in code mode (a script that calls many tools in one step), when its conversations are
//! summarized (compaction, a share of the model's window): a group of General,
//! chosen once, not per chat — its threads start on these; the model is one for every project
//! (the gear). Read from and saved to the agent its threads run with (`settings_of`).

use gpui_kit::*;
use serde_json::json;

use super::section::{Section, SettingsForm, choices};
use super::{ProjectEnv, settings_of};
use crate::app::BenchApp;
use crate::client::rest;
use bench_ui::settings_field;

#[derive(Default)]
pub struct RunEnv {
    pub effort: String,
    pub approvals: String,
    /// The share of the model's window at which its conversations are summarized; None: Pi's own.
    pub compaction: Option<f64>,
    /// The model's window, in tokens (what the share is of).
    pub window: u64,
    /// Its threads get Pi's code mode (`codemode`) from their next turn.
    pub code_mode: bool,
}

/// What a compaction share means for this window, said in a line.
/// `names`: the presets' (share, name), to name a share that has one.
fn compaction_note(at: Option<f64>, window: u64, names: &[(f64, String)]) -> String {
    let k = |t: f64| format!("{}K", (t / 1000.).round() as u64);
    let w = window as f64;
    match at {
        None if window > 0 => format!(
            "Pi's own: in the background from {}, at {} at most; keeps {} word for word",
            k(w - 16384. - 32768.),
            k(w - 16384.),
            k(20000.)
        ),
        None => "Pi's own".into(),
        Some(at) => {
            let name = names
                .iter()
                .find(|(s, _)| *s == at)
                .map(|(_, n)| n.as_str())
                .filter(|n| !n.is_empty())
                .map(|n| format!("{n} · "))
                .unwrap_or_default();
            format!(
                "{name}at {}% ({} of {}); keeps {} word for word",
                (at * 100.).round(),
                k(w * at),
                k(w),
                k(w * at * 0.4)
            )
        }
    }
}

pub struct Run;

impl Section for Run {
    fn key(&self) -> &'static str {
        "model"
    }

    fn apply(&self, e: &mut ProjectEnv, v: &serde_json::Value, _w: &mut Window, _cx: &mut App) {
        e.run.compaction = v["compaction"].as_f64();
        e.run.window = v["window"].as_u64().unwrap_or(0);
        e.run.code_mode = v["code_mode"].as_bool().unwrap_or(false);
    }
}

impl SettingsForm for Run {
    fn block(&self, _general: bool) -> &'static str {
        "Model"
    }

    /// Choices, not texts: a click sets one for the project at once (a choice has no draft to
    /// save). The reasoning levels and the compaction shares are the presets' (the gear).
    fn form(
        &self,
        app: &BenchApp,
        e: &ProjectEnv,
        cx: &mut Context<BenchApp>,
    ) -> Option<Vec<AnyElement>> {
        let view = cx.entity();
        let pid = e.id.clone();
        let segmented = |id: &'static str,
                         kind: &'static str,
                         opts: Vec<String>,
                         cur: &str,
                         cx: &mut Context<BenchApp>| {
            let chosen = opts.iter().position(|o| o == cur).unwrap_or(usize::MAX);
            let labels: Vec<SharedString> = opts.iter().map(|o| o.clone().into()).collect();
            let (view, pid) = (view.clone(), pid.clone());
            let pick: bench_ui::OnPick = std::rc::Rc::new(move |k, window, cx| {
                let value = opts[k].clone();
                let pid = pid.clone();
                view.update(cx, |this, cx| {
                    this.env_set_run(&pid, kind, value, window, cx)
                });
            });
            div()
                .flex()
                .justify_end()
                .child(bench_ui::segmented(id, &labels, chosen, cx, pick))
                .into_any_element()
        };
        let reasoning = segmented(
            "env-reasoning",
            "reasoning",
            app.defaults.efforts.clone(),
            &e.run.effort,
            cx,
        );
        let approvals = segmented(
            "env-approvals",
            "approvals",
            crate::app::model::APPROVAL_MODES
                .iter()
                .map(|m| m.to_string())
                .collect(),
            &e.run.approvals,
            cx,
        );
        let code_mode = segmented(
            "env-code-mode",
            "code_mode",
            vec!["Off".into(), "On".into()],
            if e.run.code_mode { "On" } else { "Off" },
            cx,
        );
        // Pi's own first, then the presets' shares.
        let names = app.gear.presets.compaction.clone();
        let shares: Vec<Option<f64>> = std::iter::once(None)
            .chain(names.iter().map(|(a, _)| Some(*a)))
            .collect();
        let items = shares
            .iter()
            .map(|at| {
                (
                    compaction_note(*at, e.run.window, &names),
                    e.run.compaction == *at,
                )
            })
            .collect();
        let compaction = choices(
            "env-compaction",
            items,
            std::rc::Rc::new(move |this, k, window, cx| {
                let value = shares[k].map(|a| a.to_string()).unwrap_or_default();
                this.env_set_run(&pid, "compaction", value, window, cx)
            }),
            cx,
        );
        Some(vec![
            settings_field(
                "Reasoning",
                "How hard it thinks; a new thread starts on it",
                reasoning,
                cx,
            )
            .into_any_element(),
            settings_field(
                "Approvals",
                "What it runs without asking: off — all, smart — what is safe, manual — nothing",
                approvals,
                cx,
            )
            .into_any_element(),
            settings_field(
                "Code mode",
                "A script calls many tools in one step: fewer steps for work in code",
                code_mode,
                cx,
            )
            .into_any_element(),
            settings_field(
                "Compaction",
                "When its conversations are summarized, as a share of the model's window; later keeps more in front of the model",
                compaction,
                cx,
            )
            .into_any_element(),
        ])
    }
}

impl BenchApp {
    /// One of the running choices — its reasoning level, its approvals, code mode, compaction —
    /// set for the project. A click applies it at once (a choice has no draft); the project's
    /// next threads start on it, a thread already begun keeps what it started with (code mode
    /// alone reaches its threads from their next turn).
    pub fn env_set_run(
        &mut self,
        id: &str,
        what: &'static str,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.envs.contains_key(id) {
            return;
        }
        let id = id.to_string();
        let Some((conn, eid)) = self.env_target(&id) else {
            return;
        };
        let agent = settings_of(&eid);
        let body = match what {
            "reasoning" => json!({"config": {"agent": {"reasoning_effort": value}}}),
            "compaction" => json!({"config": {"compaction": {"at": value.parse::<f64>().ok()}}}),
            "code_mode" => json!({"config": {"code_mode": {"enabled": value == "On"}}}),
            _ => json!({"config": {"approvals": {"mode": value}}}),
        };
        let asked = id.clone();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "PUT", &rest::with_settings("/api/config", &agent), body),
            move |this, r, window, cx| match r {
                Ok(_) => this.load_env(&asked, window, cx),
                Err(err) => {
                    if let Some(e) = this.envs.get_mut(&asked) {
                        e.error = Some(format!("{err:#}"));
                    }
                    cx.notify();
                }
            },
        );
    }
}
