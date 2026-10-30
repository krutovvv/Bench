//! The gear's providers, for an agent (the gear's), through the agent's API: OpenAI-compatible
//! endpoints, OpenRouter among them, one the main model and one the fallback that answers when it
//! fails or goes silent. Beside them `voice.rs`, `evaluator.rs`. A key is set on the card that
//! uses it, never in a list of names.

use std::collections::HashMap;

use gpui_kit::component::input::InputState;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;
use serde_json::{Value, json};

use super::server_form::{ServerForm, card_action, field, input_row, text};
use super::voice::Voice;
use crate::app::BenchApp;
use crate::client::rest::{self};

/// A custom endpoint's form.
pub struct EndpointForm {
    pub server: ServerForm,
    pub current: bool,
    /// Where the key lives when it is a `.env` reference (`${NAME}`).
    pub key_env: Option<String>,
    pub context: Entity<InputState>,
    /// How many requests its server takes at once; empty: no limit (`agent/src/runtime/streams.ts`).
    pub parallel: Entity<InputState>,
}

#[derive(Default)]
pub struct Providers {
    pub loaded: bool,
    pub endpoints: Vec<EndpointForm>,
    pub voice: Option<Voice>,
    pub judge: super::evaluator::Judge,
    pub fallback: Option<(String, String)>,
    _subscriptions: Vec<Subscription>,
}

impl BenchApp {
    /// The model picker's list: each provider's models kept together (the main model's group
    /// first).
    pub(crate) fn add_picker_models(&mut self) {
        let mut order: Vec<String> = vec![];
        for c in &self.defaults.models {
            if !order.contains(&c.group) {
                order.push(c.group.clone());
            }
        }
        // A stable sort: inside a group the order stays (main, fallbacks, the rest).
        self.defaults.models
            .sort_by_key(|c| order.iter().position(|g| *g == c.group));
    }

    // ---- loading ----------------------------------------------------------------------

    /// The gear shows the agent under `host`: its providers, fallback and voice are read anew.
    pub(crate) fn show_gear_agent(
        &mut self,
        host: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if host == self.gear.agent {
            return;
        }
        self.gear.agent = host;
        self.gear.providers = Default::default();
        self.load_providers(window, cx);
        cx.notify();
    }

    pub(crate) fn load_providers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.rest_on(
            self.gear.agent.clone(),
            window,
            cx,
            move |conn| {
                let endpoints = rest::custom_endpoints(conn)?;
                let env = rest::env_vars(conn)?;
                let mut yaml: Value =
                    serde_yaml::from_str(&rest::config_raw(conn)?).unwrap_or(Value::Null);
                // The evaluator's servers as the agent reads them (an earlier one-server setting too).
                yaml["judge"] = rest::judge(conn)?;
                Ok((endpoints, env, yaml))
            },
            |this, r, window, cx| match r {
                Ok((endpoints, env, yaml)) => this.fill_providers(endpoints, env, yaml, window, cx),
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Providers: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
    }

    fn fill_providers(
        &mut self,
        endpoints: Value,
        env: Value,
        yaml: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // What `GET /api/env` shows of each set value: its redacted form.
        let shown: HashMap<String, String> = env
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(_, v)| v["is_set"].as_bool() == Some(true))
            .map(|(k, v)| {
                let value = v["redacted_value"].as_str().unwrap_or_default();
                (
                    k.clone(),
                    if value.is_empty() {
                        "set".into()
                    } else {
                        value.to_string()
                    },
                )
            })
            .collect();
        self.gear.providers.endpoints = endpoints["endpoints"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|e| {
                let s = |k: &str| e[k].as_str().unwrap_or_default().to_string();
                let number = |k: &str| e[k].as_u64().map(|n| n.to_string()).unwrap_or_default();
                let context = number("context_length");
                let parallel = number("parallel");
                // `${NAME}`: the key lives in .env under NAME.
                let preview = s("api_key_preview");
                let key_env = preview
                    .strip_prefix("${")
                    .and_then(|p| p.strip_suffix('}'))
                    .map(str::to_string);
                // A key kept in the agent's env shows its value; one kept with the endpoint, itself.
                let key_value = match &key_env {
                    Some(k) => shown.get(k).cloned().unwrap_or_default(),
                    None => preview.clone(),
                };
                EndpointForm {
                    server: ServerForm {
                        id: s("id"),
                        name: field(window, cx, &s("name"), "Name"),
                        url: field(window, cx, &s("base_url"), "http://host:port/v1"),
                        model: field(window, cx, &s("model"), "Default model"),
                        key: field(window, cx, &key_value, "If the server asks for one"),
                        key_value,
                        check: None,
                    },
                    current: e["is_current"].as_bool().unwrap_or(false),
                    key_env,
                    context: field(window, cx, &context, "e.g. 131072"),
                    parallel: field(window, cx, &parallel, "No limit"),
                }
            })
            .collect();

        let f = &yaml["fallback_providers"][0];
        self.gear.providers.fallback = f["model"].as_str().map(|m| {
            (
                f["provider"].as_str().unwrap_or_default().to_string(),
                m.to_string(),
            )
        });

        self.gear.providers.voice = Some(Voice::from_config(&yaml, window, cx));
        self.fill_judge(&yaml["judge"], window, cx);

        self.gear.providers.loaded = true;
        cx.notify();
    }

    // ---- actions ----------------------------------------------------------------------

    /// After a save that changes what the agent offers: reload both lists.
    fn providers_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.load_providers(window, cx);
        self.load_agent_settings(window, cx);
    }

    pub fn add_endpoint(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.gear.providers.endpoints.push(EndpointForm {
            server: ServerForm {
                id: String::new(),
                name: field(window, cx, "", "Name, e.g. local"),
                url: field(window, cx, "", "http://host:port/v1"),
                model: field(window, cx, "", "Default model"),
                key: field(window, cx, "", "If the server asks for one"),
                key_value: String::new(),
                check: None,
            },
            current: false,
            key_env: None,
            context: field(window, cx, "", "e.g. 131072"),
            parallel: field(window, cx, "", "No limit"),
        });
        cx.notify();
    }

    fn endpoint_body(&self, i: usize, cx: &App) -> Value {
        let e = &self.gear.providers.endpoints[i];
        let mut body = e.server.body(cx);
        body["discover_models"] = json!(true);
        if let Ok(n) = text(&e.context, cx).parse::<u64>() {
            body["context_length"] = json!(n);
        }
        // Empty clears it; anything but a number is no limit either.
        body["parallel"] = json!(text(&e.parallel, cx));
        // Only a changed key: an unchanged one stays where it is (often a .env reference).
        if let Some(key) = e.server.changed_key(cx).filter(|k| !k.is_empty()) {
            body["api_key"] = json!(key);
        }
        body
    }

    pub fn save_endpoint(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let body = self.endpoint_body(i, cx);
        if body["name"].as_str().unwrap_or_default().is_empty()
            || body["base_url"].as_str().unwrap_or_default().is_empty()
        {
            self.gear.providers.endpoints[i].server.check = Some((false, "Enter a name and an address".into()));
            cx.notify();
            return;
        }
        self.rest_on(
            self.gear.agent.clone(),
            window,
            cx,
            move |conn| rest::save_endpoint(conn, body),
            |this, r, window, cx| match r {
                Ok(_) => {
                    this.toast(NotificationType::Success, "Provider saved", window, cx);
                    this.providers_changed(window, cx);
                }
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Provider: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
    }

    pub fn check_endpoint(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let body = self.endpoint_body(i, cx);
        self.gear.providers.endpoints[i].server.check = Some((true, "Checking…".into()));
        cx.notify();
        self.rest_on(
            self.gear.agent.clone(),
            window,
            cx,
            move |conn| rest::validate_endpoint(conn, body),
            move |this, r, _, cx| {
                let Some(e) = this.gear.providers.endpoints.get_mut(i) else {
                    return;
                };
                e.server.check = Some(match r {
                    Ok(v) if v["ok"].as_bool() == Some(true) => {
                        let n = v["models"].as_array().map_or(0, |a| a.len());
                        (true, format!("Works · {n} models"))
                    }
                    Ok(v) => (false, ServerForm::check_message(&v)),
                    Err(e) => (false, format!("{e:#}")),
                });
                cx.notify();
            },
        );
    }

    pub fn make_endpoint_default(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.gear.providers.endpoints[i].server.id.clone();
        self.rest_on(
            self.gear.agent.clone(),
            window,
            cx,
            move |conn| rest::activate_endpoint(conn, &id),
            |this, r, window, cx| match r {
                Ok(()) => this.providers_changed(window, cx),
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Provider: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
    }

    /// The provider's model becomes the fallback; the fallback again: no fallback.
    pub fn toggle_endpoint_fallback(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let e = &self.gear.providers.endpoints[i].server;
        let is_it = self.gear.providers.fallback.as_ref().is_some_and(|(p, _)| *p == e.id);
        let pick = (!is_it).then(|| (e.id.clone(), text(&e.model, cx)));
        self.set_fallback(pick, window, cx);
    }

    /// The fallback model: one (provider, model), or none.
    fn set_fallback(
        &mut self,
        pick: Option<(String, String)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let list = match &pick {
            Some((p, m)) => json!([{"provider": p, "model": m}]),
            None => json!([]),
        };
        self.gear.providers.fallback = pick;
        cx.notify();
        self.rest_on(
            self.gear.agent.clone(),
            window,
            cx,
            move |conn| rest::merge_config(conn, json!({"fallback_providers": list})),
            |this, r, window, cx| match r {
                Ok(()) => this.load_agent_settings(window, cx),
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Fallback: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
    }

    /// Delete a provider: a form not saved yet just goes; a saved one goes on the second click.
    pub fn delete_endpoint(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.gear.providers.endpoints[i].server.id.clone();
        if id.is_empty() {
            self.gear.providers.endpoints.remove(i);
            cx.notify();
            return;
        }
        if !self.armed.confirm(format!("endpoint:{id}")) {
            cx.notify();
            return;
        }
        self.rest_on(
            self.gear.agent.clone(),
            window,
            cx,
            move |conn| rest::delete_endpoint(conn, &id),
            |this, r, window, cx| match r {
                Ok(()) => this.providers_changed(window, cx),
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Provider: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
    }

    // ---- the page -----------------------------------------------------------------------

    /// The gear's models: a heading, a card per provider, "Add".
    pub(crate) fn render_providers(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let (primary, muted) = (cx.theme().primary, cx.theme().muted_foreground);
        let fallback = self.gear.providers.fallback.as_ref().map(|(p, _)| p.clone());
        let mut page = vec![
            bench_ui::settings_heading(
                // One model for every thread of this agent's projects: the main one and its
                // fallback.
                format!("Models · {} · its projects", Self::agent_name(&self.gear.agent)),
                vec![],
                cx,
            )
            .into_any_element(),
        ];
        for i in 0..self.gear.providers.endpoints.len() {
            let e = &self.gear.providers.endpoints[i];
            let saved = e.server.saved();
            let title = if saved { e.server.id.clone() } else { "New provider".into() };
            let key_hint = match &e.key_env {
                Some(name) => format!("In .env as {name}"),
                None => "If the server asks for one".into(),
            };
            let armed = saved && self.armed.is(&format!("endpoint:{}", e.server.id));
            let mut actions = vec![
                card_action(("ep-delete", i), if armed { "Confirm delete" } else { "Delete" }, false, move |this, w, cx| this.delete_endpoint(i, w, cx), cx),
                card_action(("ep-check", i), "Check", false, move |this, w, cx| this.check_endpoint(i, w, cx), cx),
            ];
            let is_fallback = saved && fallback.as_deref() == Some(e.server.id.as_str());
            if saved && !e.current {
                actions.push(card_action(("ep-default", i), "Make main", false, move |this, w, cx| this.make_endpoint_default(i, w, cx), cx));
                actions.push(card_action(("ep-fallback", i), if is_fallback { "No fallback" } else { "Make fallback" }, false, move |this, w, cx| this.toggle_endpoint_fallback(i, w, cx), cx));
            }
            actions.push(card_action(("ep-save", i), "Save", true, move |this, w, cx| this.save_endpoint(i, w, cx), cx));
            let role = if e.current {
                Some(("main model".into(), primary))
            } else if is_fallback {
                Some(("fallback".into(), muted))
            } else {
                None
            };
            let rows = vec![
                input_row("Name", "How it is listed", &e.server.name, cx),
                input_row("Address", "OpenAI-compatible, ends in /v1", &e.server.url, cx),
                input_row("Default model", "The model it serves first", &e.server.model, cx),
                input_row("Context length", "Tokens", &e.context, cx),
                input_row("Streams", "How many requests its server takes at once, as its config says. As the main model: a thread at work keeps one, the others wait in line", &e.parallel, cx),
                input_row("API key", key_hint, &e.server.key, cx),
            ];
            page.push(bench_ui::settings_card(title, role, rows, e.server.check_line(), actions, cx).into_any_element());
        }
        page.push(
            bench_ui::settings_add("ep-add", "Add a provider…", cx)
                .on_click(cx.listener(|this, _, window, cx| this.add_endpoint(window, cx)))
                .into_any_element(),
        );
        page
    }
}
