//! The project's name: a row of its block; its card renames the project or removes it.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};

use super::section::{Card, Request, Section, pad, project_block, remove, revert, row, save};
use super::{ProjectEnv, on_change};
use crate::app::BenchApp;

pub struct NameEnv {
    /// The project's name, as its card edits it.
    pub field: Entity<InputState>,
    pub dirty: bool,
}

impl NameEnv {
    pub(super) fn new(window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let field = cx.new(|cx| InputState::new(window, cx));
        on_change(&field, cx, |e| e.name.dirty = true);
        Self {
            field,
            dirty: false,
        }
    }
}

pub struct Name;

impl Section for Name {
    fn key(&self) -> &'static str {
        "name"
    }
    fn block(&self, general: bool) -> &'static str {
        project_block(general)
    }
    /// The Host project is this Mac's: it has no name to change.
    fn shown(&self, app: &BenchApp) -> bool {
        app.project_env.as_ref().is_some_and(|e| e.id != "general")
    }
    fn rows(&self, app: &BenchApp, cx: &mut Context<BenchApp>) -> Vec<AnyElement> {
        let Some(e) = app.project_env.as_ref() else {
            return vec![];
        };
        let name = e.name.field.read(cx).value().to_string();
        let mut rows = vec![row(
            e,
            "name",
            String::new(),
            "Name".into(),
            name,
            false,
            true,
            cx,
        )];
        // A project on a server: which one, and whether its agent there answers.
        if let Some(server) = server_of(app, &e.id) {
            rows.push(row(
                e,
                "name",
                "server".into(),
                "Server".into(),
                server.summary(),
                server.error.is_some(),
                false,
                cx,
            ));
        }
        rows
    }
    fn card(&self, app: &BenchApp, name: &str, cx: &mut Context<BenchApp>) -> Option<Card> {
        let e = app.project_env.as_ref()?;
        let muted = cx.theme().muted_foreground;
        if name == "server" {
            let server = server_of(app, &e.id)?;
            let lines = [
                format!("ssh {} — {}", server.alias, server.address),
                server.state(),
                "Its agent runs there and keeps the project's threads, skills, MCP servers and \
                 schedule; /terminal and /files work on that machine. The connection is the \
                 alias's block in ~/.ssh/config."
                    .to_string(),
            ];
            return Some(Card::new(
                "Server",
                pad(v_flex().gap_1().children(lines.into_iter().enumerate().map(|(k, l)| {
                    div().when(k > 0, |d| d.text_color(muted)).child(l)
                }))),
                vec![],
            ));
        }
        let mut actions = vec![save("name", e.name.dirty, cx)];
        actions.extend(revert("name", e.name.dirty, cx));
        actions.push(remove(
            e,
            "project",
            String::new(),
            "Remove the project",
            cx,
        ));
        Some(Card::new(
            "Name",
            pad(v_flex().child(Input::new(&e.name.field)).child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child("Removing the project keeps its threads."),
            )),
            actions,
        ))
    }
    fn apply(&self, e: &mut ProjectEnv, v: &Value, window: &mut Window, cx: &mut App) {
        if !e.name.dirty {
            let name = v["project"]["name"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            e.name
                .field
                .update(cx, |s, cx| s.set_value(name, window, cx));
        }
    }
    /// The name is the project's own row, on this Mac.
    fn save(
        &self,
        app: &mut BenchApp,
        id: &str,
        _eid: &str,
        _window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<Request> {
        let e = app.project_env.as_ref()?;
        Some((
            "PUT",
            format!("/api/projects/{id}"),
            json!({"name": e.name.field.read(cx).value().trim()}),
        ))
    }
    fn saved(&self, e: &mut ProjectEnv) {
        e.name.dirty = false;
    }
    /// The sidebar shows the name too.
    fn reload(&self, app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) {
        app.load_projects(window, cx);
    }
    fn revert(&self, app: &mut BenchApp, _: &mut Window, _: &mut Context<BenchApp>) -> bool {
        if let Some(e) = app.project_env.as_mut() {
            e.name.dirty = false;
        }
        false
    }
}

/// The server a project lives on, as the environment shows it.
struct ServerInfo {
    alias: String,
    /// `user@hostname:port`, as `~/.ssh/config` gives it.
    address: String,
    connected: bool,
    error: Option<String>,
}

impl ServerInfo {
    fn state(&self) -> String {
        match (&self.error, self.connected) {
            (Some(e), _) => format!("Its agent: {e}"),
            (None, true) => "Its agent: connected".into(),
            (None, false) => "Its agent: connecting…".into(),
        }
    }

    fn summary(&self) -> String {
        format!("{} — {} · {}", self.alias, self.address, self.state().trim_start_matches("Its agent: "))
    }
}

fn server_of(app: &BenchApp, project: &str) -> Option<ServerInfo> {
    let alias = app.projects.iter().find(|p| p.id == project)?.host.clone()?;
    let address = app
        .ssh
        .servers
        .iter()
        .find(|s| s.alias == alias)
        .map(|s| {
            let user = if s.user.is_empty() { String::new() } else { format!("{}@", s.user) };
            let port = if s.port.is_empty() || s.port == "22" { String::new() } else { format!(":{}", s.port) };
            format!("{user}{}{port}", if s.hostname.is_empty() { &alias } else { &s.hostname })
        })
        .unwrap_or_else(|| "not in ~/.ssh/config".into());
    let remote = app.remotes.get(&alias);
    Some(ServerInfo {
        alias,
        address,
        connected: remote.is_some_and(|r| r.conn.is_some()),
        error: remote.and_then(|r| r.error.clone()),
    })
}
