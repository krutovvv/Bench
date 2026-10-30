//! Adding and removing a server's agent, in the gear (`docs/remote-projects.md`). The servers'
//! agents this Mac knows are a list this Mac's agent keeps (`/api/agents`, `places.rs`); adding
//! one picks a host from ~/.ssh/config (or writes a new one there), and Bench installs and starts
//! the agent there on connecting. Removing it only forgets it here: the agent on the server runs
//! on, its Telegram bot and schedule with it.

use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::InputState;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::{ActiveTheme as _, h_flex};
use gpui_kit::*;
use serde_json::json;

use crate::app::BenchApp;
use crate::client::rest;

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
        self.places
            .ssh_servers
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
        self.gear.agent_form = Some(AgentForm {
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
        let (Some(conn), Some(form)) = (self.local_conn(), self.gear.agent_form.as_ref()) else {
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
                let server = crate::system::sshconfig::Server {
                    alias: alias.clone(),
                    hostname: address,
                    user: v(&form.user),
                    port: v(&form.port),
                    identity: v(&form.key),
                    ..Default::default()
                };
                if let Err(e) = crate::system::sshconfig::save(None, &server) {
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
        self.background(
            window,
            cx,
            move || {
                rest::send_json(
                    &conn,
                    "POST",
                    "/api/agents",
                    json!({"name": name, "host": alias}),
                )
            },
            |this, r, window, cx| match r {
                Ok(_) => {
                    this.gear.agent_form = None;
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
        if !self.armed.confirm(key) {
            cx.notify();
            return;
        }
        let Some(conn) = self.local_conn() else {
            return;
        };
        if self.gear.agent == host {
            self.gear.agent.clear();
            self.gear.providers = Default::default();
            self.load_providers(window, cx);
        }
        let path = format!("/api/agents/{host}");
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "DELETE", &path, json!(null)),
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

impl BenchApp {
    /// "+ Agent on a server", or its form: a host of ~/.ssh/config (or a new one), its name.
    pub(super) fn render_agent_form(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let muted = cx.theme().muted_foreground;
        let Some(form) = self.gear.agent_form.as_ref() else {
            return vec![
                h_flex()
                    .py_2()
                    .child(
                        bench_ui::secondary_button("gear-agent-add", "+ Agent on a server")
                            .small()
                            .on_click(cx.listener(|this, _, w, cx| this.open_agent_form(w, cx)))
                            .into_any_element(),
                    )
                    .into_any_element(),
            ];
        };
        let (host, name, address, user, port, key) = (
            form.host.clone(),
            form.name.clone(),
            form.address.clone(),
            form.user.clone(),
            form.port.clone(),
            form.key.clone(),
        );
        let mut labels = self.hosts_without_agent();
        let at = host
            .as_ref()
            .and_then(|h| labels.iter().position(|l| l == h))
            .unwrap_or(labels.len());
        labels.push("New server…".into());
        let names = labels.clone();
        let pick = bench_ui::segmented(
            "agent-server",
            labels,
            at,
            cx,
            bench_ui::on_pick(cx, move |this: &mut Self, k, _, cx| {
                if let Some(f) = this.gear.agent_form.as_mut() {
                    f.host = (k + 1 < names.len()).then(|| names[k].clone());
                }
                cx.notify();
            }),
        )
        .flex_wrap();
        let mut out = vec![
            bench_ui::settings_field(
                "Server",
                "A host from ~/.ssh/config, or a new one",
                h_flex().flex_wrap().child(pick).into_any_element(),
                cx,
            )
            .into_any_element(),
        ];
        if host.is_none() {
            for (title, hint, input) in [
                ("Address", "Its IP or domain", &address),
                ("Login", "The user on it", &user),
                ("Port", "Empty: 22", &port),
                ("Key file", "Empty: ssh-agent", &key),
            ] {
                out.push(
                    bench_ui::settings_field(
                        title,
                        hint,
                        gpui_kit::component::input::Input::new(input).into_any_element(),
                        cx,
                    )
                    .into_any_element(),
                );
            }
        }
        out.push(
            bench_ui::settings_field(
                "Name",
                "Its project in the sidebar",
                gpui_kit::component::input::Input::new(&name).into_any_element(),
                cx,
            )
            .into_any_element(),
        );
        out.push(
            div()
                .pt_2()
                .text_sm()
                .text_color(muted)
                .child("Bench installs its agent there over SSH (it needs Node.js 22.18+) and starts it. It keeps its own threads, models, skills and Telegram there and works on its own; its models are set here once it answers.")
                .into_any_element(),
        );
        out.push(
            h_flex()
                .pt_3()
                .pb_2()
                .gap_2()
                .child(
                    bench_ui::primary_button("gear-agent-save", "Add agent")
                        .small()
                        .on_click(cx.listener(|this, _, w, cx| this.add_agent(w, cx)))
                        .into_any_element(),
                )
                .child(
                    bench_ui::secondary_button("gear-agent-cancel", "Cancel")
                        .small()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.gear.agent_form = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                )
                .into_any_element(),
        );
        out
    }
}
