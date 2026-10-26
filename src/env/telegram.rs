//! The project's own Telegram bot: its chats become the project's threads. A row of the
//! project's block; its card takes the bot's token and who may write to it.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::*;
use serde_json::{Value, json};

use super::section::{Card, Request, Section, pad, project_block, revert, row, save};
use super::{ProjectEnv, on_change};
use crate::app::BenchApp;

pub struct TelegramEnv {
    /// Its bot: (@name, allowed user ids, its token).
    pub bot: Option<(String, Vec<String>, String)>,
    pub open: bool,
    pub token: Entity<InputState>,
    pub users: Entity<InputState>,
    pub dirty: bool,
}

impl TelegramEnv {
    pub(super) fn new(window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let token =
            cx.new(|cx| InputState::new(window, cx).placeholder("Bot token from @BotFather"));
        let users = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Telegram user ids who may write to it, comma-separated")
        });
        on_change(&token, cx, |e| e.telegram.dirty = true);
        on_change(&users, cx, |e| e.telegram.dirty = true);
        Self {
            bot: None,
            open: false,
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
    fn block(&self, general: bool) -> &'static str {
        project_block(general)
    }
    fn rows(&self, app: &BenchApp, cx: &mut Context<BenchApp>) -> Vec<AnyElement> {
        let Some(e) = app.project_env.as_ref() else {
            return vec![];
        };
        let note = match &e.telegram.bot {
            Some((name, users, token)) if !token.is_empty() => format!(
                "{} · {} may write",
                if name.is_empty() {
                    "a bot".to_string()
                } else {
                    format!("@{name}")
                },
                users.len()
            ),
            _ => "no bot of its own".into(),
        };
        vec![row(
            e,
            "telegram",
            String::new(),
            "Telegram".into(),
            note,
            false,
            false,
            cx,
        )]
    }
    fn card(&self, app: &BenchApp, _name: &str, cx: &mut Context<BenchApp>) -> Option<Card> {
        let e = app.project_env.as_ref()?;
        let muted = cx.theme().muted_foreground;
        let mut actions = vec![save("telegram", e.telegram.dirty, cx)];
        actions.extend(revert("telegram", e.telegram.dirty, cx));
        Some(Card::new(
            "Telegram",
            pad(v_flex()
                .child(Input::new(&e.telegram.token))
                .child(Input::new(&e.telegram.users))
                .child(div().text_sm().text_color(muted).child(
                    "Its chats become this project's threads, with its soul, skills and servers.",
                ))),
            actions,
        ))
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
    fn close(&self, e: &mut ProjectEnv) {
        e.telegram.open = false;
    }
    fn open(&self, app: &mut BenchApp, _: String, _: &mut Window, _: &mut Context<BenchApp>) {
        if let Some(e) = app.project_env.as_mut() {
            e.telegram.open = true;
        }
    }
    /// An empty token keeps the saved one.
    fn save(
        &self,
        app: &mut BenchApp,
        _id: &str,
        eid: &str,
        _window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<Request> {
        let e = app.project_env.as_ref()?;
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
    fn revert(&self, app: &mut BenchApp, _: &mut Window, _: &mut Context<BenchApp>) -> bool {
        if let Some(e) = app.project_env.as_mut() {
            e.telegram.dirty = false;
        }
        false
    }
}
