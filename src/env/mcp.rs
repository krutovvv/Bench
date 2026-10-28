//! The project's MCP servers (`mcp.json`): a row each with a sign of what the agent knows of it,
//! Check and "+ Add" in the block's head; a card is the server's entry as JSON.

use gpui_kit::component::input::EditorState;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};

use super::section::{
    Card, ExplorerParts, Item, Request, Section, add, card_act, code, head_act, new_card, remove,
    revert, save,
};
use super::{ProjectEnv, editor, enc, on_change};
use crate::app::BenchApp;
use crate::client::rest;
use crate::env::mcp_status::{McpSet, McpTone, mcp_note, mcp_tally, mcp_tone};

/// One MCP server of the project. `checked` is the field that matters most: the agent only knows
/// a server once something has asked for its tools, so without it "no answer yet" and "answers
/// fine" look the same in the block.
#[derive(Clone, Debug)]
pub struct McpInfo {
    pub name: String,
    /// "http" or "stdio".
    pub kind: String,
    pub enabled: bool,
    /// The agent tried it at least once (a thread called it, or the person pressed Check).
    pub checked: bool,
    /// Its handshake is live now.
    pub connected: bool,
    pub tools: u64,
    pub error: Option<String>,
    /// It asks to sign in (OAuth) before it answers.
    pub needs_auth: bool,
    /// Its entry in mcp.json, as JSON text.
    pub spec: String,
}

pub struct McpEnv {
    /// What each is, and what the agent knows of it.
    pub servers: Vec<McpInfo>,
    /// A check of them is in flight (the agent is shaking hands with each).
    pub checking: bool,
    /// The servers that asked for OAuth: "Authorize" signs in.
    pub auth: std::collections::HashSet<String>,
    /// The server opened under its row, and its config being edited.
    pub open: Option<String>,
    pub json: Entity<EditorState>,
    pub dirty: bool,
}

impl McpEnv {
    pub(super) fn new(id: &str, window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let json = editor("json", window, cx);
        on_change(&json, id, cx, |e| e.mcp.dirty = true);
        Self {
            servers: vec![],
            checking: false,
            auth: Default::default(),
            open: None,
            json,
            dirty: false,
        }
    }
}

/// The MCP part of the agent's answer: the whole environment read and a Check both carry the same
/// `mcp` array, so one parser serves them.
pub fn apply_mcp(e: &mut ProjectEnv, v: &Value) {
    e.mcp.servers = v["mcp"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|m| McpInfo {
            name: m["name"].as_str().unwrap_or_default().to_string(),
            kind: m["kind"].as_str().unwrap_or_default().to_string(),
            enabled: m["enabled"].as_bool().unwrap_or(true),
            checked: m["checked"].as_bool().unwrap_or(false),
            connected: m["connected"].as_bool().unwrap_or(false),
            tools: m["tools"].as_u64().unwrap_or(0),
            error: m["error"].as_str().map(str::to_string),
            needs_auth: m["needs_auth"].as_bool().unwrap_or(false),
            spec: m["spec"].as_str().unwrap_or("{}").to_string(),
        })
        .collect();
    e.mcp.auth = e
        .mcp
        .servers
        .iter()
        .filter(|m| m.needs_auth)
        .map(|m| m.name.clone())
        .collect();
}

pub struct Mcp;

impl Section for Mcp {
    fn key(&self) -> &'static str {
        "mcp"
    }

    fn apply(&self, e: &mut ProjectEnv, v: &Value, _window: &mut Window, _cx: &mut App) {
        apply_mcp(e, v);
    }

    fn save(
        &self,
        app: &mut BenchApp,
        id: &str,
        eid: &str,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<Request> {
        let e = app.envs.get(id)?;
        let mut name = e.mcp.open.clone()?;
        if name == "new" {
            name = e.new_name.read(cx).value().trim().to_string();
            if name.is_empty() {
                app.toast(NotificationType::Error, "Name it first", window, cx);
                return None;
            }
        }
        Some((
            "PUT",
            format!("/api/projects/{eid}/mcp/{}/spec", enc(&name)),
            json!({"text": e.mcp.json.read(cx).value().to_string()}),
        ))
    }

    fn saved(&self, e: &mut ProjectEnv) {
        e.mcp.dirty = false;
    }

    fn revert(
        &self,
        app: &mut BenchApp,
        id: &str,
        _: &mut Window,
        _: &mut Context<BenchApp>,
    ) -> bool {
        if let Some(e) = app.envs.get_mut(id) {
            e.mcp.dirty = false;
        }
        false
    }
}

impl ExplorerParts for Mcp {
    fn group(&self) -> &'static str {
        "Tools"
    }

    fn block(&self, _general: bool) -> &'static str {
        "MCP servers"
    }

    /// The head says how the set stands, so a block of unchecked servers cannot read as calm.
    /// Asking is one action for the project, in the head, because a check reconnects the
    /// project's servers together, not one at a time.
    fn head(
        &self,
        _app: &BenchApp,
        e: &ProjectEnv,
        cx: &mut Context<BenchApp>,
    ) -> Option<AnyElement> {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let checking = e.mcp.checking;
        let mut head: Vec<AnyElement> = Vec::new();
        if !e.mcp.servers.is_empty() {
            let (state, text) = mcp_tally(&e.mcp.servers, checking);
            let (ico, color) = match state {
                McpSet::Checking => (bench_ui::icons::Ico::Loader, muted),
                McpSet::Trouble => (bench_ui::icons::Ico::CircleAlert, theme.danger),
                McpSet::Untested => (bench_ui::icons::Ico::CircleEmpty, muted),
                McpSet::AllOff => (bench_ui::icons::Ico::CircleEmpty, muted),
                McpSet::Answering => (bench_ui::icons::Ico::CircleCheck, theme.success),
            };
            let sign = if state == McpSet::Checking {
                bench_ui::icons::Ico::Loader.live(true)
            } else {
                div()
                    .flex()
                    .items_center()
                    .child(ico.icon().small().text_color(color))
                    .into_any_element()
            };
            // The tally itself on hover: the heading sits over a narrow list.
            head.push(
                div()
                    .id("env-mcp-tally")
                    .flex()
                    .items_center()
                    .child(sign)
                    .tooltip(move |window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(text.clone()).build(window, cx)
                    })
                    .into_any_element(),
            );
        }
        let id = e.id.clone();
        head.push(
            head_act("env-mcp-check".into(), "Check", cx)
                .when(checking, |d| d.opacity(0.5))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.click_claimed = true;
                    cx.stop_propagation();
                    this.env_check_mcp(id.clone(), window, cx)
                }))
                .into_any_element(),
        );
        head.push(add("env-mcp-add", &e.id, "mcp", cx).into_any_element());
        Some(
            h_flex()
                .gap_3()
                .items_center()
                .children(head)
                .into_any_element(),
        )
    }

    /// Each server carries a sign of what the agent knows of it. A server says nothing until
    /// something asks it for its tools, so "not tried yet" is its own state and says so out
    /// loud — it is not a working server.
    fn items(&self, _app: &BenchApp, e: &ProjectEnv, cx: &mut Context<BenchApp>) -> Vec<Item> {
        let theme = cx.theme().clone();
        e.mcp
            .servers
            .iter()
            .map(|m| {
                let icon = match mcp_tone(m) {
                    McpTone::Live => (bench_ui::icons::Ico::CircleCheck, theme.success),
                    McpTone::Error => (bench_ui::icons::Ico::CircleAlert, theme.danger),
                    McpTone::Dead => (bench_ui::icons::Ico::CircleHelp, theme.danger),
                    McpTone::Auth => (bench_ui::icons::Ico::CircleAlert, theme.warning),
                    McpTone::Off | McpTone::Untested => {
                        (bench_ui::icons::Ico::CircleEmpty, theme.muted_foreground)
                    }
                };
                Item {
                    name: m.name.clone(),
                    label: m.name.clone(),
                    note: mcp_note(m),
                    icon,
                }
            })
            .collect()
    }

    fn card(
        &self,
        app: &BenchApp,
        e: &ProjectEnv,
        name: &str,
        cx: &mut Context<BenchApp>,
    ) -> Option<Card> {
        if name == "new" {
            let mut card = new_card(
                e,
                "mcp",
                "New MCP server: its name, and its config (Pi's format: {\"url\", \"headers\"} or {\"command\", \"args\", \"env\"})",
                &e.mcp.json,
                true,
                cx,
            );
            // The person's presets above the form (the gear's Presets tab): one fills it in —
            // its name and config, keys and all — to look at and Save; or write one by hand.
            let have: Vec<&str> = e.mcp.servers.iter().map(|m| m.name.as_str()).collect();
            let theme = cx.theme().clone();
            let chips = app
                .gear.presets
                .mcp
                .iter()
                .filter(|(n, _, _)| !have.contains(&n.as_str()))
                .map(|(n, title, spec)| {
                    let (n, spec, id) = (n.clone(), spec.clone(), e.id.clone());
                    div()
                        .id(SharedString::from(format!("env-mcp-preset-{n}")))
                        .px_2()
                        .py_0p5()
                        .rounded_md()
                        .border_1()
                        .border_color(theme.border)
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.list_hover))
                        .child(title.clone())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.click_claimed = true;
                            cx.stop_propagation();
                            this.env_fill_mcp(&id, n.clone(), &spec, window, cx)
                        }))
                        .into_any_element()
                });
            let body = std::mem::replace(&mut card.body, div().into_any_element());
            card.body = v_flex()
                .child(h_flex().flex_wrap().gap_1p5().px_3().pt_2().children(chips))
                .child(body)
                .into_any_element();
            return Some(card);
        }
        if !e.mcp.servers.iter().any(|m| m.name == name) {
            return None;
        }
        let mut actions = vec![];
        if e.mcp.auth.contains(name) {
            let (n, id) = (name.to_string(), e.id.clone());
            actions.push(
                card_act(
                    "env-card-auth".into(),
                    "Authorize".into(),
                    cx.theme().primary,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.click_claimed = true;
                    this.env_authorize(id.clone(), n.clone(), window, cx)
                }))
                .into_any_element(),
            );
        }
        actions.push(save(&e.id, "mcp", e.mcp.dirty, cx));
        actions.extend(revert(&e.id, "mcp", e.mcp.dirty, cx));
        actions.push(remove(app, e, "mcp", name.to_string(), "Remove", cx));
        Some(Card::new(
            format!("MCP server · {name}"),
            code(&e.mcp.json, cx),
            actions,
        ))
    }

    fn dirty(&self, e: &ProjectEnv) -> bool {
        e.mcp.open.is_some() && e.mcp.dirty
    }

    fn close(&self, e: &mut ProjectEnv) {
        e.mcp.open = None;
        e.mcp.dirty = false;
    }

    fn open(
        &self,
        app: &mut BenchApp,
        id: &str,
        name: String,
        w: &mut Window,
        cx: &mut Context<BenchApp>,
    ) {
        app.env_open_mcp(id, name, w, cx);
    }

    fn new_one(&self, e: &mut ProjectEnv) -> Option<(Entity<EditorState>, &'static str)> {
        e.mcp.open = Some("new".into());
        Some((e.mcp.json.clone(), "{\n  \"url\": \"https://\"\n}\n"))
    }

    /// A new server, saved, is its name's part.
    fn made(&self, e: &ProjectEnv, cx: &App) -> Option<String> {
        Some(e.new_name.read(cx).value().trim().to_string())
    }

    fn remove_path(&self, eid: &str, name: &str) -> Option<String> {
        Some(format!("/api/projects/{eid}/mcp/{}", enc(name)))
    }
}

impl BenchApp {
    /// A server's row in the project `id`: its config opens to edit (again: closes).
    pub fn env_open_mcp(
        &mut self,
        id: &str,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(e) = self.envs.get_mut(id) else {
            return;
        };
        if e.mcp.open.as_deref() == Some(&name) && !e.mcp.dirty {
            e.mcp.open = None;
            cx.notify();
            return;
        }
        e.mcp.open = Some(name.clone());
        e.mcp.dirty = false;
        let spec = e
            .mcp
            .servers
            .iter()
            .find(|m| m.name == name)
            .map(|m| m.spec.clone())
            .unwrap_or_default();
        e.mcp.json.update(cx, |s, cx| s.set_value(spec, window, cx));
        cx.notify();
    }

    /// Ask the project's MCP servers now: the agent shakes hands with every one of them and the
    /// rows say what came back. A server only tells you what it is when you ask it, so this is
    /// the difference between "unknown" and "working"; the agent answers when the slowest has had
    /// its say, so this is the one environment job allowed to take a minute.
    pub fn env_check_mcp(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        // One check at a time: a second reconnect while the first is in flight would answer out
        // of order and show a stale set.
        if self.envs.get(&id).is_none_or(|e| e.mcp.checking) {
            return;
        }
        let Some((conn, eid)) = self.env_target(&id) else {
            return;
        };
        if let Some(e) = self.envs.get_mut(&id) {
            e.mcp.checking = true;
        }
        cx.notify();
        let path = format!("/api/projects/{eid}/mcp/check");
        self.background(
            window,
            cx,
            move || rest::send_json_secs(&conn, "POST", &path, Value::Null, 90),
            move |this, r, window, cx| {
                let mut note = None;
                if let Some(e) = this.envs.get_mut(&id) {
                    e.mcp.checking = false;
                    match &r {
                        Ok(v) => {
                            apply_mcp(e, v);
                            note = Some(mcp_tally(&e.mcp.servers, false).1);
                        }
                        Err(err) => note = Some(format!("{err:#}")),
                    }
                }
                if let Some(text) = note {
                    this.toast(
                        if r.is_ok() {
                            NotificationType::Info
                        } else {
                            NotificationType::Error
                        },
                        text,
                        window,
                        cx,
                    );
                }
                cx.notify();
            },
        );
    }

    /// Sign in to an MCP server (OAuth): its page opens in the browser; the agent connects it once
    /// the browser comes back and says so (`Event::EnvChanged`, `app/events.rs`).
    pub fn env_authorize(
        &mut self,
        id: String,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((conn, eid)) = self.env_target(&id) else {
            return;
        };
        let path = format!("/api/projects/{eid}/mcp/{}/authorize", enc(&name));
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "POST", &path, Value::Null),
            move |this, r, window, cx| match r {
                Ok(v) => {
                    match v["url"].as_str().filter(|u| !u.is_empty()) {
                        Some(url) => {
                            cx.open_url(url);
                            this.toast(
                                NotificationType::Info,
                                "Sign in in the browser: the server connects when you are back",
                                window,
                                cx,
                            );
                        }
                        None => this.toast(NotificationType::Success, "Connected", window, cx),
                    }
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }
}

impl BenchApp {
    /// A preset into the new MCP server's form: its name and its config, to look at and Save.
    pub fn env_fill_mcp(
        &mut self,
        id: &str,
        name: String,
        spec: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(e) = self.envs.get_mut(id) else {
            return;
        };
        let text = serde_json::to_string_pretty(spec).unwrap_or_default();
        e.new_name.update(cx, |s, cx| s.set_value(name, window, cx));
        e.mcp.json.update(cx, |s, cx| s.set_value(text, window, cx));
        cx.notify();
    }
}
