//! Settings → Providers and Messages, for the current machine and profile, through the agent's
//! API: providers (OpenAI-compatible endpoints, OpenRouter among them), the fallback model,
//! voice (speech to text, text to speech) and Telegram. A key is set
//! on the card that uses it, never in a list of names.

use std::collections::HashMap;

use gpui_kit::component::input::InputState;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;
use serde_json::{Value, json};

use crate::app::BenchApp;
use crate::client::rest::{self};

/// A custom endpoint's form.
pub struct EndpointForm {
    /// "" for one not saved yet.
    pub id: String,
    pub current: bool,
    /// The key as shown (its redacted `.env` value when the endpoint refers to one), to send only
    /// a change.
    pub key_value: String,
    /// Where the key lives when it is a `.env` reference (`${NAME}`).
    pub key_env: Option<String>,
    pub name: Entity<InputState>,
    pub url: Entity<InputState>,
    pub model: Entity<InputState>,
    pub context: Entity<InputState>,
    /// How many requests its server answers at once ("" = as many as asked): subagents keep to it.
    pub parallel: Entity<InputState>,
    pub key: Entity<InputState>,
    /// The last check: (ok, what it said).
    pub check: Option<(bool, String)>,
}

pub struct Voice {
    pub stt_url: Entity<InputState>,
    pub stt_model: Entity<InputState>,
    pub stt_lang: Entity<InputState>,
    pub stt_key: Entity<InputState>,
    pub tts_url: Entity<InputState>,
    pub tts_model: Entity<InputState>,
    pub tts_voice: Entity<InputState>,
}

#[derive(Default)]
pub struct Providers {
    pub loaded: bool,
    pub endpoints: Vec<EndpointForm>,
    pub voice: Option<Voice>,
    pub fallback: Option<(String, String)>,
    /// A provider armed for deletion (its id; the second click deletes it).
    pub armed: Option<String>,
    _subscriptions: Vec<Subscription>,
}

fn field(
    window: &mut Window,
    cx: &mut Context<BenchApp>,
    text: &str,
    hint: &str,
) -> Entity<InputState> {
    let text = text.to_string();
    let hint = hint.to_string();
    cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder(hint);
        s.set_value(text, window, cx);
        s
    })
}

fn text(input: &Entity<InputState>, cx: &App) -> String {
    input.read(cx).value().trim().to_string()
}

impl BenchApp {
    /// The model picker's list: each provider's models kept together (the main model's group
    /// first).
    pub(crate) fn add_picker_models(&mut self) {
        let mut order: Vec<String> = vec![];
        for c in &self.models {
            if !order.contains(&c.group) {
                order.push(c.group.clone());
            }
        }
        // A stable sort: inside a group the order stays (main, fallbacks, the rest).
        self.models
            .sort_by_key(|c| order.iter().position(|g| *g == c.group));
    }

    // ---- loading ----------------------------------------------------------------------

    /// Fill both tabs from the agent.
    /// The gear shows the agent under `host`: its providers, fallback and voice are read anew.
    pub(crate) fn show_gear_agent(
        &mut self,
        host: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if host == self.gear_agent {
            return;
        }
        self.gear_agent = host;
        self.pv = Default::default();
        self.load_providers(window, cx);
        cx.notify();
    }

    pub(crate) fn load_providers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.rest_on(
            self.gear_agent.clone(),
            window,
            cx,
            move |conn, profile| {
                let endpoints = rest::custom_endpoints(conn, profile)?;
                let env = rest::env_vars(conn, profile)?;
                let yaml: Value =
                    serde_yaml::from_str(&rest::config_raw(conn, profile)?).unwrap_or(Value::Null);
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
        self.pv.endpoints = endpoints["endpoints"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|e| {
                let s = |k: &str| e[k].as_str().unwrap_or_default().to_string();
                let context = e["context_length"]
                    .as_u64()
                    .map(|n| n.to_string())
                    .unwrap_or_default();
                let parallel = e["parallel"]
                    .as_u64()
                    .map(|n| n.to_string())
                    .unwrap_or_default();
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
                    id: s("id"),
                    current: e["is_current"].as_bool().unwrap_or(false),
                    name: field(window, cx, &s("name"), "Name"),
                    url: field(window, cx, &s("base_url"), "http://host:port/v1"),
                    model: field(window, cx, &s("model"), "Default model"),
                    context: field(window, cx, &context, "e.g. 131072"),
                    parallel: field(window, cx, &parallel, "e.g. 2; empty: no limit"),
                    key: field(window, cx, &key_value, "If the server asks for one"),
                    key_value,
                    key_env,
                    check: None,
                }
            })
            .collect();

        let f = &yaml["fallback_providers"][0];
        self.pv.fallback = f["model"].as_str().map(|m| {
            (
                f["provider"].as_str().unwrap_or_default().to_string(),
                m.to_string(),
            )
        });

        let y = |path: &[&str]| {
            let mut v = &yaml;
            for p in path {
                v = &v[*p];
            }
            v.as_str().unwrap_or_default().to_string()
        };
        self.pv.voice = Some(Voice {
            stt_url: field(
                window,
                cx,
                &y(&["stt", "openai", "base_url"]),
                "http://host:port/v1",
            ),
            stt_model: field(
                window,
                cx,
                &y(&["stt", "openai", "model"]),
                "e.g. whisper-1",
            ),
            stt_lang: field(
                window,
                cx,
                &y(&["stt", "openai", "language"]),
                "e.g. en, ru; empty = auto",
            ),
            stt_key: field(
                window,
                cx,
                &y(&["stt", "openai", "api_key"]),
                "If the server asks for one",
            ),
            tts_url: field(
                window,
                cx,
                &y(&["tts", "openai", "base_url"]),
                "http://host:port/v1",
            ),
            tts_model: field(
                window,
                cx,
                &y(&["tts", "openai", "model"]),
                "e.g. gpt-4o-mini-tts",
            ),
            tts_voice: field(window, cx, &y(&["tts", "openai", "voice"]), "e.g. alloy"),
        });

        self.pv.loaded = true;
        cx.notify();
    }

    // ---- actions ----------------------------------------------------------------------

    /// After a save that changes what the profile offers: reload both lists.
    fn providers_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.load_providers(window, cx);
        self.load_agent_settings(window, cx);
    }

    pub fn add_endpoint(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pv.endpoints.push(EndpointForm {
            id: String::new(),
            current: false,
            key_value: String::new(),
            key_env: None,
            name: field(window, cx, "", "Name, e.g. local"),
            url: field(window, cx, "", "http://host:port/v1"),
            model: field(window, cx, "", "Default model"),
            context: field(window, cx, "", "e.g. 131072"),
            parallel: field(window, cx, "", "e.g. 2; empty: no limit"),
            key: field(window, cx, "", "If the server asks for one"),
            check: None,
        });
        cx.notify();
    }

    fn endpoint_body(&self, i: usize, cx: &App) -> Value {
        let e = &self.pv.endpoints[i];
        let mut body = json!({
            "id": e.id,
            "name": text(&e.name, cx),
            "base_url": text(&e.url, cx),
            "model": text(&e.model, cx),
            "discover_models": true,
        });
        if let Ok(n) = text(&e.context, cx).parse::<u64>() {
            body["context_length"] = json!(n);
        }
        // Empty clears the limit (0 on the wire).
        body["parallel"] = json!(text(&e.parallel, cx).parse::<u64>().unwrap_or(0));
        // Only a changed key: an unchanged one stays where it is (often a .env reference).
        let key = text(&e.key, cx);
        if !key.is_empty() && key != e.key_value {
            body["api_key"] = json!(key);
        }
        body
    }

    pub fn save_endpoint(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let body = self.endpoint_body(i, cx);
        if body["name"].as_str().unwrap_or_default().is_empty()
            || body["base_url"].as_str().unwrap_or_default().is_empty()
        {
            self.pv.endpoints[i].check = Some((false, "Enter a name and an address".into()));
            cx.notify();
            return;
        }
        self.rest_on(
            self.gear_agent.clone(),
            window,
            cx,
            move |conn, profile| rest::save_endpoint(conn, profile, body),
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
        self.pv.endpoints[i].check = Some((true, "Checking…".into()));
        cx.notify();
        self.rest_on(
            self.gear_agent.clone(),
            window,
            cx,
            move |conn, profile| rest::validate_endpoint(conn, profile, body),
            move |this, r, _, cx| {
                let Some(e) = this.pv.endpoints.get_mut(i) else {
                    return;
                };
                e.check = Some(match r {
                    Ok(v) if v["ok"].as_bool() == Some(true) => {
                        let n = v["models"].as_array().map_or(0, |a| a.len());
                        (true, format!("Works · {n} models"))
                    }
                    Ok(v) => (
                        false,
                        v["message"]
                            .as_str()
                            .unwrap_or("Doesn't answer")
                            .to_string(),
                    ),
                    Err(e) => (false, format!("{e:#}")),
                });
                cx.notify();
            },
        );
    }

    pub fn make_endpoint_default(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.pv.endpoints[i].id.clone();
        self.rest_on(
            self.gear_agent.clone(),
            window,
            cx,
            move |conn, profile| rest::activate_endpoint(conn, profile, &id),
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

    /// Delete a provider: a form not saved yet just goes; a saved one goes on the second click.
    pub fn delete_endpoint(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.pv.endpoints[i].id.clone();
        if id.is_empty() {
            self.pv.endpoints.remove(i);
            cx.notify();
            return;
        }
        if self.pv.armed.as_deref() != Some(&id) {
            self.pv.armed = Some(id);
            cx.notify();
            return;
        }
        self.pv.armed = None;
        self.rest_on(
            self.gear_agent.clone(),
            window,
            cx,
            move |conn, profile| rest::delete_endpoint(conn, profile, &id),
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

    /// The fallback model: one (provider, model), or none.
    pub fn set_fallback(
        &mut self,
        pick: Option<(String, String)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let list = match &pick {
            Some((p, m)) => json!([{"provider": p, "model": m}]),
            None => json!([]),
        };
        self.pv.fallback = pick;
        cx.notify();
        self.rest_on(
            self.gear_agent.clone(),
            window,
            cx,
            move |conn, profile| {
                rest::merge_config(conn, profile, json!({"fallback_providers": list}))
            },
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

    pub fn save_voice(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(v) = &self.pv.voice else { return };
        let mut stt = json!({
            "base_url": text(&v.stt_url, cx),
            "model": text(&v.stt_model, cx),
            "language": text(&v.stt_lang, cx),
        });
        stt["api_key"] = json!(text(&v.stt_key, cx));
        let tts = json!({
            "base_url": text(&v.tts_url, cx),
            "model": text(&v.tts_model, cx),
            "voice": text(&v.tts_voice, cx),
        });
        let config = json!({
            "stt": {"provider": "openai", "openai": stt},
            "tts": {"provider": "openai", "openai": tts},
        });
        self.rest_on(
            self.gear_agent.clone(),
            window,
            cx,
            move |conn, profile| rest::merge_config(conn, profile, config),
            |this, r, window, cx| match r {
                Ok(()) => this.toast(NotificationType::Success, "Voice saved", window, cx),
                Err(e) => this.toast(NotificationType::Error, format!("Voice: {e:#}"), window, cx),
            },
        );
    }
}
