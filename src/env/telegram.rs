//! The project's own Telegram bot: its chats become the project's threads. A group of General:
//! the bot's token and who may write to it.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::*;
use serde_json::{Value, json};

use super::section::{SettingsForm, Request, Section, form_actions};
use super::{ProjectEnv, on_change};
use crate::app::BenchApp;
use bench_ui::settings_field;

pub struct TelegramEnv {
    /// Its bot: (@name, allowed user ids, its token).
    pub bot: Option<(String, Vec<String>, String)>,
    pub token: Entity<InputState>,
    pub users: Entity<InputState>,
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
        on_change(&token, id, cx, |e| e.telegram.dirty = true);
        on_change(&users, id, cx, |e| e.telegram.dirty = true);
        Self {
            bot: None,
            token,
            users,
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
        }
    }

    /// An empty token keeps the saved one.
    fn save(
        &self,
        app: &mut BenchApp,
        id: &str,
        eid: &str,
        _window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<Request> {
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
        let mut body = json!({"allowed_users": users});
        if !token.is_empty() {
            body["token"] = json!(token);
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
            Some((name, _, token)) if !token.is_empty() && !name.is_empty() => {
                format!("@{name}: its chats become this project's threads")
            }
            Some((_, _, token)) if !token.is_empty() => {
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
        ];
        out.extend(form_actions(&e.id, "telegram", e.telegram.dirty, cx));
        Some(out)
    }
}
