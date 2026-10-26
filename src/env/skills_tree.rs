//! What is done to the open skill's folder: read its tree, open a file of it, make, rename or
//! delete a file or folder.

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;
use serde_json::json;

use super::enc;
use crate::app::BenchApp;
use crate::rest;

impl BenchApp {
    /// Read the open skill's folder again (after a file or folder is made, renamed or removed).
    pub fn skill_load_tree(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(skill) = self.open_skill() else {
            return;
        };
        let pid = self
            .project_env
            .as_ref()
            .map(|e| e.id.clone())
            .unwrap_or_default();
        let Some((conn, _)) = self.env_target(&pid) else {
            return;
        };
        let profile = self.current_profile();
        let path = format!("/api/fs/tree?path={}", enc(&skill.dir));
        self.background(
            window,
            cx,
            move || rest::get_raw(&conn, &profile, &path),
            move |this, r, window, cx| match r {
                Ok(v) => {
                    if let Some(e) = this
                        .project_env
                        .as_mut()
                        .filter(|e| e.skills.open.as_deref() == Some(&skill.name))
                    {
                        e.skills.tree = v["entries"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|t| {
                                (
                                    t["path"].as_str().unwrap_or_default().to_string(),
                                    t["dir"].as_bool().unwrap_or(false),
                                )
                            })
                            .collect();
                    }
                    cx.notify();
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// A file of the open skill opens under its tree, highlighted as its kind.
    pub fn skill_open_file(&mut self, rel: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(skill) = self.open_skill() else {
            return;
        };
        let pid = self
            .project_env
            .as_ref()
            .map(|e| e.id.clone())
            .unwrap_or_default();
        let Some((conn, _)) = self.env_target(&pid) else {
            return;
        };
        let profile = self.current_profile();
        let path = skill.path(&rel);
        self.background(
            window,
            cx,
            move || rest::read_text(&conn, &profile, None, &path),
            move |this, r, window, cx| {
                match r {
                    Ok(t) if t.binary => this.toast(
                        NotificationType::Info,
                        "Not a text file: show it in Finder",
                        window,
                        cx,
                    ),
                    Ok(t) => {
                        if let Some(e) = this
                            .project_env
                            .as_mut()
                            .filter(|e| e.skills.open.as_deref() == Some(&skill.name))
                        {
                            let language = crate::lang::language_for_text(&rel, &t.text);
                            e.skills.file = Some(rel);
                            e.skills.text.update(cx, |s, cx| {
                                s.set_highlighter(language, cx);
                                s.set_value(t.text, window, cx)
                            });
                            e.skills.dirty = false;
                        }
                    }
                    Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
                }
                cx.notify();
            },
        );
    }

    /// A row of the skill's tree: a folder opens or shuts (and is where a new file goes), a file
    /// opens under the tree.
    pub fn skill_entry(
        &mut self,
        rel: String,
        dir: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        if dir {
            if !e.skills.expanded.remove(&rel) {
                e.skills.expanded.insert(rel.clone());
                e.skills.folder = rel;
            } else {
                e.skills.folder = parent_rel(&rel);
            }
            cx.notify();
            return;
        }
        if e.skills.dirty {
            self.toast(
                NotificationType::Info,
                "Save or revert the open file first",
                window,
                cx,
            );
            return;
        }
        e.skills.folder = parent_rel(&rel);
        self.skill_open_file(rel, window, cx);
    }

    /// "New file", "New folder" (in the folder last opened), or a row's rename: the field that
    /// names it, under the tree.
    pub fn skill_form(
        &mut self,
        what: &'static str,
        target: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        let name = if what == "rename" {
            target.rsplit('/').next().unwrap_or_default().to_string()
        } else {
            String::new()
        };
        e.skills.form_name.update(cx, |s, cx| {
            s.set_value(name, window, cx);
            s.focus(window, cx)
        });
        e.skills.form = Some((what, target));
        cx.notify();
    }

    /// Make what the field names (or rename), then read the tree again; a new file opens.
    pub fn skill_form_done(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(skill), Some(e)) = (self.open_skill(), self.project_env.as_ref()) else {
            return;
        };
        let Some((what, target)) = e.skills.form.clone() else {
            return;
        };
        let name = e
            .skills
            .form_name
            .read(cx)
            .value()
            .trim()
            .trim_matches('/')
            .to_string();
        if name.is_empty()
            || name.split('/').any(|p| p.is_empty() || p == "..")
            || (what == "rename" && name.contains('/'))
        {
            self.toast(
                NotificationType::Error,
                "A name (a file may go in a folder: a/b.md)",
                window,
                cx,
            );
            return;
        }
        let rel = match what {
            "rename" => {
                let up = parent_rel(&target);
                if up.is_empty() {
                    name.clone()
                } else {
                    format!("{up}/{name}")
                }
            }
            _ if target.is_empty() => name.clone(),
            _ => format!("{target}/{name}"),
        };
        let Some((conn, _)) = self.env_target(&e.id) else {
            return;
        };
        let profile = self.current_profile();
        let (path, from) = (skill.path(&rel), skill.path(&target));
        let rel2 = rel.clone();
        self.background(
            window,
            cx,
            move || match what {
                "folder" => rest::send_json(
                    &conn,
                    "POST",
                    "/api/fs/mkdir",
                    &profile,
                    json!({"path": path}),
                )
                .map(|_| ()),
                "file" => rest::write_text(&conn, &profile, None, &path, ""),
                _ => rest::send_json(
                    &conn,
                    "POST",
                    "/api/fs/rename",
                    &profile,
                    json!({"from": from, "to": path}),
                )
                .map(|_| ()),
            },
            move |this, r, window, cx| match r {
                Ok(()) => {
                    if let Some(e) = this.project_env.as_mut() {
                        e.skills.form = None;
                        let up = parent_rel(&rel2);
                        if !up.is_empty() {
                            e.skills.expanded.insert(up);
                        }
                        // What was renamed: the open file (or a folder above it) moves with it.
                        if what == "rename" {
                            if let Some(f) = e.skills.file.clone() {
                                if f == target {
                                    e.skills.file = Some(rel2.clone());
                                } else if let Some(rest) = f.strip_prefix(&format!("{target}/")) {
                                    e.skills.file = Some(format!("{rel2}/{rest}"));
                                }
                            }
                        }
                    }
                    this.skill_load_tree(window, cx);
                    if what == "file" {
                        this.skill_open_file(rel2, window, cx);
                    }
                    let id = this
                        .project_env
                        .as_ref()
                        .map(|e| e.id.clone())
                        .unwrap_or_default();
                    this.load_env(&id, window, cx);
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// A tree row's trash, twice: armed, then the file or the folder (with what is in it) goes.
    pub fn skill_delete(&mut self, rel: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(skill) = self.open_skill() else {
            return;
        };
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        let key = format!("skillfile:{rel}");
        if e.armed.as_deref() != Some(&key) {
            e.armed = Some(key);
            cx.notify();
            return;
        }
        e.armed = None;
        let id = e.id.clone();
        let Some((conn, _)) = self.env_target(&id) else {
            return;
        };
        let profile = self.current_profile();
        let path = skill.path(&rel);
        self.background(
            window,
            cx,
            move || {
                rest::send_json(
                    &conn,
                    "POST",
                    "/api/fs/delete",
                    &profile,
                    json!({"path": path}),
                )
            },
            move |this, r, window, cx| match r {
                Ok(_) => {
                    if let Some(e) = this.project_env.as_mut() {
                        if e.skills
                            .file
                            .as_ref()
                            .is_some_and(|f| *f == rel || f.starts_with(&format!("{rel}/")))
                        {
                            e.skills.file = None;
                            e.skills.dirty = false;
                        }
                    }
                    this.skill_load_tree(window, cx);
                    let id = this
                        .project_env
                        .as_ref()
                        .map(|e| e.id.clone())
                        .unwrap_or_default();
                    this.load_env(&id, window, cx);
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }
}

/// The folder a path inside a skill is in ("" its top).
fn parent_rel(rel: &str) -> String {
    rel.rsplit_once('/')
        .map(|(p, _)| p.to_string())
        .unwrap_or_default()
}
