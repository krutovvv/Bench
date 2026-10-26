//! Git projects: which repo (and branch) a session works in, and the few git actions on it —
//! a chat in its own branch (a worktree folder), merge back, delete.
//!
//! The project comes from the session's folder when the agent recorded one, else from the files
//! the agent edited (their repo), remembered per profile. Chats without files have none.

use std::collections::HashMap;

use gpui_kit::*;

use super::BenchApp;
use crate::connection;
use crate::model::Project;
use crate::rest;

impl BenchApp {
    fn projects_path(&self) -> std::path::PathBuf {
        let key: String = format!("{}-{}", self.target.label(), self.current_profile())
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        connection::state_dir().join(format!("projects-{key}.json"))
    }

    /// Load what the agent knows about each session's folder, plus our own inferences.
    pub(crate) fn load_places(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.inferred = std::fs::read(self.projects_path())
            .ok()
            .and_then(|b| serde_json::from_slice::<HashMap<String, String>>(&b).ok())
            .unwrap_or_default();
        if self.default_cwd.is_none() {
            self.rest(window, cx, rest::default_cwd, |this, r, _, cx| {
                if let Ok(d) = r {
                    this.default_cwd = Some(d);
                    cx.notify();
                }
            });
        }
        self.rest(window, cx, rest::session_places, |this, r, window, cx| {
            if let Ok(places) = r {
                this.places = places;
                this.resolve_project(window, cx);
                cx.notify();
            }
        });
    }

    /// The project of a session, as far as we know without asking git.
    pub fn project_of(&self, session: &str) -> Option<Project> {
        let place = self.places.get(session);
        if let Some(p) = place.filter(|p| !p.repo.is_empty()) {
            return Some(Project::from_top(&p.repo, &p.branch));
        }
        // A branch folder is recognizable before its repo is recorded (Hermes named the branches
        // `hermes/<folder>`).
        if let Some(p) = place.filter(|p| p.cwd.contains("/.worktrees/")) {
            let branch = if p.branch.is_empty() {
                format!("hermes/{}", p.cwd.rsplit('/').next().unwrap_or_default())
            } else {
                p.branch.clone()
            };
            return Some(Project::from_top(&p.cwd, &branch));
        }
        self.inferred
            .get(session)
            .map(|top| Project::from_top(top, place.map(|p| p.branch.as_str()).unwrap_or("")))
    }

    /// The folder a session was started in, when it is not the agent's default (the user chose it).
    pub fn folder_of(&self, session: &str) -> Option<String> {
        // Until the agent's default folder is known, nothing counts as chosen (and nothing is probed).
        let default = self.default_cwd.as_ref()?;
        let cwd = &self.places.get(session)?.cwd;
        (!cwd.is_empty() && cwd != default).then(|| cwd.clone())
    }

    /// Work out the open session's project (and its live branch); infer it from edited files
    /// when the agent has none.
    pub(crate) fn resolve_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone() else {
            self.project = None;
            return;
        };
        let known = self.project_of(&session);
        let folder = self.folder_of(&session);
        let probe = match (&known, folder) {
            (Some(p), _) => p.dir.clone(),
            (None, Some(f)) => f,
            (None, None) => {
                // The first edited file's folder: is it inside a repo?
                let Some(dir) = self
                    .session_files()
                    .first()
                    .and_then(|(p, _)| p.rsplit_once('/').map(|(d, _)| d.to_string()))
                else {
                    self.project = None;
                    return;
                };
                dir
            }
        };
        if known.is_some() {
            self.project = known;
        }
        let target = self.target.clone();
        self.background(
            window,
            cx,
            move || {
                let top = connection::git(&target, &probe, &["rev-parse", "--show-toplevel"])?;
                let branch = connection::git(&target, &top, &["rev-parse", "--abbrev-ref", "HEAD"])
                    .unwrap_or_default();
                Ok((top, branch))
            },
            move |this, r: anyhow::Result<(String, String)>, _window, cx| {
                if this.session.as_deref() != Some(&session) {
                    return;
                }
                match r {
                    Ok((top, branch)) => {
                        if this.project_of(&session).is_none() {
                            this.inferred.insert(session.clone(), top.clone());
                            let _ = std::fs::create_dir_all(connection::state_dir());
                            let _ = std::fs::write(
                                this.projects_path(),
                                serde_json::to_vec(&this.inferred).unwrap_or_default(),
                            );
                        }
                        this.project = Some(Project::from_top(&top, &branch));
                    }
                    // The folder is gone or not a repo: keep what the agent recorded, if anything.
                    Err(_) => {}
                }
                cx.notify();
            },
        );
    }

    /// Off-thread work that does not need the gateway connection (git over SSH, etc.).
    pub(crate) fn background<T: Send + 'static>(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        job: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
        done: impl FnOnce(&mut Self, anyhow::Result<T>, &mut Window, &mut Context<Self>) + 'static,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move { job() }).await;
            let _ = this.update_in(cx, |this, window, cx| done(this, result, window, cx));
        })
        .detach();
    }

    /// Use `path` for the next new chat: its repo when it is in one, else the folder itself.
    pub fn set_new_project(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.target.clone();
        let p = path.clone();
        self.background(
            window,
            cx,
            move || connection::git(&target, &p, &["rev-parse", "--show-toplevel"]),
            move |this, r, _, cx| {
                match r {
                    Ok(top) => {
                        this.new_project = Some(Project::from_top(&top, "").root);
                        this.new_project_repo = true;
                    }
                    Err(_) => {
                        this.new_project = Some(path);
                        this.new_project_repo = false;
                    }
                }
                cx.notify();
            },
        );
    }

    pub(crate) fn current_title(&self) -> String {
        self.session
            .as_ref()
            .and_then(|id| self.sessions.iter().find(|s| &s.id == id))
            .map(|s| s.title.clone())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "Bench changes".into())
    }
}

impl BenchApp {
    /// A file or folder in Zed (the code editor beside Bench): `zed://file/<path>:<line>`, or
    /// over SSH `zed://ssh/<alias>/<path>` for a server's.
    pub fn open_in_zed(
        &mut self,
        path: &str,
        line: Option<usize>,
        host: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let at = line.map(|l| format!(":{l}")).unwrap_or_default();
        let url = match host
            .filter(|h| !h.is_empty() && *h != crate::inserts::terminal::session::LOCAL)
        {
            Some(h) => format!("zed://ssh/{h}{path}{at}"),
            None => format!("zed://file{path}{at}"),
        };
        cx.open_url(&url);
    }
}
