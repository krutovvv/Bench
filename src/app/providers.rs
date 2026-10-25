//! Settings → Providers and Messages, for the current machine and profile, through the agent's
//! API: custom (OpenAI-compatible) endpoints, OpenRouter with the models picked for the model
//! list, the fallback model, voice (speech to text, text to speech), service keys from `.env`,
//! and Telegram.

use std::collections::HashMap;

use gpui_kit::component::input::InputState;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;
use serde_json::{Value, json};

use super::BenchApp;
use crate::connection;
use crate::rest::{self, ModelChoice};

/// Service keys shown even when not set: the ones the agent's tools use most.
const COMMON_KEYS: [&str; 5] = [
    "TAVILY_API_KEY",
    "BRAVE_SEARCH_API_KEY",
    "GITHUB_TOKEN",
    "FIRECRAWL_API_KEY",
    "ELEVENLABS_API_KEY",
];

/// One `.env` value with its field: what it is and the value as the agent shows it (redacted:
/// it says the key is set). Saving an unchanged field does nothing; an emptied one removes the key.
pub struct KeyField {
    pub key: String,
    pub hint: String,
    pub value: String,
    pub input: Entity<InputState>,
}

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

/// What Bench itself remembers per machine and profile: the OpenRouter models picked for the
/// model list.
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct PickerPrefs {
    #[serde(default)]
    pub openrouter: Vec<String>,
}

#[derive(Default)]
pub struct Providers {
    pub loaded: bool,
    pub endpoints: Vec<EndpointForm>,
    pub keys: Vec<KeyField>,
    pub openrouter_key: Option<KeyField>,
    pub voice: Option<Voice>,
    pub fallback: Option<(String, String)>,
    pub prefs: PickerPrefs,
    pub or_search: Option<Entity<InputState>>,
    pub or_query: String,
    /// OpenRouter's whole model list (ids), fetched once when needed.
    pub or_catalog: Vec<String>,
    pub or_loading: bool,
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
    fn picker_prefs_path(&self) -> std::path::PathBuf {
        let key: String = format!("{}-{}", self.target.label(), self.current_profile())
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        connection::state_dir().join(format!("models-{key}.json"))
    }

    fn save_picker_prefs(&self) {
        let _ = std::fs::create_dir_all(connection::state_dir());
        if let Ok(text) = serde_json::to_string_pretty(&self.pv.prefs) {
            let _ = std::fs::write(self.picker_prefs_path(), text);
        }
    }

    /// After the profile's own models: the OpenRouter models picked here, then
    /// each provider's models kept together (the main model's group first).
    pub(super) fn add_picker_models(&mut self) {
        self.pv.prefs = std::fs::read_to_string(self.picker_prefs_path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        let extra: Vec<ModelChoice> = self
            .pv
            .prefs
            .openrouter
            .iter()
            .map(|m| ModelChoice {
                model: m.clone(),
                provider: "openrouter".into(),
                group: "OpenRouter".into(),
                tag: "",
            })
            .collect();
        for c in extra {
            if !self
                .profile_models
                .iter()
                .any(|x| x.model == c.model && x.provider == c.provider)
            {
                self.profile_models.push(c);
            }
        }
        let mut order: Vec<String> = vec![];
        for c in &self.profile_models {
            if !order.contains(&c.group) {
                order.push(c.group.clone());
            }
        }
        // A stable sort: inside a group the order stays (main, fallbacks, the rest).
        self.profile_models
            .sort_by_key(|c| order.iter().position(|g| *g == c.group));
    }

    // ---- loading ----------------------------------------------------------------------

    /// Fill both tabs from the agent.
    pub(super) fn load_providers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pv.or_search.is_none() {
            let search =
                cx.new(|cx| InputState::new(window, cx).placeholder("Search OpenRouter models"));
            let sub = cx.subscribe_in(
                &search,
                window,
                |this, s, ev: &gpui_kit::component::input::InputEvent, _, cx| {
                    if let gpui_kit::component::input::InputEvent::Change = ev {
                        this.pv.or_query = s.read(cx).value().trim().to_lowercase();
                        cx.notify();
                    }
                },
            );
            self.pv.or_search = Some(search);
            self.pv._subscriptions.push(sub);
        }
        self.rest(
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
        // Ahead of the search field's first use, so nothing appears and vanishes under it.
        self.load_openrouter_catalog(window, cx);
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
                // `${NAME}`: the key lives in .env under NAME.
                let preview = s("api_key_preview");
                let key_env = preview
                    .strip_prefix("${")
                    .and_then(|p| p.strip_suffix('}'))
                    .map(str::to_string);
                let key_value = key_env
                    .as_ref()
                    .and_then(|k| shown.get(k).cloned())
                    .unwrap_or_default();
                EndpointForm {
                    id: s("id"),
                    current: e["is_current"].as_bool().unwrap_or(false),
                    name: field(window, cx, &s("name"), "Name"),
                    url: field(window, cx, &s("base_url"), "http://host:port/v1"),
                    model: field(window, cx, &s("model"), "Default model"),
                    context: field(window, cx, &context, "e.g. 131072"),
                    key: field(window, cx, &key_value, "If the server asks for one"),
                    key_value,
                    key_env,
                    check: None,
                }
            })
            .collect();

        let key_field = |key: &str, window: &mut Window, cx: &mut Context<Self>| {
            let value = shown.get(key).cloned().unwrap_or_default();
            KeyField {
                key: key.to_string(),
                hint: env[key]["description"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                input: field(window, cx, &value, "Not set"),
                value,
            }
        };
        // Set keys first, then the common ones not set yet. Not here: model provider keys
        // (their cards have them), Telegram (Messages tab), anything a channel page owns.
        let endpoint_keys: Vec<String> = yaml["providers"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(_, e)| e["key_env"].as_str().map(str::to_string))
            .collect();
        let mut names: Vec<String> = env
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(k, v)| {
                v["is_set"].as_bool() == Some(true)
                    && v["channel_managed"].as_bool() != Some(true)
                    && !k.starts_with("TELEGRAM_")
                    && *k != "OPENROUTER_API_KEY"
                    && !endpoint_keys.contains(k)
                    && v["category"].as_str() != Some("provider")
                    // Keys and tokens only: .env also holds plain settings (timeouts, flags).
                    && ["_KEY", "_TOKEN", "_SECRET"].iter().any(|e| k.ends_with(e))
                    // Model endpoints' keys belong to their cards; the API server's is the agent's own.
                    && !k.contains("CUSTOM_")
                    && *k != "API_SERVER_KEY"
            })
            .map(|(k, _)| k.clone())
            .collect();
        names.sort();
        for k in COMMON_KEYS {
            if !names.iter().any(|n| n == k) {
                names.push(k.to_string());
            }
        }
        self.pv.keys = names.iter().map(|k| key_field(k, window, cx)).collect();
        self.pv.openrouter_key = Some(key_field("OPENROUTER_API_KEY", window, cx));

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
        self.load_profile_effort(window, cx);
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
        self.rest(
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
        self.rest(
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
        self.rest(
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

    /// Save a key as its field shows it: a changed value is written, an emptied field removes
    /// the key, an unchanged one does nothing.
    pub fn save_key(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        let field = self
            .pv
            .keys
            .iter()
            .chain(self.pv.openrouter_key.iter())
            .find(|f| f.key == key);
        let Some(f) = field else { return };
        let value = text(&f.input, cx);
        if value == f.value {
            return;
        }
        self.rest(
            window,
            cx,
            move |conn, profile| {
                if value.is_empty() {
                    rest::remove_env(conn, profile, &key)
                } else {
                    rest::set_env(conn, profile, &key, &value)
                }
            },
            |this, r, window, cx| match r {
                Ok(()) => {
                    this.toast(NotificationType::Success, "Saved", window, cx);
                    this.load_providers(window, cx);
                }
                Err(e) => this.toast(NotificationType::Error, format!("Key: {e:#}"), window, cx),
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
        self.rest(
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

    pub fn toggle_openrouter_model(&mut self, model: String, cx: &mut Context<Self>) {
        let list = &mut self.pv.prefs.openrouter;
        if let Some(i) = list.iter().position(|m| *m == model) {
            list.remove(i);
        } else {
            list.push(model);
        }
        self.save_picker_prefs();
        self.profile_models
            .retain(|c| c.provider != "openrouter" || c.tag != "");
        self.add_picker_models();
        cx.notify();
    }

    /// OpenRouter's model list (public, no key needed), once.
    pub fn load_openrouter_catalog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pv.or_loading || !self.pv.or_catalog.is_empty() {
            return;
        }
        self.pv.or_loading = true;
        self.background(
            window,
            cx,
            || {
                let v: Value = ureq::get("https://openrouter.ai/api/v1/models")
                    .timeout(std::time::Duration::from_secs(20))
                    .call()?
                    .into_json()?;
                Ok(v["data"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|m| m["id"].as_str().map(str::to_string))
                    .collect::<Vec<_>>())
            },
            |this, r: anyhow::Result<Vec<String>>, window, cx| {
                this.pv.or_loading = false;
                match r {
                    Ok(mut ids) => {
                        ids.sort();
                        this.pv.or_catalog = ids;
                    }
                    Err(e) => this.toast(
                        NotificationType::Error,
                        format!("OpenRouter: {e:#}"),
                        window,
                        cx,
                    ),
                }
                cx.notify();
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
        self.rest(
            window,
            cx,
            move |conn, profile| {
                rest::merge_config(conn, profile, json!({"fallback_providers": list}))
            },
            |this, r, window, cx| match r {
                Ok(()) => this.load_profile_effort(window, cx),
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
        self.rest(
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
