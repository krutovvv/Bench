//! Where the project lives (`docs/remote-projects.md`), rows of its block: its agent (this Mac's,
//! or a server's — its address and whether it answers), the machine its work is on when that is
//! another (this Mac's agent over SSH), and its folder — which can be changed, and the agent kept
//! in it (`agent/src/confined-env.ts`). Agent and machine are chosen when the project is made.

use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};

use super::section::{Card, Request, Section, pad, project_block, revert, row, save};
use super::{ProjectEnv, on_change};
use crate::app::BenchApp;
use crate::model::ProjectKey;

pub struct PlaceEnv {
    /// The folder, as its card edits it ("" none: a folder the agent keeps for it).
    pub folder: Entity<InputState>,
    /// Its agent works in the folder and nowhere else.
    pub confined: bool,
    pub dirty: bool,
}

impl PlaceEnv {
    pub(super) fn new(window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let folder = cx
            .new(|cx| InputState::new(window, cx).placeholder("/path/to/the/folder — empty: none"));
        on_change(&folder, cx, |e| e.place.dirty = true);
        Self {
            folder,
            confined: false,
            dirty: false,
        }
    }
}

pub struct Place;

impl Section for Place {
    fn key(&self) -> &'static str {
        "place"
    }
    fn block(&self, general: bool) -> &'static str {
        project_block(general)
    }
    fn rows(&self, app: &BenchApp, cx: &mut Context<BenchApp>) -> Vec<AnyElement> {
        let Some(e) = app.project_env.as_ref() else {
            return vec![];
        };
        let key = ProjectKey::parse(&e.id);
        let at = app.location(&e.id);
        let mut rows = vec![];
        // The agent: this Mac's, or a server's with its address and state.
        let (agent, bad) = if at.agent.is_empty() {
            ("This Mac".to_string(), false)
        } else {
            let link = app.agents.get(&at.agent);
            let state = match link {
                Some(l) if l.conn.is_some() => "connected".to_string(),
                Some(l) => l.error.clone().unwrap_or_else(|| "connecting…".into()),
                None => "not connected".into(),
            };
            (
                format!("{} — {} · {state}", at.agent, address_of(app, &at.agent)),
                link.is_some_and(|l| l.error.is_some()),
            )
        };
        rows.push(row(
            e,
            "place",
            "agent".into(),
            "Agent".into(),
            agent,
            bad,
            false,
            cx,
        ));
        if let Some(m) = &at.machine {
            rows.push(row(
                e,
                "place",
                "machine".into(),
                "Works on".into(),
                format!("{m} — {} · over SSH", address_of(app, m)),
                false,
                false,
                cx,
            ));
        }
        // An agent's Host works in its home; any other project has a folder, or none.
        if !key.is_host() {
            let folder = match (&at.folder, e.place.confined) {
                (Some(f), true) => format!("{f} · the agent is kept in it"),
                (Some(f), false) => f.clone(),
                (None, _) => format!("none — a folder its agent keeps for it ({})", e.dir),
            };
            rows.push(row(
                e,
                "place",
                "folder".into(),
                "Folder".into(),
                folder,
                false,
                false,
                cx,
            ));
        }
        rows
    }
    fn card(&self, app: &BenchApp, name: &str, cx: &mut Context<BenchApp>) -> Option<Card> {
        let e = app.project_env.as_ref()?;
        let muted = cx.theme().muted_foreground;
        let at = app.location(&e.id);
        let lines = |lines: Vec<String>| {
            pad(v_flex().gap_1().children(
                lines
                    .into_iter()
                    .enumerate()
                    .map(move |(k, l)| div().when(k > 0, |d| d.text_color(muted)).child(l)),
            ))
        };
        match name {
            "agent" if at.agent.is_empty() => Some(Card::new(
                "Agent",
                lines(vec![
                    "This Mac's agent runs its threads.".into(),
                    "Its models and voice are This Mac's in the gear.".into(),
                ]),
                vec![],
            )),
            "agent" => Some(Card::new(
                "Agent",
                lines(vec![
                    format!("ssh {} — {}", at.agent, address_of(app, &at.agent)),
                    "Its agent runs there and keeps the project's threads, skills, MCP servers and \
                     schedule, without this Mac; /terminal and /files work on that machine. Its \
                     models are set in the gear; the connection is the alias's block in ~/.ssh/config."
                        .into(),
                ]),
                vec![],
            )),
            "machine" => {
                let m = at.machine.clone()?;
                Some(Card::new(
                    "Works on",
                    lines(vec![
                        format!("ssh {m} — {}", address_of(app, &m)),
                        "This Mac's agent works there over SSH, one connection kept open: read, write, \
                         edit and bash act on that machine, /terminal opens there. Nothing is installed \
                         on it; the models are this Mac's, and it answers while this Mac is on."
                            .into(),
                    ]),
                    vec![],
                ))
            }
            _ => {
                let view = cx.entity();
                let kept = Checkbox::new("env-folder-confined")
                    .checked(e.place.confined)
                    .label("Keep its agent in this folder")
                    .on_click(move |on, _, cx| {
                        let on = *on;
                        view.update(cx, |this, cx| {
                            if let Some(e) = this.project_env.as_mut() {
                                e.place.confined = on;
                                e.place.dirty = true;
                            }
                            cx.notify();
                        })
                    });
                let mut actions = vec![save("place", e.place.dirty, cx)];
                actions.extend(revert("place", e.place.dirty, cx));
                let local = at.machine_here().is_empty();
                Some(Card::new(
                    "Folder",
                    pad(v_flex()
                        .gap_2()
                        .child(
                            h_flex()
                                .gap_2()
                                .child(div().flex_1().child(Input::new(&e.place.folder)))
                                .when(local, |d| {
                                    d.child(crate::ui::form_action("env-folder-pick", "Choose…", false, cx).on_click(
                                        cx.listener(|this, _, window, cx| this.env_pick_folder(window, cx)),
                                    ))
                                }),
                        )
                        .child(kept)
                        .child(div().text_sm().text_color(muted).child(
                            "Its threads work in it. Kept in it, the agent's tools refuse anything outside it and \
                             its commands cannot write outside it (a sandbox: macOS's, or bubblewrap on Linux \
                             when installed). New threads take a change; a running one keeps what it began with.",
                        ))),
                    actions,
                ))
            }
        }
    }
    fn apply(&self, e: &mut ProjectEnv, v: &Value, window: &mut Window, cx: &mut App) {
        if !e.place.dirty {
            let folder = v["project"]["folder"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            e.place.confined = v["project"]["confined"].as_bool().unwrap_or(false);
            e.place
                .folder
                .update(cx, |s, cx| s.set_value(folder, window, cx));
        }
    }
    fn save(
        &self,
        app: &mut BenchApp,
        _id: &str,
        eid: &str,
        _window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<Request> {
        let e = app.project_env.as_ref()?;
        let folder = e.place.folder.read(cx).value().trim().to_string();
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
    fn reload(&self, app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) {
        app.load_projects(window, cx);
        let agent = ProjectKey::parse(
            app.project_env
                .as_ref()
                .map(|e| e.id.as_str())
                .unwrap_or(""),
        )
        .agent;
        if !agent.is_empty() {
            app.load_server_projects(agent, window, cx);
        }
    }
    fn revert(&self, app: &mut BenchApp, _: &mut Window, _: &mut Context<BenchApp>) -> bool {
        if let Some(e) = app.project_env.as_mut() {
            e.place.dirty = false;
        }
        false
    }
}

impl BenchApp {
    /// The folder card's "Choose…": a folder on this Mac.
    pub fn env_pick_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                if let Some(e) = this.project_env.as_mut() {
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
    app.ssh
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
