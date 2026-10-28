//! What is done to a project's MCP servers: one opened as a card, all checked (the agent asks
//! each to answer), one signed in to (OAuth in the browser), a preset filled into the new one's form.

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;
use serde_json::Value;

use crate::app::BenchApp;
use crate::client::rest;
use crate::env::enc;
use crate::env::mcp::apply_mcp;
use crate::env::mcp_status::mcp_tally;

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
