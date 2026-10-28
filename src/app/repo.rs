//! The git repo a thread works in (its folder's, or — a thread with no folder of its own — the
//! repo of the files its agent edited, remembered here), and what the title bar's repo chip shows
//! of it: its branch, uncommitted files, newest commits.

use std::collections::HashMap;

use gpui_kit::*;

use crate::app::BenchApp;
use crate::app::model::{CommitBrief, Repo, RepoInfo, Target};
use crate::client::connection;
use crate::thread::ThreadRef;

/// Repos inferred from edited files, by thread (its agent and id), kept between starts.
fn inferred_path() -> std::path::PathBuf {
    connection::state_dir().join("inferred-repos.json")
}

fn inferred() -> HashMap<String, String> {
    std::fs::read(inferred_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// A thread's key among the inferred repos: its agent and its id.
fn inferred_key(at: &ThreadRef) -> Option<String> {
    Some(format!("{}/{}", at.host, at.id.as_ref()?))
}

impl BenchApp {
    /// The repo a thread works in, as far as its agent's list says without asking git.
    pub fn repo_of(&self, at: &ThreadRef) -> Option<Repo> {
        let row = at
            .id
            .as_ref()
            .and_then(|id| self.threads.iter().find(|r| r.host == at.host && &r.id == id));
        if let Some(top) = row.and_then(|r| r.repo.clone()) {
            return Some(Repo::from_top(&top, ""));
        }
        inferred_key(at)
            .and_then(|k| inferred().remove(&k))
            .map(|top| Repo::from_top(&top, ""))
    }

    /// The folder a thread was started in, when it is not the agent's default (the person chose
    /// it).
    pub fn folder_of(&self, at: &ThreadRef) -> Option<String> {
        // Until the agent's default folder is known, nothing counts as chosen (and nothing is probed).
        let default = self.default_cwd.as_ref()?;
        let id = at.id.as_ref()?;
        let cwd = &self
            .threads
            .iter()
            .find(|r| r.host == at.host && &r.id == id)?
            .cwd;
        (!cwd.is_empty() && cwd != default).then(|| cwd.clone())
    }

    /// Work out the open thread's repo and its live branch; infer it from the edited files when
    /// its folder is none.
    pub(crate) fn resolve_repo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let at = self.thread_ref(cx);
        if at.id.is_none() {
            self.active_thread.update(cx, |v, _| v.thread.repo = None);
            return;
        }
        let known = self.repo_of(&at);
        let probe = match (&known, self.folder_of(&at)) {
            (Some(p), _) => p.dir.clone(),
            (None, Some(f)) => f,
            (None, None) => {
                // The first edited file's folder: is it inside a repo?
                let Some(dir) = self
                    .active_thread
                    .read(cx)
                    .session_files()
                    .first()
                    .and_then(|(p, _)| p.rsplit_once('/').map(|(d, _)| d.to_string()))
                else {
                    self.active_thread.update(cx, |v, _| v.thread.repo = None);
                    return;
                };
                dir
            }
        };
        if known.is_some() {
            self.active_thread.update(cx, |v, _| v.thread.repo = known.clone());
        }
        let target = self.thread_target(&at);
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
                if this.thread_ref(cx).id != at.id {
                    return;
                }
                // The folder is gone or not a repo: keep what the agent recorded, if anything.
                let Ok((top, branch)) = r else {
                    return;
                };
                if known.is_none()
                    && let Some(key) = inferred_key(&at)
                {
                    let mut all = inferred();
                    all.insert(key, top.clone());
                    let _ = std::fs::create_dir_all(connection::state_dir());
                    let _ = std::fs::write(inferred_path(), serde_json::to_vec(&all).unwrap_or_default());
                }
                this.active_thread
                    .update(cx, |v, _| v.thread.repo = Some(Repo::from_top(&top, &branch)));
                // The branch may have moved: the chip reads the repo again.
                this.git_info.remove(&top);
                cx.notify();
            },
        );
    }

    /// Read what the repo chip shows for one repo folder: uncommitted count, newest commits,
    /// origin URL. Once per folder until it is dropped again (a run ending, a new branch).
    pub(crate) fn refresh_git(&mut self, target: Target, dir: String, cx: &mut Context<Self>) {
        if self.git_info.contains_key(&dir) || !self.git_reading.insert(dir.clone()) {
            return;
        }
        cx.spawn(async move |this, cx| {
            let at = dir.clone();
            let r = cx
                .background_spawn(async move {
                    let dirty = connection::git(&target, &at, &["status", "--porcelain"])
                        .unwrap_or_default()
                        .lines()
                        .count();
                    let log = connection::git(
                        &target,
                        &at,
                        &["log", "-n", "8", "--pretty=format:%h\x1f%s\x1f%ar"],
                    )
                    .unwrap_or_default();
                    let commits = log
                        .lines()
                        .filter_map(|l| {
                            let mut f = l.split('\x1f');
                            Some(CommitBrief {
                                sha: f.next()?.to_string(),
                                subject: f.next()?.to_string(),
                                age: f.next().unwrap_or_default().to_string(),
                            })
                        })
                        .collect();
                    let remote =
                        connection::git(&target, &at, &["config", "--get", "remote.origin.url"])
                            .ok()
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty());
                    RepoInfo {
                        dirty,
                        commits,
                        remote,
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.git_reading.remove(&dir);
                this.git_info.insert(dir, r);
                cx.notify();
            });
        })
        .detach();
    }

    /// Off-thread work that does not need the agent's connection (git over SSH, etc.).
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
        let p = path.clone();
        self.background(
            window,
            cx,
            move || connection::git(&Target::Local, &p, &["rev-parse", "--show-toplevel"]),
            move |this, r, _, cx| {
                match r {
                    Ok(top) => {
                        this.new_project = Some(Repo::from_top(&top, "").root);
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

    /// A thread's title as its row has it.
    pub(crate) fn title_of(&self, at: &ThreadRef) -> String {
        at.id
            .as_ref()
            .and_then(|id| self.threads.iter().find(|s| s.host == at.host && &s.id == id))
            .map(|s| s.title.clone())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "Bench".into())
    }
}
