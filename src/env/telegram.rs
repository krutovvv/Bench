//! The project's own Telegram bot: its chats become the project's threads. A group of General:
//! the bot's token and who may write to it.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, h_flex};
use gpui_kit::*;
use serde_json::{Value, json};

use super::section::{SaveCall, Section, SettingsForm, form_actions};
use super::{ProjectEnv, on_change};
use crate::app::BenchApp;
use bench_ui::settings_field;

pub struct TelegramEnv {
    /// Its bot: (@name, allowed user ids, its token, its proxy URL).
    pub bot: Option<(String, Vec<String>, String, String)>,
    /// The agent's live view of the connection: state, detail (@name or the reason), the minute
    /// it was last heard. Nothing to show until a bot with a token exists.
    pub status: Option<(String, Option<String>, String)>,
    /// Set by the Check button: the save says `retry`, and the agent starts the bot anew.
    pub retry: bool,
    pub token: Entity<InputState>,
    pub users: Entity<InputState>,
    pub proxy: Entity<InputState>,
    pub dirty: bool,
}

impl TelegramEnv {
    pub(super) fn new(id: &str, window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let token =
            cx.new(|cx| InputState::new(window, cx).placeholder("Bot token from @BotFather"));
        let users = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Telegram user ids who may write to it, comma-separated")
        });
        let proxy = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("http://127.0.0.1:7890 — blank: the agent's own proxy settings")
        });
        on_change(&token, id, cx, |e| e.telegram.dirty = true);
        on_change(&users, id, cx, |e| e.telegram.dirty = true);
        on_change(&proxy, id, cx, |e| e.telegram.dirty = true);
        Self {
            bot: None,
            status: None,
            retry: false,
            token,
            users,
            proxy,
            dirty: false,
        }
    }
}

pub struct Telegram;

impl Section for Telegram {
    fn key(&self) -> &'static str {
        "telegram"
    }

    fn apply(&self, e: &mut ProjectEnv, v: &Value, window: &mut Window, cx: &mut App) {
        e.telegram.bot = v["telegram"].as_object().map(|t| {
            (
                t.get("username")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                t.get("allowed_users")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|u| u.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
                t.get("token")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                t.get("proxy")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            )
        });
        e.telegram.status = v["telegram"]["status"].as_object().map(|s| {
            let at = s
                .get("at")
                .and_then(Value::as_i64)
                .map(|ms| {
                    chrono::DateTime::from_timestamp_millis(ms)
                        .map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string())
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            (
                s.get("state")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                s.get("detail").and_then(Value::as_str).map(str::to_string),
                at,
            )
        });
        if !e.telegram.dirty {
            let users = e
                .telegram
                .bot
                .as_ref()
                .map(|t| t.1.join(", "))
                .unwrap_or_default();
            e.telegram
                .users
                .update(cx, |s, cx| s.set_value(users, window, cx));
            let token = e
                .telegram
                .bot
                .as_ref()
                .map(|t| t.2.clone())
                .unwrap_or_default();
            e.telegram
                .token
                .update(cx, |s, cx| s.set_value(token, window, cx));
            let proxy = e
                .telegram
                .bot
                .as_ref()
                .map(|t| t.3.clone())
                .unwrap_or_default();
            e.telegram
                .proxy
                .update(cx, |s, cx| s.set_value(proxy, window, cx));
        }
    }

    /// The field shows the saved token, so it is sent as it stands: blank removes the bot.
    fn save(
        &self,
        app: &mut BenchApp,
        id: &str,
        eid: &str,
        _window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<SaveCall> {
        let e = app.envs.get(id)?;
        let token = e.telegram.token.read(cx).value().trim().to_string();
        let users: Vec<String> = e
            .telegram
            .users
            .read(cx)
            .value()
            .split(',')
            .map(|u| u.trim().to_string())
            .filter(|u| !u.is_empty())
            .collect();
        let mut body = json!({ "allowed_users": users });
        // Check before the fields were filled sends no blank that would remove the bot.
        if e.telegram.dirty || !token.is_empty() {
            body["token"] = json!(token);
        }
        // Sent always: blank clears it, and the bot falls back to the agent's env again.
        body["proxy"] = json!(e.telegram.proxy.read(cx).value().trim());
        // The Check button: the same settings, but the agent starts the bot anew.
        if e.telegram.retry {
            body["retry"] = json!(true);
            if let Some(e) = app.envs.get_mut(id) {
                e.telegram.retry = false;
            }
        }
        Some(("PUT", format!("/api/projects/{eid}/telegram"), body))
    }

    fn saved(&self, e: &mut ProjectEnv) {
        e.telegram.dirty = false;
    }

    fn revert(
        &self,
        app: &mut BenchApp,
        id: &str,
        _: &mut Window,
        _: &mut Context<BenchApp>,
    ) -> bool {
        if let Some(e) = app.envs.get_mut(id) {
            e.telegram.dirty = false;
        }
        false
    }
}

impl SettingsForm for Telegram {
    fn block(&self, _general: bool) -> &'static str {
        "Telegram"
    }

    fn form(
        &self,
        _app: &BenchApp,
        e: &ProjectEnv,
        cx: &mut Context<BenchApp>,
    ) -> Option<Vec<AnyElement>> {
        let bot = match &e.telegram.bot {
            Some((name, _, token, _)) if !token.is_empty() && !name.is_empty() => {
                format!("@{name}: its chats become this project's threads")
            }
            Some((_, _, token, _)) if !token.is_empty() => {
                "Its chats become this project's threads".to_string()
            }
            _ => "No bot of its own yet: a token from @BotFather".to_string(),
        };
        let mut out = vec![
            settings_field(
                "Bot token",
                bot,
                Input::new(&e.telegram.token).into_any_element(),
                cx,
            )
            .into_any_element(),
            settings_field(
                "Who may write",
                "Telegram user ids, comma-separated",
                Input::new(&e.telegram.users).into_any_element(),
                cx,
            )
            .into_any_element(),
            settings_field(
                "Proxy",
                "A URL, http://host:port — for servers where Telegram is blocked; blank: the agent's own settings",
                Input::new(&e.telegram.proxy).into_any_element(),
                cx,
            )
            .into_any_element(),
        ];
        // The connection as the agent sees it, and a way to ask it again after a change —
        // a dead proxy says nothing until a timeout, so the state is dated.
        let theme = cx.theme();
        let pid = e.id.clone();
        let (dot, line) = match e
            .telegram
            .status
            .as_ref()
            .map(|(s, d, at)| (s.as_str(), d.clone(), at.clone()))
        {
            Some(("live", detail, at)) => (
                theme.success,
                format!(
                    "Connects{} · since {}",
                    detail.map(|d| format!(" {d}")).unwrap_or_default(),
                    at
                ),
            ),
            Some(("error", detail, at)) => (
                theme.danger,
                format!(
                    "Fails: {} · {}",
                    detail.unwrap_or_else(|| "unknown".into()),
                    at
                ),
            ),
            Some((_, _, _)) => (
                theme.warning,
                "Connecting… an unreachable proxy says nothing until it times out".into(),
            ),
            None => (
                theme.muted_foreground,
                "Not started since the agent last knew".into(),
            ),
        };
        out.push(
            settings_field(
                "Connection",
                "How the bot reaches Telegram now; Check starts it anew with the fields as they stand",
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(bench_ui::dot(dot))
                    .child(div().flex_1().min_w_0().child(line))
                    .child(
                        bench_ui::form_action(
                            SharedString::from(format!("env-tg-check-{pid}")),
                            "Check",
                            false,
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if let Some(env) = this.envs.get_mut(&pid) {
                                env.telegram.retry = true;
                            }
                            this.env_save(&pid, "telegram", window, cx);
                        })),
                    )
                    .into_any_element(),
                cx,
            )
            .into_any_element(),
        );
        out.extend(form_actions(&e.id, "telegram", e.telegram.dirty, cx));
        Some(out)
    }
}
