//! Where the project lives (`docs/remote-projects.md`), fields of General: its agent (this Mac's,
//! or a server's — its address and whether it answers), the machine its work is on when that is
//! another (this Mac's agent over SSH), and its folder — which can be changed, and the agent kept
//! in it (`agent/src/machine/confined.ts`). Agent and machine are chosen when the project is made.
//! An agent's Host has a folder too: its agent's `terminal.cwd`, its home when none. A changed
//! folder takes the threads that worked in the old one with it, from their next turn.

use gpui_kit::component::h_flex;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};

use super::section::{Request, Section, SettingsForm, form_actions, text};
use super::{ProjectEnv, on_change};
use crate::app::BenchApp;
use crate::app::model::ProjectKey;
use bench_ui::settings_field;

pub struct PlaceEnv {
    /// The folder, as its field edits it ("" none: a folder the agent keeps for it).
    pub folder: Entity<InputState>,
    /// Its agent works in the folder and nowhere else.
    pub confined: bool,
    /// Where its threads work now, as its agent says: the folder, else the one it keeps for it.
    pub cwd: String,
    pub dirty: bool,
}

impl PlaceEnv {
    pub(super) fn new(id: &str, window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let folder = cx
            .new(|cx| InputState::new(window, cx).placeholder("/path/to/the/folder — empty: none"));
        on_change(&folder, id, cx, |e| e.place.dirty = true);
        Self {
            folder,
            confined: false,
            cwd: String::new(),
            dirty: false,
        }
    }
}

pub struct Place;

impl Section for Place {
    fn key(&self) -> &'static str {
        "place"
    }

    fn apply(&self, e: &mut ProjectEnv, v: &Value, window: &mut Window, cx: &mut App) {
        e.place.cwd = v["cwd"].as_str().unwrap_or_default().to_string();
        if !e.place.dirty {
            // The Host's folder is where its threads work (its home unless set).
            let folder = if ProjectKey::parse(&e.id).is_host() {
                e.place.cwd.clone()
            } else {
                v["project"]["folder"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string()
            };
            e.place.confined = v["project"]["confined"].as_bool().unwrap_or(false);
            e.place
                .folder
                .update(cx, |s, cx| s.set_value(folder, window, cx));
        }
    }

    fn save(
        &self,
        app: &mut BenchApp,
        id: &str,
        eid: &str,
        _window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<Request> {
        let e = app.envs.get(id)?;
        let folder = e.place.folder.read(cx).value().trim().to_string();
        if ProjectKey::parse(id).is_host() {
            return Some((
                "PUT",
                format!("/api/projects/{eid}"),
                json!({"folder": folder}),
            ));
        }
        Some((
            "PUT",
            format!("/api/projects/{eid}"),
            json!({"folder": (!folder.is_empty()).then_some(folder), "confined": e.place.confined}),
        ))
    }

    fn saved(&self, e: &mut ProjectEnv) {
        e.place.dirty = false;
    }

    /// The sidebar knows the folder too (Zed, the icon).
    fn reload(
        &self,
        app: &mut BenchApp,
        id: &str,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) {
        app.load_projects(window, cx);
        let agent = ProjectKey::parse(id).agent;
        if !agent.is_empty() {
            app.load_server_projects(agent, window, cx);
        }
    }

    fn revert(
        &self,
        app: &mut BenchApp,
        id: &str,
        _: &mut Window,
        _: &mut Context<BenchApp>,
    ) -> bool {
        if let Some(e) = app.envs.get_mut(id) {
            e.place.dirty = false;
        }
        false
    }
}

impl SettingsForm for Place {
    fn block(&self, _general: bool) -> &'static str {
        "Project"
    }

    /// Its agent and the machine it works on (chosen when it was made, read here), its folder and
    /// whether the agent is kept in it.
    fn form(
        &self,
        app: &BenchApp,
        e: &ProjectEnv,
        cx: &mut Context<BenchApp>,
    ) -> Option<Vec<AnyElement>> {
        let key = ProjectKey::parse(&e.id);
        let at = app.location(&e.id);
        let mut out = vec![];
        let (agent, hint, bad) = if at.agent.is_empty() {
            (
                "Local".to_string(),
                "Its models and voice are the local agent's, in the gear",
                false,
            )
        } else {
            let link = app.agents.get(&at.agent);
            let state = match link {
                Some(l) if l.conn.is_some() => "connected".to_string(),
                Some(l) => l.error.clone().unwrap_or_else(|| "connecting…".into()),
                None => "not connected".into(),
            };
            (
                format!("{} — {} · {state}", at.agent, address_of(app, &at.agent)),
                "It runs there and keeps the project's threads, skills, servers and schedule",
                link.is_some_and(|l| l.error.is_some()),
            )
        };
        out.push(settings_field("Agent", hint, text(agent, bad, cx), cx).into_any_element());
        if let Some(m) = &at.machine {
            out.push(
                settings_field(
                    "Works on",
                    "The local agent works there over SSH; nothing is installed on it",
                    text(format!("{m} — {}", address_of(app, m)), false, cx),
                    cx,
                )
                .into_any_element(),
            );
        }
        // Where its threads work, on which machine; an agent's Host works in its home unless set.
        let local = at.machine_here().is_empty();
        let on = if local {
            String::new()
        } else {
            format!("On {} · ", at.machine_here())
        };
        let hint = if key.is_host() {
            format!("{on}Its threads work in it; empty: the home folder")
        } else if at.folder.is_some() {
            format!("{on}Its threads work in it, and follow a change from their next turn")
        } else if e.place.cwd.is_empty() {
            format!("{on}None: a folder its agent keeps for it")
        } else {
            format!("{on}None: its threads work in {}", e.place.cwd)
        };
        let pid = e.id.clone();
        out.push(
            settings_field(
                "Folder",
                hint,
                h_flex()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&e.place.folder)))
                    .when(local, |d| {
                        d.child(
                            bench_ui::form_action("env-folder-pick", "Choose…", false, cx)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.env_pick_folder(pid.clone(), window, cx)
                                })),
                        )
                    })
                    .into_any_element(),
                cx,
            )
            .into_any_element(),
        );
        if key.is_host() {
            out.extend(form_actions(&e.id, "place", e.place.dirty, cx));
            return Some(out);
        }
        let view = cx.entity();
        let pid = e.id.clone();
        // On or off, as a Mac's settings have it: a switch.
        let kept = gpui_kit::component::switch::Switch::new("env-folder-confined")
            .checked(e.place.confined)
            .on_click(move |on, _, cx| {
                let on = *on;
                view.update(cx, |this, cx| {
                    if let Some(e) = this.envs.get_mut(&pid) {
                        e.place.confined = on;
                        e.place.dirty = true;
                    }
                    cx.notify();
                })
            });
        out.push(
            settings_field(
                "Keep the agent in it",
                "Its tools refuse anything outside the folder, its commands cannot write outside \
                 it. New threads take a change",
                div().flex().justify_end().child(kept).into_any_element(),
                cx,
            )
            .into_any_element(),
        );
        out.extend(form_actions(&e.id, "place", e.place.dirty, cx));
        Some(out)
    }
}

impl BenchApp {
    /// The folder's "Choose…" in the project `id`'s Settings: a folder on this Mac.
    pub fn env_pick_folder(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
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
                if let Some(e) = this.envs.get_mut(&id) {
                    let folder = path.to_string_lossy().to_string();
                    e.place
                        .folder
                        .update(cx, |s, cx| s.set_value(folder, window, cx));
                    e.place.dirty = true;
                }
                cx.notify();
            });
        })
        .detach();
    }
}

/// `user@hostname:port` of an ~/.ssh/config alias.
fn address_of(app: &BenchApp, alias: &str) -> String {
    app.inserts.ssh
        .servers
        .iter()
        .find(|s| s.alias == alias)
        .map(|s| {
            let user = if s.user.is_empty() {
                String::new()
            } else {
                format!("{}@", s.user)
            };
            let port = if s.port.is_empty() || s.port == "22" {
                String::new()
            } else {
                format!(":{}", s.port)
            };
            format!(
                "{user}{}{port}",
                if s.hostname.is_empty() {
                    alias
                } else {
                    &s.hostname
                }
            )
        })
        .unwrap_or_else(|| "not in ~/.ssh/config".into())
}
