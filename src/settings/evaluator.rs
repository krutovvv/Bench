//! The gear's evaluator: System One servers (Jev's API: TypeSafe, OpenRouter, a local one), as
//! many as the person keeps, one of them the main and one its fallback, as the main model has its
//! own. No switch: a project's Settings say whether its threads use it (`agent/src/judge/`).

use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;
use serde_json::{Value, json};

use crate::app::BenchApp;
use super::server_form::{ServerForm, card_action, field, input_row, text};
use crate::client::rest;

/// The servers, and which is the main one and which its fallback.
#[derive(Default)]
pub struct Judge {
    pub servers: Vec<ServerForm>,
    pub main: Option<String>,
    pub fallback: Option<String>,
}

impl BenchApp {
    /// The agent's list (`GET /api/judge`: `{servers, main, fallback}`) as the gear's forms.
    pub(crate) fn fill_judge(&mut self, v: &Value, window: &mut Window, cx: &mut Context<Self>) {
        let s = |x: &Value, k: &str| x[k].as_str().unwrap_or_default().to_string();
        let servers = v["servers"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|x| ServerForm {
                id: s(x, "id"),
                name: field(window, cx, &s(x, "name"), "Name"),
                url: field(window, cx, &s(x, "base_url"), "https://api.typesafe.ai or http://host:port"),
                model: field(window, cx, &s(x, "model"), "jev-latest"),
                key: field(window, cx, &s(x, "api_key"), "If the server asks for one"),
                key_value: s(x, "api_key"),
                check: None,
            })
            .collect();
        self.gear.providers.judge = Judge {
            servers,
            main: v["main"].as_str().map(str::to_string),
            fallback: v["fallback"].as_str().map(str::to_string),
        };
        cx.notify();
    }

    pub fn add_judge_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.gear.providers.judge.servers.push(ServerForm {
            id: String::new(),
            name: field(window, cx, "", "Name, e.g. Local Jev"),
            url: field(window, cx, "", "https://api.typesafe.ai or http://host:port"),
            model: field(window, cx, "", "jev-latest"),
            key: field(window, cx, "", "If the server asks for one"),
            key_value: String::new(),
            check: None,
        });
        cx.notify();
    }

    fn judge_body(&self, i: usize, cx: &App) -> Value {
        let j = &self.gear.providers.judge.servers[i];
        let mut body = j.body(cx);
        // Only a changed key: an empty one keeps the saved one.
        if let Some(key) = j.changed_key(cx) {
            body["api_key"] = json!(key);
        }
        body
    }

    /// A change to the list, the agent's answer the list as it is now.
    fn judge_change(
        &mut self,
        what: &'static str,
        change: impl FnOnce(&crate::client::connection::Connection) -> anyhow::Result<Value> + Send + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.rest_on(
            self.gear.agent.clone(),
            window,
            cx,
            change,
            move |this, r, window, cx| match r {
                Ok(v) => {
                    this.fill_judge(&v, window, cx);
                    if !what.is_empty() {
                        this.toast(NotificationType::Success, what, window, cx);
                    }
                }
                Err(e) => this.toast(NotificationType::Error, format!("Evaluator: {e:#}"), window, cx),
            },
        );
    }

    pub fn save_judge_server(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let body = self.judge_body(i, cx);
        if body["base_url"].as_str().unwrap_or_default().is_empty() {
            self.gear.providers.judge.servers[i].check = Some((false, "Enter the server's address".into()));
            cx.notify();
            return;
        }
        self.judge_change("Evaluator saved", move |conn| rest::judge_save(conn, body), window, cx);
    }

    /// Ask the server one question with the fields as typed, before or after saving them.
    pub fn check_judge_server(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let mut body = self.judge_body(i, cx);
        if body.get("api_key").is_none() {
            body["api_key"] = json!("");
        }
        self.gear.providers.judge.servers[i].check =
            Some((true, "Checking… a local server may load its model first".into()));
        cx.notify();
        self.rest_on(
            self.gear.agent.clone(),
            window,
            cx,
            move |conn| rest::check_judge(conn, body),
            move |this, r, _, cx| {
                let Some(j) = this.gear.providers.judge.servers.get_mut(i) else {
                    return;
                };
                j.check = Some(match r {
                    Ok(v) => (
                        v["ok"].as_bool() == Some(true),
                        ServerForm::check_message(&v),
                    ),
                    Err(e) => (false, format!("{e:#}")),
                });
                cx.notify();
            },
        );
    }

    pub fn make_judge_main(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.gear.providers.judge.servers[i].id.clone();
        self.judge_change("", move |conn| rest::judge_main(conn, &id), window, cx);
    }

    /// The server becomes the fallback; the fallback again: no fallback.
    pub fn toggle_judge_fallback(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let j = &self.gear.providers.judge;
        let id = j.servers[i].id.clone();
        let pick = (j.fallback.as_deref() != Some(&id)).then_some(id);
        self.judge_change("", move |conn| rest::judge_fallback(conn, pick.as_deref()), window, cx);
    }

    /// A form not saved yet just goes; a saved server goes on the second click.
    pub fn delete_judge_server(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.gear.providers.judge.servers[i].id.clone();
        if id.is_empty() {
            self.gear.providers.judge.servers.remove(i);
            cx.notify();
            return;
        }
        if !self.armed.confirm(format!("judge:{id}")) {
            cx.notify();
            return;
        }
        self.judge_change("", move |conn| rest::judge_delete(conn, &id), window, cx);
    }

    /// The gear's evaluator block: a heading, a line on what it is for, a card per server, "Add".
    pub(crate) fn render_evaluator(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let mut page = vec![
            bench_ui::settings_heading("Evaluator · System One API", vec![], cx).into_any_element(),
            div()
                .pb_1()
                .text_sm()
                .text_color(muted)
                .child("A small model asked many quick questions: the agent's own checks on its way, and code mode scripts' models.classify(). Projects that turn it on in their Settings use the main server, the fallback when it fails. No server: none does")
                .into_any_element(),
        ];
        let j = &self.gear.providers.judge;
        let (main, fallback) = (j.main.clone(), j.fallback.clone());
        for i in 0..j.servers.len() {
            let s = &self.gear.providers.judge.servers[i];
            let saved = s.saved();
            let is_main = saved && main.as_deref() == Some(&s.id);
            let is_fallback = saved && fallback.as_deref() == Some(&s.id);
            let named = text(&s.name, cx);
            let title = if !saved {
                "New server".to_string()
            } else if named.is_empty() {
                s.id.clone()
            } else {
                named
            };
            let armed = saved && self.armed.is(&format!("judge:{}", s.id));
            let mut actions = vec![
                card_action(("judge-delete", i), if armed { "Confirm delete" } else { "Delete" }, false, move |this, w, cx| this.delete_judge_server(i, w, cx), cx),
                card_action(("judge-check", i), "Check", false, move |this, w, cx| this.check_judge_server(i, w, cx), cx),
            ];
            if saved && !is_main {
                actions.push(card_action(("judge-main", i), "Make main", false, move |this, w, cx| this.make_judge_main(i, w, cx), cx));
                actions.push(card_action(("judge-fallback", i), if is_fallback { "No fallback" } else { "Make fallback" }, false, move |this, w, cx| this.toggle_judge_fallback(i, w, cx), cx));
            }
            actions.push(card_action(("judge-save", i), "Save", true, move |this, w, cx| this.save_judge_server(i, w, cx), cx));
            let role = if is_main {
                Some(("main".into(), theme.primary))
            } else if is_fallback {
                Some(("fallback".into(), muted))
            } else {
                None
            };
            let rows = vec![
                input_row("Name", "How it is listed", &s.name, cx),
                input_row("Server", "Its address; /v1/systemone is added", &s.url, cx),
                input_row("Model", "", &s.model, cx),
                input_row("API key", "If the server asks for one", &s.key, cx),
            ];
            page.push(bench_ui::settings_card(title, role, rows, s.check_line(), actions, cx).into_any_element());
        }
        page.push(
            bench_ui::settings_add("judge-add", "Add a server…", cx)
                .on_click(cx.listener(|this, _, window, cx| this.add_judge_server(window, cx)))
                .into_any_element(),
        );
        page
    }
}
