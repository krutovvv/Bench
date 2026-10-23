//! Settings → Providers and Messages, for the current machine and profile, through Hermes' own
//! API: custom (OpenAI-compatible) endpoints, OpenRouter with the models picked for the model
//! list, Claude on the subscription through the official plugin, the fallback model, voice
//! (speech to text, text to speech), service keys from `.env`, and Telegram.

use std::collections::HashMap;

use gpui_kit::component::input::InputState;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;
use serde_json::{Value, json};

use super::HermesApp;
use crate::connection::{self, CLAUDE_PROVIDER, ClaudeStatus};
use crate::rest::{self, ModelChoice};

/// Models the Claude plugin answers to (Claude Code's aliases: always the newest of each).
pub const CLAUDE_MODELS: [&str; 3] = ["sonnet", "opus", "haiku"];

/// Service keys shown even when not set: the ones Hermes' tools use most.
const COMMON_KEYS: [&str; 5] = [
    "TAVILY_API_KEY",
    "BRAVE_SEARCH_API_KEY",
    "GITHUB_TOKEN",
    "FIRECRAWL_API_KEY",
    "ELEVENLABS_API_KEY",
];

/// One `.env` value with its field: what it is, a masked preview of the saved value.
pub struct KeyField {
    pub key: String,
    pub hint: String,
    pub preview: Option<String>,
    pub input: Entity<InputState>,
}

/// A custom endpoint's form.
pub struct EndpointForm {
    /// "" for one not saved yet.
    pub id: String,
    pub current: bool,
    pub key_preview: Option<String>,
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

pub struct Telegram {
    pub token: KeyField,
    pub users: Entity<InputState>,
    pub proxy: Entity<InputState>,
}

/// What Bench itself remembers per machine and profile: the OpenRouter models picked for the
/// model list, and whether the Claude plugin is there.
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct PickerPrefs {
    #[serde(default)]
    pub openrouter: Vec<String>,
    #[serde(default)]
    pub claude: bool,
}

#[derive(Default)]
pub struct Providers {
    pub loaded: bool,
    pub endpoints: Vec<EndpointForm>,
    pub keys: Vec<KeyField>,
    pub openrouter_key: Option<KeyField>,
    pub voice: Option<Voice>,
    pub telegram: Option<Telegram>,
    pub fallback: Option<(String, String)>,
    pub prefs: PickerPrefs,
    pub or_search: Option<Entity<InputState>>,
    pub or_query: String,
    /// OpenRouter's whole model list (ids), fetched once when needed.
    pub or_catalog: Vec<String>,
    pub or_loading: bool,
    pub claude: Option<ClaudeStatus>,
    pub claude_busy: Option<String>,
    /// A key armed for removal (the second click removes it).
    pub armed: Option<String>,
    _subscriptions: Vec<Subscription>,
}

fn field(
    window: &mut Window,
    cx: &mut Context<HermesApp>,
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

fn secret(
    window: &mut Window,
    cx: &mut Context<HermesApp>,
    preview: Option<&str>,
) -> Entity<InputState> {
    let hint = if preview.is_some() {
        "Saved · type to replace"
    } else {
        "Not set"
    };
    cx.new(|cx| InputState::new(window, cx).placeholder(hint).masked(true))
}

/// `«redacted:sk-o…1234»` → `sk-o…1234`.
fn preview(row: &Value) -> Option<String> {
    row["redacted_value"].as_str().map(|v| {
        v.trim_start_matches("«redacted:")
            .trim_end_matches('»')
            .to_string()
    })
}

fn text(input: &Entity<InputState>, cx: &App) -> String {
    input.read(cx).value().trim().to_string()
}

impl HermesApp {
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

    /// After the profile's own models: the OpenRouter models picked here and Claude's, then
    /// each provider's models kept together (the main model's group first).
    pub(super) fn add_picker_models(&mut self) {
        self.pv.prefs = std::fs::read_to_string(self.picker_prefs_path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        let mut extra: Vec<ModelChoice> = self
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
        if self.pv.prefs.claude {
            extra.extend(CLAUDE_MODELS.iter().map(|m| ModelChoice {
                model: m.to_string(),
                provider: CLAUDE_PROVIDER.into(),
                group: "Claude".into(),
                tag: "",
            }));
        }
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

    /// Fill both tabs from the gateway (and Claude's state from the machine).
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
            |conn, profile| {
                let endpoints = rest::custom_endpoints(conn, profile)?;
                let env = rest::env_vars(conn, profile)?;
                let yaml: Value =
                    serde_yaml::from_str(&rest::config_raw(conn, profile)?).unwrap_or(Value::Null);
                let mut shown = HashMap::new();
                for key in ["TELEGRAM_ALLOWED_USERS", "TELEGRAM_PROXY"] {
                    if env[key]["is_set"].as_bool() == Some(true) {
                        if let Ok(Some(v)) = rest::reveal_env(conn, profile, key) {
                            shown.insert(key, v);
                        }
                    }
                }
                Ok((endpoints, env, yaml, shown))
            },
            |this, r, window, cx| match r {
                Ok((endpoints, env, yaml, shown)) => {
                    this.fill_providers(endpoints, env, yaml, shown, window, cx)
                }
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Providers: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
        self.check_claude(window, cx);
    }

    fn fill_providers(
        &mut self,
        endpoints: Value,
        env: Value,
        yaml: Value,
        shown: HashMap<&'static str, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
                let key_preview = e["api_key_preview"].as_str().map(str::to_string);
                EndpointForm {
                    id: s("id"),
                    current: e["is_current"].as_bool().unwrap_or(false),
                    name: field(window, cx, &s("name"), "Name"),
                    url: field(window, cx, &s("base_url"), "http://host:port/v1"),
                    model: field(window, cx, &s("model"), "Default model"),
                    context: field(window, cx, &context, "e.g. 131072"),
                    key: secret(window, cx, key_preview.as_deref()),
                    key_preview,
                    check: None,
                }
            })
            .collect();

        let row = |key: &str| &env[key];
        let key_field = |key: &str, window: &mut Window, cx: &mut Context<Self>| {
            let r = row(key);
            let p = preview(r);
            KeyField {
                key: key.to_string(),
                hint: r["description"].as_str().unwrap_or_default().to_string(),
                input: secret(window, cx, p.as_deref()),
                preview: p,
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
                    // Model endpoints' keys belong to their cards; the API server's is Hermes' own.
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
            stt_key: secret(
                window,
                cx,
                Some(&y(&["stt", "openai", "api_key"]))
                    .filter(|k| !k.is_empty())
                    .map(|_| "set"),
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

        let token = key_field("TELEGRAM_BOT_TOKEN", window, cx);
        let users = shown
            .get("TELEGRAM_ALLOWED_USERS")
            .cloned()
            .unwrap_or_default();
        let proxy = shown.get("TELEGRAM_PROXY").cloned().unwrap_or_default();
        self.pv.telegram = Some(Telegram {
            token,
            users: field(window, cx, &users, "Numeric user IDs, comma-separated"),
            proxy: field(window, cx, &proxy, "socks5://host:port or http://host:port"),
        });
        self.pv.loaded = true;
        cx.notify();
    }

    pub fn check_claude(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.target.clone();
        self.pv.claude_busy = Some("Checking…".into());
        self.background(
            window,
            cx,
            move || Ok(connection::claude_status(&target)),
            |this, r: anyhow::Result<ClaudeStatus>, _, cx| {
                this.pv.claude_busy = None;
                if let Ok(s) = r {
                    if s.plugin != this.pv.prefs.claude {
                        this.pv.prefs.claude = s.plugin;
                        this.save_picker_prefs();
                        this.profile_models
                            .retain(|c| c.provider != CLAUDE_PROVIDER);
                        this.add_picker_models();
                    }
                    this.pv.claude = Some(s);
                }
                cx.notify();
            },
        );
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
            key_preview: None,
            name: field(window, cx, "", "Name, e.g. local"),
            url: field(window, cx, "", "http://host:port/v1"),
            model: field(window, cx, "", "Default model"),
            context: field(window, cx, "", "e.g. 131072"),
            key: secret(window, cx, None),
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
        let key = text(&e.key, cx);
        if !key.is_empty() {
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

    /// Save a key typed into its field (an empty field changes nothing).
    pub fn save_key(
        &mut self,
        key: String,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if value.is_empty() {
            return;
        }
        self.rest(
            window,
            cx,
            move |conn, profile| rest::set_env(conn, profile, &key, &value),
            |this, r, window, cx| match r {
                Ok(()) => {
                    this.toast(NotificationType::Success, "Saved", window, cx);
                    this.load_providers(window, cx);
                }
                Err(e) => this.toast(NotificationType::Error, format!("Key: {e:#}"), window, cx),
            },
        );
    }

    /// Remove a key: the first click arms, the second removes.
    pub fn remove_key(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.pv.armed.as_deref() != Some(&key) {
            self.pv.armed = Some(key);
            cx.notify();
            return;
        }
        self.pv.armed = None;
        self.rest(
            window,
            cx,
            move |conn, profile| rest::remove_env(conn, profile, &key),
            |this, r, window, cx| match r {
                Ok(()) => this.load_providers(window, cx),
                Err(e) => this.toast(NotificationType::Error, format!("Key: {e:#}"), window, cx),
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
        let key = text(&v.stt_key, cx);
        if !key.is_empty() {
            stt["api_key"] = json!(key);
        }
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

    pub fn save_telegram(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(t) = &self.pv.telegram else { return };
        let token = text(&t.token.input, cx);
        let users: Vec<String> = text(&t.users, cx)
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        if let Some(bad) = users
            .iter()
            .find(|u| !u.chars().all(|c| c.is_ascii_digit()))
        {
            self.toast(
                NotificationType::Error,
                format!("Telegram: \"{bad}\" is not a numeric user ID"),
                window,
                cx,
            );
            return;
        }
        let users = users.join(",");
        let proxy = text(&t.proxy, cx);
        self.rest(
            window,
            cx,
            move |conn, profile| {
                if !token.is_empty() {
                    rest::set_env(conn, profile, "TELEGRAM_BOT_TOKEN", &token)?;
                }
                rest::set_env(conn, profile, "TELEGRAM_ALLOWED_USERS", &users)?;
                if proxy.is_empty() {
                    let _ = rest::remove_env(conn, profile, "TELEGRAM_PROXY");
                } else {
                    rest::set_env(conn, profile, "TELEGRAM_PROXY", &proxy)?;
                }
                Ok(())
            },
            |this, r, window, cx| match r {
                Ok(()) => {
                    this.toast(
                        NotificationType::Success,
                        "Telegram saved · restart the gateway to apply",
                        window,
                        cx,
                    );
                    this.load_providers(window, cx);
                }
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Telegram: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
    }

    pub fn install_claude_plugin(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.target.clone();
        self.pv.claude_busy = Some("Installing the plugin…".into());
        cx.notify();
        self.background(
            window,
            cx,
            move || connection::install_claude_plugin(&target),
            |this, r, window, cx| {
                this.pv.claude_busy = None;
                match r {
                    Ok(_) => {
                        this.toast(
                            NotificationType::Success,
                            "Claude plugin installed · restart the gateway to load it",
                            window,
                            cx,
                        );
                        this.check_claude(window, cx);
                    }
                    Err(e) => this.toast(
                        NotificationType::Error,
                        format!("Claude: {e:#}"),
                        window,
                        cx,
                    ),
                }
            },
        );
    }
}
