//! Adding and removing a server's agent, in the gear (`docs/remote-projects.md`). The servers'
//! agents this Mac knows are a list this Mac's agent keeps (`/api/agents`, `places.rs`); adding
//! one picks a host from ~/.ssh/config (or writes a new one there), and Bench installs and starts
//! the agent there on connecting. Removing it only forgets it here: the agent on the server runs
//! on, its Telegram bot and schedule with it.

use gpui_kit::component::input::InputState;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;
use serde_json::json;

use super::BenchApp;
use crate::rest;

/// The gear's "+ Agent on a server" form.
pub struct AgentForm {
    /// The host chosen from ~/.ssh/config; None: a new one, typed below.
    pub host: Option<String>,
    pub name: Entity<InputState>,
    pub address: Entity<InputState>,
    pub user: Entity<InputState>,
    pub port: Entity<InputState>,
    pub key: Entity<InputState>,
}

impl BenchApp {
    /// The hosts of ~/.ssh/config that have no agent here yet.
    pub fn hosts_without_agent(&self) -> Vec<String> {
        self.ssh
            .servers
            .iter()
            .map(|s| s.alias.clone())
            .filter(|a| !self.has_agent_on(a))
            .collect()
    }

    pub fn open_agent_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field = |hint: &str, window: &mut Window, cx: &mut Context<Self>| {
            let hint = hint.to_string();
            cx.new(|cx| InputState::new(window, cx).placeholder(hint))
        };
        self.agent_form = Some(AgentForm {
            host: self.hosts_without_agent().into_iter().next(),
            name: field(
                "How it shows in the sidebar (empty: the host's name)",
                window,
                cx,
            ),
            address: field("Address or domain", window, cx),
            user: field("Login", window, cx),
            port: field("22", window, cx),
            key: field("Private key file (empty = ssh-agent)", window, cx),
        });
        cx.notify();
    }

    /// The server joins Bench: its row here, then its agent installed and connected.
    pub fn add_agent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(conn), Some(form)) = (self.local_conn(), self.agent_form.as_ref()) else {
            return;
        };
        let v = |e: &Entity<InputState>| e.read(cx).value().trim().to_string();
        let typed_name = v(&form.name);
        let alias = match form.host.clone() {
            Some(alias) => alias,
            // A new server: into ~/.ssh/config first, named after it.
            None => {
                let address = v(&form.address);
                if address.is_empty() {
                    self.toast(
                        NotificationType::Error,
                        "The server's address is empty",
                        window,
                        cx,
                    );
                    return;
                }
                let base = if typed_name.is_empty() {
                    address.clone()
                } else {
                    typed_name.clone()
                };
                let alias: String = base
                    .to_lowercase()
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                    .collect::<String>()
                    .trim_matches('-')
                    .to_string();
                let server = crate::sshconfig::Server {
                    alias: alias.clone(),
                    hostname: address,
                    user: v(&form.user),
                    port: v(&form.port),
                    identity: v(&form.key),
                    ..Default::default()
                };
                if let Err(e) = crate::sshconfig::save(None, &server) {
                    self.toast(
                        NotificationType::Error,
                        format!("~/.ssh/config: {e:#}"),
                        window,
                        cx,
                    );
                    return;
                }
                self.reload_servers(cx);
                alias
            }
        };
        let name = if typed_name.is_empty() {
            let mut c = alias.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        } else {
            typed_name
        };
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || {
                rest::send_json(
                    &conn,
                    "POST",
                    "/api/agents",
                    &profile,
                    json!({"name": name, "host": alias}),
                )
            },
            |this, r, window, cx| match r {
                Ok(_) => {
                    this.agent_form = None;
                    // Listed: it is connected (its agent installed there first).
                    this.load_server_agents(window, cx);
                }
                Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
            },
        );
    }

    /// Bench forgets the server's agent (asked once): its row here goes, its threads and projects
    /// leave the sidebar. The agent there runs on.
    pub fn remove_agent(&mut self, host: String, window: &mut Window, cx: &mut Context<Self>) {
        let key = format!("agent:{host}");
        if self.confirm_delete.as_deref() != Some(key.as_str()) {
            self.confirm_delete = Some(key);
            cx.notify();
            return;
        }
        self.confirm_delete = None;
        let Some(conn) = self.local_conn() else {
            return;
        };
        if self.gear_agent == host {
            self.gear_agent.clear();
            self.pv = Default::default();
            self.load_providers(window, cx);
        }
        let profile = self.current_profile();
        let path = format!("/api/agents/{host}");
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "DELETE", &path, &profile, json!(null)),
            |this, r, window, cx| {
                if let Err(e) = r {
                    this.toast(NotificationType::Error, format!("{e:#}"), window, cx);
                }
                // Not listed any more: its link, threads and projects go (`load_server_agents`).
                this.load_server_agents(window, cx);
            },
        );
        cx.notify();
    }
}
