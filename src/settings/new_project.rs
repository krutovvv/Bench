//! The new project's popover (the sidebar's "New project"), `docs/remote-projects.md`: its
//! name, its agent — this Mac's or a server's — and where its work is. A short decision gets
//! a short surface: a popover under the sidebar's strip, where the button pressed lives, as
//! the status bar's panels do — not a takeover of the window. Every line of prose the page
//! carried became a placeholder or went away — the chips name themselves ("This Mac", "dev"),
//! the folder field says its shape in the placeholder.
//!
//! Creating it starts its first thread; its environment is set up in its environment thread.

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::json;

use crate::app::BenchApp;
use crate::client::rest;

/// The new project's form; lives while the dialog is open.
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
    /// Enter in either field creates the project — the dialog's commit.
    _enter: Vec<Subscription>,
}

impl BenchApp {
    /// The sidebar's "New project": the card over the thread (the thread stays as it was).
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
        self.project_form = Some(ProjectForm {
            name,
            agent: String::new(),
            machine: None,
            folder: None,
            server_folder,
            _enter: enter,
        });
        cx.notify();
    }

    /// Close the card: Escape, its ✕, Cancel, a click beside it.
    pub fn cancel_project_form(&mut self, cx: &mut Context<Self>) {
        self.project_form = None;
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
                    let Some(id) = v["project"]["id"].as_str().map(str::to_string) else {
                        return;
                    };
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

    /// The card: a name, who runs it, where its work is. Enter creates, Escape closes.
    pub(crate) fn render_project_dialog(&mut self, cx: &mut Context<Self>) -> AnyElement {
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
        // One size for everything in the card, the fields' own (`text_sm`, set on the card):
        // a label, a choice, a folder and an action read as one form.
        let label = |text: &'static str| div().text_color(muted).child(text).into_any_element();
        // Who runs it: this Mac's agent, or a server's (added in the gear). With one agent
        // there is nothing to choose, and no row to show.
        let hosts = self.agent_hosts();
        let agent_row = (hosts.len() > 1).then(|| {
            let labels: Vec<String> = hosts.iter().map(|h| Self::agent_name(h)).collect();
            let at = hosts.iter().position(|h| *h == agent).unwrap_or(0);
            v_flex()
                .gap_1p5()
                .child(label("Who runs its threads"))
                .child(crate::inserts::tools::view::segmented_tall_owned(
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
                ))
                .into_any_element()
        });
        // Where its files and commands are, when this Mac's agent runs it.
        let machine_row =
            (agent.is_empty() && self.ssh.servers.iter().next().is_some()).then(|| {
                let mut labels = vec!["This Mac".to_string()];
                labels.extend(self.ssh.servers.iter().map(|s| s.alias.clone()));
                let at = machine
                    .as_ref()
                    .and_then(|m| labels.iter().position(|l| l == m))
                    .unwrap_or(0);
                let names = labels.clone();
                v_flex()
                    .gap_1p5()
                    .child(label("Where its work is"))
                    .child(crate::inserts::tools::view::segmented_tall_owned(
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
                    ))
                    .into_any_element()
            });
        // The folder: picked from this Mac's disk, or typed for a server's.
        let on_server = !agent.is_empty() || machine.is_some();
        let folder_row = if on_server {
            let who = if !agent.is_empty() {
                agent.clone()
            } else {
                machine.clone().unwrap_or_default()
            };
            v_flex()
                .gap_1p5()
                .child(label("Folder on it, optional"))
                .child(bench_ui::field(&server_folder).into_any_element())
                .child(
                    div()
                        .text_color(muted)
                        .child(format!("Empty: {who}'s home directory.")),
                )
                .into_any_element()
        } else {
            let has = folder.is_some();
            v_flex()
                .gap_1p5()
                .child(label("Codebase folder, optional"))
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_color(if has { theme.foreground } else { muted })
                                .child(folder.clone().unwrap_or_else(|| "No folder".into())),
                        )
                        .child(
                            bench_ui::form_action(
                                "project-folder",
                                if has { "Change…" } else { "Choose…" },
                                false,
                                cx,
                            )
                            .on_click(cx.listener(
                                |this, _, window, cx| this.pick_project_codebase(window, cx),
                            )),
                        )
                        .when(has, |d| {
                            d.child(
                                bench_ui::form_action("project-folder-clear", "Clear", false, cx)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(f) = this.project_form.as_mut() {
                                            f.folder = None;
                                        }
                                        cx.notify();
                                    })),
                            )
                        })
                        .into_any_element(),
                )
                .into_any_element()
        };
        v_flex()
            .w(px(bench_ui::POPOVER_CARD_W))
            .text_sm()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(theme.border)
            .bg(theme.background)
            .shadow_lg()
            // Clicks inside the card are the card's; only the dim behind closes it.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                h_flex()
                    .items_center()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::MEDIUM)
                            .child("New project"),
                    )
                    .child(div().flex_1())
                    .child(
                        bench_ui::icon_button(
                            "project-close",
                            bench_ui::icons::Ico::X,
                            "Close  esc",
                            false,
                            cx,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_project_form(cx))),
                    ),
            )
            .child(
                v_flex()
                    .gap_1p5()
                    .child(label("Name"))
                    .child(bench_ui::field(&name).into_any_element()),
            )
            .children(agent_row)
            .children(machine_row)
            .child(folder_row)
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        bench_ui::form_action("project-cancel", "Cancel", false, cx)
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_project_form(cx))),
                    )
                    .child(
                        bench_ui::form_action("project-save", "Create project", true, cx).on_click(
                            cx.listener(|this, _, window, cx| this.save_project(window, cx)),
                        ),
                    ),
            )
            .into_any_element()
    }
}
