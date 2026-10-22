//! Git projects: which repo (and branch) a session works in, and the few git actions on it —
//! a chat in its own branch (a worktree folder), merge back, delete.
//!
//! The project comes from the session's folder when Hermes recorded one, else from the files
//! the agent edited (their repo), remembered per profile. Chats without files have none.

use std::collections::HashMap;

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use super::HermesApp;
use crate::connection;
use crate::model::{Project, Target};
use crate::rest;

/// A git action that needs a second click.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitArm {
    Delete,
}

impl HermesApp {
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

    /// Load what Hermes knows about each session's folder, plus our own inferences.
    pub(super) fn load_places(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        // A branch folder is recognizable before Hermes records its repo.
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

    /// The folder a session was started in, when it is not Hermes' default (the user chose it).
    pub fn folder_of(&self, session: &str) -> Option<String> {
        // Until Hermes' default folder is known, nothing counts as chosen (and nothing is probed).
        let default = self.default_cwd.as_ref()?;
        let cwd = &self.places.get(session)?.cwd;
        (!cwd.is_empty() && cwd != default).then(|| cwd.clone())
    }

    /// Every project seen in this profile (for the new-chat picker), by name.
    pub fn known_projects(&self) -> Vec<String> {
        let mut roots: Vec<String> = self
            .places
            .values()
            .filter(|p| !p.repo.is_empty())
            .map(|p| Project::from_top(&p.repo, "").root)
            .chain(
                self.inferred
                    .values()
                    .map(|t| Project::from_top(t, "").root),
            )
            .collect();
        roots.sort_by_key(|r| r.rsplit('/').next().unwrap_or(r).to_lowercase());
        roots.dedup();
        roots
    }

    /// Work out the open session's project (and its live branch); infer it from edited files
    /// when Hermes has none.
    pub(super) fn resolve_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
            move |this, r: anyhow::Result<(String, String)>, window, cx| {
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
                        // The Files tab follows the chat's project.
                        if this.right == Some(super::RightTab::Files) && this.tree_root.is_none() {
                            this.load_tree_root(window, cx);
                        }
                    }
                    // The folder is gone or not a repo: keep what Hermes recorded, if anything.
                    Err(_) => {}
                }
                cx.notify();
            },
        );
    }

    /// Off-thread work that does not need the gateway connection (git over SSH, etc.).
    pub(super) fn background<T: Send + 'static>(
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

    /// Start a new chat in `root`, optionally in a fresh branch of its own.
    pub fn new_chat_in(&mut self, root: Option<String>, branch: bool, cx: &mut Context<Self>) {
        self.new_session(cx);
        self.new_project = root;
        self.new_branch = branch && self.new_project.is_some();
        cx.notify();
    }

    /// "Choose folder…": the macOS dialog for this Mac; a typed path for an SSH host.
    pub fn choose_project_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.target, Target::Local) {
            self.path_entry = true;
            cx.notify();
            return;
        }
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Work in this folder".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update_in(cx, |this, window, cx| {
                this.set_new_project(path.to_string_lossy().to_string(), window, cx)
            });
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
                        this.new_branch = false;
                    }
                }
                cx.notify();
            },
        );
    }

    /// Merge this chat's branch into the default branch of the main checkout. Uncommitted work
    /// in the branch folder is committed first (titled after the chat).
    pub fn merge_branch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.project.clone().filter(|p| p.in_branch_folder()) else {
            return;
        };
        let target = self.target.clone();
        let title = self.current_title();
        self.toast(
            NotificationType::Info,
            format!("Merging {}…", p.branch),
            window,
            cx,
        );
        self.background(
            window,
            cx,
            move || merge(&target, &p, &title),
            |this, r, window, cx| match r {
                Ok(msg) => this.toast(NotificationType::Success, msg, window, cx),
                Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
            },
        );
    }

    /// Delete this chat's branch and its folder (second click confirms).
    pub fn delete_branch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.project.clone().filter(|p| p.in_branch_folder()) else {
            return;
        };
        if self.git_armed != Some(GitArm::Delete) {
            self.git_armed = Some(GitArm::Delete);
            self.toast(
                NotificationType::Warning,
                format!(
                    "Choose “Delete branch” again to delete {} and its folder",
                    p.branch
                ),
                window,
                cx,
            );
            cx.notify();
            return;
        }
        self.git_armed = None;
        let target = self.target.clone();
        let Some(conn) = self.conn.clone() else {
            return;
        };
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || {
                rest::worktree_remove(&conn, &profile, &p.root, &p.dir)?;
                connection::git(&target, &p.root, &["branch", "-D", &p.branch])?;
                Ok(p.branch)
            },
            |this, r, window, cx| match r {
                Ok(branch) => {
                    this.toast(
                        NotificationType::Success,
                        format!("Deleted {branch}"),
                        window,
                        cx,
                    );
                    this.resolve_project(window, cx);
                }
                Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
            },
        );
    }

    fn current_title(&self) -> String {
        self.session
            .as_ref()
            .and_then(|id| self.sessions.iter().find(|s| &s.id == id))
            .map(|s| s.title.clone())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "Hermes changes".into())
    }
}

pub(crate) fn merge(target: &Target, p: &Project, title: &str) -> anyhow::Result<String> {
    let git = |dir: &str, args: &[&str]| connection::git(target, dir, args);
    if !git(&p.dir, &["status", "--porcelain"])?.is_empty() {
        git(&p.dir, &["add", "-A"])?;
        git(&p.dir, &["commit", "-m", title])?;
    }
    let main_branch = git(&p.root, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    if main_branch == p.branch {
        anyhow::bail!("the main folder is on {main_branch} itself");
    }
    if let Err(e) = git(&p.root, &["merge", "--no-ff", "--no-edit", &p.branch]) {
        let _ = git(&p.root, &["merge", "--abort"]);
        anyhow::bail!("Merge into {main_branch} failed, nothing changed: {e:#}");
    }
    Ok(format!("Merged {} into {main_branch}", p.branch))
}
