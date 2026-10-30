//! The new project's page (the sidebar's last row, "New project"), `docs/remote-projects.md`:
//! where its work happens — this Mac, a server from this Mac, a server's own agent, or no folder
//! — as four cards, then what that place needs, its name, and one sentence saying who will do
//! the work where. It takes the center, as Settings does; Escape or Cancel goes back.
//!
//! Creating it starts its first thread.

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::json;

use crate::app::BenchApp;
use crate::client::rest;
use bench_ui::icons::Ico;

/// The new project's form; lives while the dialog is open.
pub struct ProjectForm {
    pub name: Entity<InputState>,
    /// Where its work happens: the card picked.
    pub place: Place,
    /// The agent that keeps it: "" = this Mac's, else a server's (its ~/.ssh/config alias).
    pub agent: String,
    /// With this Mac's agent: the machine its work is on, None = this Mac.
    pub machine: Option<String>,
    /// On this Mac: a codebase on disk, if it has one (picked).
    pub folder: Option<String>,
    /// On a server: its folder there, typed (there is no picker for a server's disk).
    pub server_folder: Entity<InputState>,
    /// An exported environment it starts with (`env/backup.rs`): the file's name and what it
    /// holds, imported once it is made.
    pub environment: Option<(String, serde_json::Value)>,
    /// Enter in either field creates the project — the dialog's commit.
    _enter: Vec<Subscription>,
}

impl BenchApp {
    /// The sidebar's "New project": its page in the center.
    pub fn new_project_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Project name"));
        name.update(cx, |s, cx| s.focus(window, cx));
        let server_folder = cx.new(|cx| {
            InputState::new(window, cx).placeholder("/var/www/site — or empty: its home")
        });
        let mut enter = Vec::new();
        for field in [&name, &server_folder] {
            enter.push(
                cx.subscribe_in(field, window, |this, _, ev: &InputEvent, window, cx| {
                    if matches!(ev, InputEvent::PressEnter { .. }) {
                        this.save_project(window, cx);
                    }
                }),
            );
        }
        self.new_project.form = Some(ProjectForm {
            name,
            place: Place::ThisMac,
            agent: String::new(),
            machine: None,
            folder: None,
            server_folder,
            environment: None,
            _enter: enter,
        });
        self.center = crate::workspace::Center::NewProject;
        cx.notify();
    }

    /// Leave the page: Escape, its ✕, Cancel — back to the thread.
    pub fn cancel_project_form(&mut self, cx: &mut Context<Self>) {
        self.new_project.form = None;
        if self.center == crate::workspace::Center::NewProject {
            self.center = crate::workspace::Center::Chat;
        }
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
                if let Some(f) = this.new_project.form.as_mut() {
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

    /// The form's environment "Choose…": an exported one it starts with.
    pub fn pick_project_environment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Start with it".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let file = crate::env::backup::read(&path);
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(file) = file else {
                    this.not_environment(window, cx);
                    return;
                };
                if let Some(f) = this.new_project.form.as_mut() {
                    // An empty name takes the project's it was exported from.
                    if f.name.read(cx).value().trim().is_empty() {
                        let from = file["name"].as_str().unwrap_or_default().to_string();
                        f.name.update(cx, |s, cx| s.set_value(from, window, cx));
                    }
                    let shown = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    f.environment = Some((shown, file));
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Create the form's project on its agent and start its first thread.
    pub fn save_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = self.new_project.form.as_ref() else {
            return;
        };
        let name = form.name.read(cx).value().trim().to_string();
        if name.is_empty() {
            self.toast(
                NotificationType::Error,
                "The project needs a name",
                window,
                cx,
            );
            return;
        }
        let agent = form.agent.clone();
        let typed = form.server_folder.read(cx).value().trim().to_string();
        let typed = (!typed.is_empty()).then_some(typed);
        let body = match (agent.as_str(), &form.machine) {
            // No folder: an assistant.
            _ if form.place == Place::NoFolder => json!({"name": name}),
            // A server's agent: the folder is its own disk's.
            (a, _) if !a.is_empty() => json!({"name": name, "folder": typed}),
            // This Mac's agent working on a server.
            (_, Some(m)) => json!({"name": name, "folder": typed, "machine": m}),
            // This Mac's agent on this Mac.
            _ => json!({"name": name, "folder": form.folder}),
        };
        let environment = form.environment.as_ref().map(|(_, v)| v.clone());
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
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "POST", "/api/projects", body),
            move |this, r, window, cx| match r {
                Ok(v) => {
                    this.new_project.form = None;
                    this.center = crate::workspace::Center::Chat;
                    let Some(id) = v["project"]["id"].as_str().map(str::to_string) else {
                        return;
                    };
                    if let Some(file) = environment {
                        let key = crate::app::model::ProjectKey::new(&agent, &id).to_string();
                        this.env_import_file(key, file, window, cx);
                    }
                    if agent.is_empty() {
                        this.load_projects(window, cx);
                        this.new_thread_in(id, cx);
                    } else {
                        // A server's project: in the sidebar at once, after its Host.
                        let bench_id = crate::app::model::ProjectKey::new(&agent, &id).to_string();
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

    /// The page: where the work happens (four cards), then what that place needs, the name, a
    /// line that says who will do the work where, and Create. Enter creates, Escape goes back.
    pub(crate) fn render_new_project_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let Some(form) = self.new_project.form.as_ref() else {
            return div().into_any_element();
        };
        let (name, place, agent, machine, folder, server_folder) = (
            form.name.clone(),
            form.place,
            form.agent.clone(),
            form.machine.clone(),
            form.folder.clone(),
            form.server_folder.clone(),
        );
        let environment = form.environment.as_ref().map(|(file, _)| file.clone());
        // What the chosen file holds, under it; else what one would hold.
        let environment_holds = form.environment.as_ref().map_or_else(
            || {
                "Its soul, prompts, skills, servers, schedule and bot from a file another \
                 project exported (its Settings → Backup)."
                    .to_string()
            },
            |(_, v)| format!("{}.", crate::env::backup::contents(v)),
        );
        let servers: Vec<String> = self.places.ssh_servers.iter().map(|s| s.alias.clone()).collect();
        let agents: Vec<String> = self.places.server_agents.iter().map(|a| a.host.clone()).collect();
        let label = |text: &'static str| {
            div().text_sm().font_weight(FontWeight::SEMIBOLD).child(text).into_any_element()
        };
        let hint = |text: String| div().text_xs().text_color(muted).child(text).into_any_element();

        // The four places, as cards; one not possible here says what would make it so.
        let card = |k: usize, icon, title: &'static str, text: String, pick: Place, off: bool, cx: &mut Context<Self>| {
            bench_ui::choice_card(("project-place", k), icon, title, text, place == pick, off, cx)
                .when(!off, |d| {
                    d.on_click(cx.listener(move |this, _, _, cx| this.pick_project_place(pick, cx)))
                })
                .into_any_element()
        };
        let cards = h_flex()
            .gap_2()
            .items_stretch()
            .child(card(0, Ico::Laptop, "Host", "A folder here. This agent's Host runs it.".into(), Place::ThisMac, false, cx))
            .child(card(
                1,
                Ico::Link,
                "A server, from here",
                if servers.is_empty() {
                    "Needs a server in ~/.ssh/config.".into()
                } else {
                    "The local agent works there over SSH; nothing is installed on it.".into()
                },
                Place::OverSsh,
                servers.is_empty(),
                cx,
            ))
            .child(card(
                2,
                Ico::Server,
                "A server's own agent",
                if agents.is_empty() {
                    "Needs a server's agent: Settings → Agents.".into()
                } else {
                    "Its agent lives there and works while this one sleeps.".into()
                },
                Place::ServerAgent,
                agents.is_empty(),
                cx,
            ))
            .child(card(3, Ico::Layers, "No folder", "An assistant: research, writing, Telegram.".into(), Place::NoFolder, false, cx));

        // What the place needs: a folder here, a server and its folder, or nothing.
        let mut fields: Vec<AnyElement> = vec![];
        let pick_row = |id: &'static str, labels: Vec<String>, at: usize, server: bool, cx: &mut Context<Self>| {
            let names = labels.clone();
            crate::inserts::tools::view::segmented_tall_owned(id, labels, at, cx, move |this, k, _, cx| {
                if let Some(f) = this.new_project.form.as_mut() {
                    if server {
                        f.machine = Some(names[k].clone());
                    } else {
                        f.agent = names[k].clone();
                    }
                }
                cx.notify();
            })
        };
        match place {
            Place::ThisMac => {
                let has = folder.is_some();
                fields.push(label("Folder"));
                fields.push(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_sm()
                                .text_color(if has { theme.foreground } else { muted })
                                .child(folder.clone().unwrap_or_else(|| "None yet — its threads work in your home folder".into())),
                        )
                        .child(
                            bench_ui::form_action("project-folder", if has { "Change…" } else { "Choose…" }, false, cx)
                                .on_click(cx.listener(|this, _, window, cx| this.pick_project_codebase(window, cx))),
                        )
                        .when(has, |d| {
                            d.child(bench_ui::form_action("project-folder-clear", "Clear", false, cx).on_click(
                                cx.listener(|this, _, _, cx| {
                                    if let Some(f) = this.new_project.form.as_mut() {
                                        f.folder = None;
                                    }
                                    cx.notify();
                                }),
                            ))
                        })
                        .into_any_element(),
                );
            }
            Place::OverSsh | Place::ServerAgent => {
                let (id, list, chosen, server) = if place == Place::OverSsh {
                    ("project-machine", servers.clone(), machine.clone().unwrap_or_default(), true)
                } else {
                    ("project-agent", agents.clone(), agent.clone(), false)
                };
                let at = list.iter().position(|h| *h == chosen).unwrap_or(0);
                fields.push(label("Server"));
                fields.push(pick_row(id, list, at, server, cx));
                fields.push(div().h_2().into_any_element());
                fields.push(label("Folder on it, optional"));
                fields.push(bench_ui::field(&server_folder).into_any_element());
                fields.push(hint(format!("Empty: {chosen}'s home folder.")));
            }
            Place::NoFolder => {}
        }

        // Who does the work, and where — the place in one sentence.
        let typed = server_folder.read(cx).value().trim().to_string();
        let summary = match place {
            Place::ThisMac => match &folder {
                Some(f) => format!("The local agent runs its threads, in {f}."),
                None => "The local agent runs its threads.".into(),
            },
            Place::OverSsh => format!(
                "The local agent runs its threads; files and commands are on {}{}, over SSH.",
                machine.clone().unwrap_or_default(),
                if typed.is_empty() { String::new() } else { format!(" in {typed}") }
            ),
            Place::ServerAgent => format!(
                "{}'s agent runs its threads, on its own machine{}.",
                agent,
                if typed.is_empty() { String::new() } else { format!(" in {typed}") }
            ),
            Place::NoFolder => "The local agent runs its threads; files it makes go to its own space.".into(),
        };

        let column = v_flex()
            .w_full()
            .max_w(px(bench_ui::PAGE_W))
            .mx_auto()
            .pt_8()
            .pb_8()
            .gap_2()
            .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("New project"))
            .child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child("A folder the agent works in, with its own threads, soul, skills and tools."),
            )
            .child(div().h_4())
            .child(label("Where does the work happen?"))
            .child(cards)
            .child(div().h_4())
            .child(label("Name"))
            .child(bench_ui::field(&name).into_any_element())
            .when(!fields.is_empty(), |d| d.child(div().h_2()).children(fields))
            .child(div().h_4())
            .child(label("Environment, optional"))
            .child(self.environment_row(environment, cx))
            .child(hint(environment_holds))
            .child(div().h_4())
            .child(
                // A long folder wraps inside the note, the icon at its first line.
                h_flex()
                    .gap_2()
                    .items_start()
                    .px_3()
                    .py_2p5()
                    .rounded_lg()
                    .bg(theme.secondary)
                    .text_sm()
                    .text_color(muted)
                    .child(div().flex_none().pt_0p5().child(Ico::Info.icon().small().text_color(muted)))
                    .child(div().flex_1().min_w_0().child(summary)),
            )
            .child(
                h_flex()
                    .pt_4()
                    .justify_end()
                    .gap_2()
                    .child(
                        bench_ui::form_action("project-cancel", "Cancel", false, cx)
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_project_form(cx))),
                    )
                    .child(
                        bench_ui::form_action("project-save", "Create project", true, cx)
                            .on_click(cx.listener(|this, _, window, cx| this.save_project(window, cx))),
                    ),
            );
        v_flex()
            .id("new-project-page")
            .size_full()
            .overflow_y_scroll()
            .px(px(bench_ui::CHAT_LEFT))
            .child(column)
            .into_any_element()
    }

    /// A card picked: what the form keeps follows the place (a server or an agent chosen first).
    /// The environment it starts with: the file and what it holds, or none; Choose… and Clear.
    fn environment_row(&self, environment: Option<String>, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let has = environment.is_some();
        h_flex()
            .gap_2()
            .items_center()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .text_color(if has { theme.foreground } else { theme.muted_foreground })
                    .child(environment.unwrap_or_else(|| "None — an empty one".into())),
            )
            .child(
                bench_ui::form_action("project-env", if has { "Change…" } else { "Choose…" }, false, cx)
                    .on_click(cx.listener(|this, _, window, cx| this.pick_project_environment(window, cx))),
            )
            .when(has, |d| {
                d.child(bench_ui::form_action("project-env-clear", "Clear", false, cx).on_click(
                    cx.listener(|this, _, _, cx| {
                        if let Some(f) = this.new_project.form.as_mut() {
                            f.environment = None;
                        }
                        cx.notify();
                    }),
                ))
            })
            .into_any_element()
    }

    fn pick_project_place(&mut self, place: Place, cx: &mut Context<Self>) {
        let first_server = self.places.ssh_servers.iter().next().map(|s| s.alias.clone());
        let first_agent = self.places.server_agents.first().map(|a| a.host.clone());
        if let Some(f) = self.new_project.form.as_mut() {
            f.place = place;
            f.agent = String::new();
            f.machine = None;
            match place {
                Place::OverSsh => f.machine = first_server,
                Place::ServerAgent => f.agent = first_agent.unwrap_or_default(),
                Place::NoFolder => f.folder = None,
                Place::ThisMac => {}
            }
        }
        cx.notify();
    }
}

/// Where a new project's work happens (`docs/remote-projects.md`): the four answers.
#[derive(Clone, Copy, PartialEq)]
pub enum Place {
    /// A folder on this Mac, this Mac's agent.
    ThisMac,
    /// This Mac's agent, its tools on a server over SSH.
    OverSsh,
    /// A server's own agent, on its machine.
    ServerAgent,
    /// No folder: an assistant.
    NoFolder,
}

/// The new project's page and where the next new chat works: its form, the folder chosen, whether
/// that folder is a git repo, and the path typed for a server (which has no folder dialog here).
pub struct NewProject {
    pub form: Option<ProjectForm>,
    pub folder: Option<String>,
    pub in_repo: bool,
    pub path_entry: bool,
    pub path: Entity<gpui_kit::component::input::InputState>,
}

impl NewProject {
    pub fn new(window: &mut Window, cx: &mut Context<crate::app::BenchApp>) -> Self {
        Self {
            form: None,
            folder: None,
            in_repo: true,
            path_entry: false,
            path: cx.new(|cx| {
                gpui_kit::component::input::InputState::new(window, cx)
                    .placeholder("/home/you/project — Enter")
            }),
        }
    }
}
