//! The new project's page (the sidebar's "+"): its name, where its agent runs (this Mac or a
//! server), and a folder if it has one (on this Mac, or on the server). On a server that has an
//! agent already the project is made there, beside the server's Host; on a new server Bench
//! installs the agent and the project is that server's Host (`docs/remote-projects.md`).
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
    /// A codebase on disk, if it has one.
    pub folder: Option<String>,
    /// A server whose agent the project is (an ~/.ssh/config host); None = this Mac, Some("") =
    /// another server, typed in below (it goes into ~/.ssh/config too).
    pub host: Option<String>,
    pub new_host: Entity<InputState>,
    pub new_user: Entity<InputState>,
    pub new_port: Entity<InputState>,
    pub new_key: Entity<InputState>,
    /// A folder on the chosen server, typed (there is no picker for a server's disk).
    pub server_folder: Entity<InputState>,
    /// With this Mac's agent: the machine its work is done on (an ~/.ssh/config host), None =
    /// this Mac. Nothing is installed there: the agent works on it over SSH.
    pub machine: Option<String>,
}

impl BenchApp {
    /// The sidebar's "+": the new project's page, in place of the thread (the thread stays as it was).
    pub fn new_project_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.panel = crate::app::Panel::NewProject;
        self.ssh.form = None;
        self.files_remote.form = false;
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Project name"));
        name.update(cx, |s, cx| s.focus(window, cx));
        let field = |hint: &str, window: &mut Window, cx: &mut Context<Self>| {
            let hint = hint.to_string();
            cx.new(|cx| InputState::new(window, cx).placeholder(hint))
        };
        let new_host = field("Address or domain", window, cx);
        let server_folder = field("/var/www/site — empty: none", window, cx);
        let new_user = field("Login", window, cx);
        let new_port = field("22", window, cx);
        let new_key = field("Private key file (empty = ssh-agent)", window, cx);
        self.project_form = Some(ProjectForm {
            name,
            folder: None,
            host: None,
            new_host,
            server_folder,
            machine: None,
            new_user,
            new_port,
            new_key,
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

    /// Create the form's project and start its first thread.
    pub fn save_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(conn), Some(form)) = (self.local_conn(), self.project_form.as_ref()) else {
            return;
        };
        let name = form.name.read(cx).value().trim().to_string();
        if name.is_empty() {
            self.toast(
                NotificationType::Error,
                "The project needs a name".to_string(),
                window,
                cx,
            );
            return;
        }
        // Another server: into ~/.ssh/config first, named after the project.
        let mut host = form.host.clone();
        if host.as_deref() == Some("") {
            let v = |e: &Entity<InputState>| e.read(cx).value().trim().to_string();
            let address = v(&form.new_host);
            if address.is_empty() {
                self.toast(
                    NotificationType::Error,
                    "The server's address is empty".to_string(),
                    window,
                    cx,
                );
                return;
            }
            let alias: String = name
                .to_lowercase()
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                .collect::<String>()
                .trim_matches('-')
                .to_string();
            let alias = if alias.is_empty() {
                address.clone()
            } else {
                alias
            };
            let server = crate::sshconfig::Server {
                alias: alias.clone(),
                hostname: address,
                user: v(&form.new_user),
                port: v(&form.new_port),
                identity: v(&form.new_key),
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
            host = Some(alias);
        }
        // A server with an agent already: the project is made there, its folder on that disk.
        if let Some(alias) = host.clone().filter(|a| self.has_agent_on(a)) {
            let folder = form.server_folder.read(cx).value().trim().to_string();
            let Some(conn) = self.conn_of(&alias) else {
                self.toast(
                    NotificationType::Error,
                    format!("{alias}: its agent is not connected"),
                    window,
                    cx,
                );
                return;
            };
            let body = json!({"name": name, "folder": (!folder.is_empty()).then_some(folder)});
            let profile = self.current_profile();
            self.background(
                window,
                cx,
                move || rest::send_json(&conn, "POST", "/api/projects", &profile, body),
                move |this, r, window, cx| match r {
                    Ok(v) => {
                        this.project_form = None;
                        this.panel = crate::app::Panel::Chat;
                        if let Some(rid) = v["project"]["id"].as_str() {
                            let id = format!("{alias}:{rid}");
                            let entry = crate::model::ProjectEntry {
                                id: id.clone(),
                                name: v["project"]["name"].as_str().unwrap_or_default().into(),
                                folder: v["project"]["folder"].as_str().map(str::to_string),
                                host: Some(alias.clone()),
                                remote_id: Some(rid.to_string()),
                                machine: None,
                                browser: false,
                            };
                            let mut list = this.server_projects.remove(&alias).unwrap_or_default();
                            list.push(entry);
                            this.server_projects.insert(alias.clone(), list.clone());
                            this.place_server_projects(&alias, list);
                            this.new_thread_in(id, cx);
                        }
                        this.load_server_projects(alias, window, cx);
                    }
                    Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
                },
            );
            return;
        }
        // This Mac's agent working on a server: its folder is a path there.
        let (machine, folder) = match (&host, &form.machine) {
            (None, Some(m)) => {
                let f = form.server_folder.read(cx).value().trim().to_string();
                (Some(m.clone()), (!f.is_empty()).then_some(f))
            }
            _ => (None, form.folder.clone()),
        };
        let body = json!({"name": name, "folder": folder, "host": host, "machine": machine});
        self.reload_servers(cx);
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "POST", "/api/projects", &profile, body),
            |this, r, window, cx| match r {
                Ok(v) => {
                    this.project_form = None;
                    this.panel = crate::app::Panel::Chat;
                    this.load_projects(window, cx);
                    // A local project starts with its first thread; a remote one with its
                    // server agent's threads (they show once it is connected).
                    if let (Some(id), None) =
                        (v["project"]["id"].as_str(), v["project"]["host"].as_str())
                    {
                        this.new_thread_in(id.to_string(), cx);
                    }
                }
                Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
            },
        );
    }

    /// The new project's page (the sidebar's "+"), laid out like Settings: what it is called,
    /// where its agent runs (this Mac or a server), and on this Mac a folder if it has one.
    pub(crate) fn render_project_form(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let Some(form) = self.project_form.as_ref() else {
            return div().into_any_element();
        };
        let (name, host, folder) = (form.name.clone(), form.host.clone(), form.folder.clone());
        let (new_host, new_user, new_port, new_key) = (
            form.new_host.clone(),
            form.new_user.clone(),
            form.new_port.clone(),
            form.new_key.clone(),
        );
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
        // Where it runs: this Mac, or a server (its agent there keeps the threads, skills, MCP).
        let on_server = host.is_some();
        let place = crate::inserts::tools::view::segmented_tall(
            "project-place",
            &["This Mac", "A server"],
            on_server as usize,
            cx,
            |this, k, _, cx| {
                if let Some(f) = this.project_form.as_mut() {
                    f.host = if k == 0 {
                        None
                    } else {
                        Some(
                            this.ssh
                                .servers
                                .first()
                                .map(|s| s.alias.clone())
                                .unwrap_or_default(),
                        )
                    };
                }
                cx.notify();
            },
        );
        page.push(
            self.pv_row(
                "Runs on",
                "Where its agent works and keeps its threads",
                h_flex().child(place).into_any_element(),
                cx,
            )
            .into_any_element(),
        );
        if let Some(chosen) = host.clone() {
            // The servers from ~/.ssh/config, and a new one typed in.
            let mut labels: Vec<String> =
                self.ssh.servers.iter().map(|s| s.alias.clone()).collect();
            let at = labels
                .iter()
                .position(|a| *a == chosen)
                .unwrap_or(labels.len());
            labels.push("New server…".into());
            let aliases = labels.clone();
            let pick = crate::inserts::tools::view::segmented_tall_owned(
                "project-server",
                labels,
                at,
                cx,
                move |this, k, _, cx| {
                    if let Some(f) = this.project_form.as_mut() {
                        f.host = Some(if k + 1 == aliases.len() {
                            String::new()
                        } else {
                            aliases[k].clone()
                        });
                    }
                    cx.notify();
                },
            );
            page.push(
                self.pv_row(
                    "Server",
                    "A host from ~/.ssh/config, or a new one",
                    h_flex().flex_wrap().child(pick).into_any_element(),
                    cx,
                )
                .into_any_element(),
            );
            if !chosen.is_empty() && self.has_agent_on(&chosen) {
                let server_folder = self.project_form.as_ref().map(|f| f.server_folder.clone());
                if let Some(input) = server_folder {
                    page.push(
                        self.pv_row(
                            "Folder",
                            format!("Optional: where on {chosen} its threads work"),
                            self.pv_input(&input),
                            cx,
                        )
                        .into_any_element(),
                    );
                }
            }
            if chosen.is_empty() {
                page.push(
                    self.pv_row("Address", "Its IP or domain", self.pv_input(&new_host), cx)
                        .into_any_element(),
                );
                page.push(
                    self.pv_row("Login", "The user on it", self.pv_input(&new_user), cx)
                        .into_any_element(),
                );
                page.push(
                    self.pv_row("Port", "Empty: 22", self.pv_input(&new_port), cx)
                        .into_any_element(),
                );
                page.push(
                    self.pv_row("Key file", "Empty: ssh-agent", self.pv_input(&new_key), cx)
                        .into_any_element(),
                );
            }
            page.push(
                div()
                    .pt_2()
                    .text_sm()
                    .text_color(muted)
                    .child(if !chosen.is_empty() && self.has_agent_on(&chosen) {
                        format!("Made on {chosen}'s agent, beside its Host: the project's threads, skills and MCP servers live there, and its models are {chosen}'s (the gear).")
                    } else {
                        "Bench installs and starts the agent there over SSH (it needs Node.js 22.18+); the project is that server's Host: its threads, skills and MCP servers live there.".to_string()
                    })
                    .into_any_element(),
            );
        } else {
            // This Mac's agent, its work on this Mac or on a server over SSH (like an agent in a
            // terminal: nothing is installed there, the models are this Mac's).
            let machine = self.project_form.as_ref().and_then(|f| f.machine.clone());
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
        }
        if let (None, Some(m)) = (
            host.clone(),
            self.project_form.as_ref().and_then(|f| f.machine.clone()),
        ) {
            let input = self.project_form.as_ref().map(|f| f.server_folder.clone());
            if let Some(input) = input {
                page.push(
                    self.pv_row(
                        "Folder",
                        format!("Optional: where on {m} its threads work"),
                        self.pv_input(&input),
                        cx,
                    )
                    .into_any_element(),
                );
            }
            page.push(
                div()
                    .pt_2()
                    .text_sm()
                    .text_color(muted)
                    .child(format!("This Mac's agent works on {m} over SSH: nothing is installed there, its models and settings are this Mac's, and it answers while this Mac is on."))
                    .into_any_element(),
            );
        } else if host.is_none() {
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
                    .on_click(
                        cx.listener(|this, _, window, cx| this.pick_project_codebase(window, cx)),
                    ),
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
