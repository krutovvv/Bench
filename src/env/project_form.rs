//! The new project's page (the sidebar's "+"), `docs/remote-projects.md`: its name, its agent —
//! this Mac's or a server's (servers' agents are added in the gear) — and where its work is:
//! - this Mac's agent: on this Mac (a folder, if any), or on a server over SSH (a folder there;
//!   nothing installed on it);
//! - a server's agent: on that server (a folder there, if any).
//!
//! Creating it starts its first thread; its environment is set up in its environment thread.

use gpui_kit::component::input::InputState;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::json;

use crate::app::BenchApp;
use crate::rest;

/// The new project's form (the sidebar's "+").
pub struct ProjectForm {
    pub name: Entity<InputState>,
    /// The agent that keeps it: "" = this Mac's, else a server's (its ~/.ssh/config alias).
    pub agent: String,
    /// With this Mac's agent: the machine its work is on, None = this Mac.
    pub machine: Option<String>,
    /// On this Mac: a codebase on disk, if it has one (picked).
    pub folder: Option<String>,
    /// On a server: its folder there, typed (there is no picker for a server's disk).
    pub server_folder: Entity<InputState>,
}

impl BenchApp {
    /// The sidebar's "+": the new project's page, in place of the thread (the thread stays as it was).
    pub fn new_project_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.panel = crate::app::Panel::NewProject;
        self.ssh.form = None;
        self.files_remote.form = false;
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Project name"));
        name.update(cx, |s, cx| s.focus(window, cx));
        let server_folder =
            cx.new(|cx| InputState::new(window, cx).placeholder("/var/www/site — empty: its home"));
        self.project_form = Some(ProjectForm {
            name,
            agent: String::new(),
            machine: None,
            folder: None,
            server_folder,
        });
        cx.notify();
    }

    /// The form's "Choose…": a folder on disk the project works in.
    pub fn pick_project_codebase(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("The project's folder".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update_in(cx, |this, window, cx| {
                let folder = path.to_string_lossy().to_string();
                if let Some(f) = this.project_form.as_mut() {
                    // An empty name takes the folder's.
                    if f.name.read(cx).value().trim().is_empty() {
                        let base = folder.rsplit('/').next().unwrap_or(&folder).to_string();
                        f.name.update(cx, |s, cx| s.set_value(base, window, cx));
                    }
                    f.folder = Some(folder);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Create the form's project on its agent and start its first thread.
    pub fn save_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = self.project_form.as_ref() else {
            return;
        };
        let name = form.name.read(cx).value().trim().to_string();
        if name.is_empty() {
            self.toast(NotificationType::Error, "The project needs a name", window, cx);
            return;
        }
        let agent = form.agent.clone();
        let typed = form.server_folder.read(cx).value().trim().to_string();
        let typed = (!typed.is_empty()).then_some(typed);
        let body = match (agent.as_str(), &form.machine) {
            // A server's agent: the folder is its own disk's.
            (a, _) if !a.is_empty() => json!({"name": name, "folder": typed}),
            // This Mac's agent working on a server.
            (_, Some(m)) => json!({"name": name, "folder": typed, "machine": m}),
            // This Mac's agent on this Mac.
            _ => json!({"name": name, "folder": form.folder}),
        };
        let Some(conn) = self.conn_of(&agent) else {
            let who = Self::agent_name(&agent);
            self.toast(
                NotificationType::Error,
                format!("{who}: its agent is not connected"),
                window,
                cx,
            );
            return;
        };
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "POST", "/api/projects", &profile, body),
            move |this, r, window, cx| match r {
                Ok(v) => {
                    this.project_form = None;
                    this.panel = crate::app::Panel::Chat;
                    let Some(id) = v["project"]["id"].as_str().map(str::to_string) else {
                        return;
                    };
                    if agent.is_empty() {
                        this.load_projects(window, cx);
                        this.new_thread_in(id, cx);
                    } else {
                        // A server's project: in the sidebar at once, after its Host.
                        let bench_id = crate::model::ProjectKey::new(&agent, &id).to_string();
                        if let Some(entry) = crate::app::places::entry_of(&agent, &v["project"]) {
                            this.add_server_project(entry);
                        }
                        this.new_thread_in(bench_id, cx);
                        this.load_server_projects(agent, window, cx);
                    }
                }
                Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
            },
        );
    }

    /// The new project's page, laid out like Settings: name, agent, where its work is.
    pub(crate) fn render_project_form(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let Some(form) = self.project_form.as_ref() else {
            return div().into_any_element();
        };
        let (name, agent, machine, folder, server_folder) = (
            form.name.clone(),
            form.agent.clone(),
            form.machine.clone(),
            form.folder.clone(),
            form.server_folder.clone(),
        );
        let note = |text: String| {
            div()
                .pt_2()
                .text_sm()
                .text_color(muted)
                .child(text)
                .into_any_element()
        };
        let mut page: Vec<AnyElement> = vec![
            div()
                .pt_6()
                .text_base()
                .text_color(muted)
                .child("A project is what you work on: a client, a site, a codebase. Its threads share its environment (soul, MCP servers, skills, prompts, Telegram, schedule); you set that up in its environment thread once it exists.")
                .into_any_element(),
            self.pv_heading("PROJECT", cx).into_any_element(),
            self.pv_row("Name", "How it shows in the sidebar", self.pv_input(&name), cx).into_any_element(),
        ];

        // The agent: this Mac's, or one of the servers' the gear lists.
        let hosts = self.agent_hosts();
        let labels: Vec<String> = hosts.iter().map(|h| Self::agent_name(h)).collect();
        let at = hosts.iter().position(|h| *h == agent).unwrap_or(0);
        let pick = crate::inserts::tools::view::segmented_tall_owned(
            "project-agent",
            labels,
            at,
            cx,
            move |this, k, _, cx| {
                if let Some(f) = this.project_form.as_mut() {
                    f.agent = hosts[k].clone();
                    f.machine = None;
                }
                cx.notify();
            },
        );
        page.push(
            self.pv_row(
                "Agent",
                "Who runs its threads (servers' agents are added in the gear)",
                h_flex().flex_wrap().child(pick).into_any_element(),
                cx,
            )
            .into_any_element(),
        );

        if !agent.is_empty() {
            // A server's agent: the server is the machine; only its folder there.
            page.push(
                self.pv_row(
                    "Folder",
                    format!("Optional: where on {agent} its threads work"),
                    self.pv_input(&server_folder),
                    cx,
                )
                .into_any_element(),
            );
            page.push(note(format!(
                "Made on {agent}'s agent: its threads, skills and MCP servers live there, its models are {agent}'s (the gear), and it works without this Mac — its Telegram bot too."
            )));
        } else {
            // This Mac's agent: its work on this Mac, or on a server over SSH.
            let mut labels = vec!["This Mac".to_string()];
            labels.extend(self.ssh.servers.iter().map(|s| s.alias.clone()));
            let at = machine
                .as_ref()
                .and_then(|m| labels.iter().position(|l| l == m))
                .unwrap_or(0);
            let names = labels.clone();
            let pick = crate::inserts::tools::view::segmented_tall_owned(
                "project-machine",
                labels,
                at,
                cx,
                move |this, k, _, cx| {
                    if let Some(f) = this.project_form.as_mut() {
                        f.machine = (k > 0).then(|| names[k].clone());
                    }
                    cx.notify();
                },
            );
            page.push(
                self.pv_row(
                    "Works on",
                    "Where its files and commands are",
                    h_flex().flex_wrap().child(pick).into_any_element(),
                    cx,
                )
                .into_any_element(),
            );
            match machine {
                Some(m) => {
                    page.push(
                        self.pv_row(
                            "Folder",
                            format!("Optional: where on {m} its threads work"),
                            self.pv_input(&server_folder),
                            cx,
                        )
                        .into_any_element(),
                    );
                    page.push(note(format!(
                        "This Mac's agent works on {m} over SSH: nothing is installed there, its models and settings are this Mac's, and it answers while this Mac is on."
                    )));
                }
                None => {
                    let has = folder.is_some();
                    let control = h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_base()
                                .text_color(if has { theme.foreground } else { muted })
                                .child(folder.clone().unwrap_or_else(|| "None".into())),
                        )
                        .child(
                            crate::ui::form_action(
                                "project-folder",
                                if has { "Change…" } else { "Choose…" },
                                false,
                                cx,
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.pick_project_codebase(window, cx)
                            })),
                        )
                        .when(has, |d| {
                            d.child(
                                crate::ui::form_action("project-folder-clear", "Clear", false, cx)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(f) = this.project_form.as_mut() {
                                            f.folder = None;
                                        }
                                        cx.notify();
                                    })),
                            )
                        })
                        .into_any_element();
                    page.push(
                        self.pv_row(
                            "Folder",
                            "Optional: a codebase its threads work in (opens in Zed)",
                            control,
                            cx,
                        )
                        .into_any_element(),
                    );
                }
            }
        }
        page.push(
            h_flex()
                .pt_4()
                .gap_2()
                .child(
                    crate::ui::form_action("project-save", "Create project", true, cx)
                        .on_click(cx.listener(|this, _, window, cx| this.save_project(window, cx))),
                )
                .child(
                    crate::ui::form_action("project-cancel", "Cancel", false, cx).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.project_form = None;
                            this.panel = crate::app::Panel::Chat;
                            cx.notify();
                        }),
                    ),
                )
                .into_any_element(),
        );
        v_flex()
            .id("new-project")
            .size_full()
            .overflow_y_scroll()
            .px_4()
            .pb_6()
            .child(v_flex().max_w(px(820.)).children(page))
            .into_any_element()
    }
}
