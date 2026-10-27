//! What is done in the tree: a folder listed when it opens, a name typed for a new file, folder,
//! prompt or skill or for a rename, an entry deleted (asked once), ⌘P to open a file of the
//! project's folder by name, ⌘⇧F to find a text in it. Every call goes to the agent that keeps
//! the project (`/api/fs/*`, on the project's machine when it works over SSH).

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;
use serde_json::json;

use super::{DocKey, Entry, Explorer, FileRef, Named, Naming};
use crate::app::BenchApp;
use crate::client::rest;
use crate::workspace::SidebarPanel;

impl BenchApp {
    /// The Explorer on `project` (its environment read, its tree as it was left).
    pub fn show_explorer(&mut self, project: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.explorer.project != project {
            // Another project: its own tree; open documents stay in their tabs.
            self.explorer.project = project.clone();
            self.explorer.open = ["files", "instructions"].map(String::from).into();
            self.explorer.listings.clear();
            self.explorer.find = None;
        }
        // The IDE takes the window: the tree on the left, documents in the center (its last one
        // again, or a word on how to open one).
        self.sidebar_panel = SidebarPanel::Explorer;
        self.center = crate::workspace::Center::Doc;
        if self
            .explorer
            .active
            .as_ref()
            .is_some_and(|k| k.project() != project)
        {
            self.explorer.active = self
                .explorer
                .tabs
                .iter()
                .rev()
                .find(|k| k.project() == project)
                .cloned();
        }
        self.load_env(&project, window, cx);
        if let Some(root) = self.explorer_root() {
            self.list_folder(root, window, cx);
        }
        cx.notify();
    }

    /// The IDE closed (its pressed button atop the sidebar, ⌘E): the threads again, the thread
    /// that was open in the center, nothing beside it. Its documents stay for the next time.
    pub fn close_explorer(&mut self, cx: &mut Context<Self>) {
        self.sidebar_panel = SidebarPanel::Threads;
        self.center = crate::workspace::Center::Chat;
        self.right = None;
        let ex = &mut self.explorer;
        ex.quick = None;
        ex.find = None;
        ex.naming = None;
        ex.armed = None;
        cx.notify();
    }

    /// ⌘E: the IDE closed, or opened on the open thread's project.
    pub fn toggle_side(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sidebar_panel == SidebarPanel::Explorer {
            self.close_explorer(cx);
        } else {
            let project = self.thread_project_or_host();
            self.show_explorer(project, window, cx);
        }
    }

    /// The project's folder, where it is (None: it has none, an agent's Host).
    pub fn explorer_root(&self) -> Option<FileRef> {
        let p = &self.explorer.project;
        let at = self.location(p);
        Some(FileRef {
            project: p.clone(),
            machine: at.files_machine(),
            path: at.folder.filter(|f| !f.is_empty())?,
        })
    }

    /// A file of the agent's own (the soul, a prompt, a skill's file) as the Explorer reaches it.
    pub fn agent_file(&self, path: String) -> FileRef {
        FileRef {
            project: self.explorer.project.clone(),
            machine: None,
            path,
        }
    }

    /// A folder of the tree opened or shut; listed the first time it opens.
    pub fn toggle_folder(&mut self, f: FileRef, window: &mut Window, cx: &mut Context<Self>) {
        let node = format!("d:{}", f.listing_key());
        if !self.explorer.open.remove(&node) {
            self.explorer.open.insert(node);
            if !self.explorer.listings.contains_key(&f.listing_key()) {
                self.list_folder(f, window, cx);
            }
        }
        cx.notify();
    }

    /// A node of the tree that is not a folder ("prompts", "mcp"…) opened or shut.
    pub fn toggle_node(&mut self, node: &str, cx: &mut Context<Self>) {
        if !self.explorer.open.remove(node) {
            self.explorer.open.insert(node.to_string());
        }
        cx.notify();
    }

    /// Read a folder's entries: folders first, then by name.
    pub fn list_folder(&mut self, f: FileRef, window: &mut Window, cx: &mut Context<Self>) {
        if self.demo {
            let entries = crate::demo::listing(&f.path)
                .into_iter()
                .map(|(name, path, dir)| Entry { name, path, dir })
                .collect();
            self.explorer.listings.insert(f.listing_key(), entries);
            return;
        }
        let Some((conn, _)) = self.env_target(&f.project) else {
            return;
        };
        let profile = self.current_profile();
        let (machine, path) = (f.machine.clone(), f.path.clone());
        self.background(
            window,
            cx,
            move || rest::list_dir(&conn, &profile, machine.as_deref(), &path),
            move |this, r, window, cx| {
                match r {
                    Ok(list) => {
                        let mut entries: Vec<Entry> = list
                            .into_iter()
                            .filter(|(name, _, _)| name != ".DS_Store" && name != ".git")
                            .map(|(name, path, dir)| Entry { name, path, dir })
                            .collect();
                        entries.sort_by(|a, b| {
                            b.dir
                                .cmp(&a.dir)
                                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                        });
                        this.explorer.listings.insert(f.listing_key(), entries);
                    }
                    Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
                }
                cx.notify();
            },
        );
    }

    /// Every listed folder read again (the agent may have changed them).
    pub fn refresh_explorer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let project = self.explorer.project.clone();
        let root = self.explorer_root();
        let keys: Vec<String> = self.explorer.listings.keys().cloned().collect();
        for key in keys {
            let (machine, path) = key.split_once(':').unwrap_or(("", &key));
            let f = FileRef {
                project: project.clone(),
                machine: (!machine.is_empty()).then(|| machine.to_string()),
                path: path.to_string(),
            };
            self.list_folder(f, window, cx);
        }
        if let Some(root) = root {
            self.list_folder(root, window, cx);
        }
        self.load_env(&project, window, cx);
    }

    /// A name to type, in the tree where it will go.
    pub fn start_naming(&mut self, what: Named, window: &mut Window, cx: &mut Context<Self>) {
        let start = match &what {
            Named::Rename(f) => f.name().to_string(),
            _ => String::new(),
        };
        let placeholder = match &what {
            Named::File(_) => "New file's name",
            Named::Folder(_) => "New folder's name",
            Named::Rename(_) => "Its new name",
            Named::Prompt => "The prompt's name",
            Named::Skill => "The skill's name",
        };
        // A folder a new entry goes into is shown open.
        if let Named::File(d) | Named::Folder(d) = &what {
            let node = format!("d:{}", d.listing_key());
            if !self.explorer.open.contains(&node) {
                self.toggle_folder(d.clone(), window, cx);
            }
        }
        match &what {
            Named::Prompt => {
                self.explorer
                    .open
                    .extend(["instructions".into(), "prompts".into()]);
            }
            Named::Skill => {
                self.explorer
                    .open
                    .extend(["instructions".into(), "skills".into()]);
            }
            _ => {}
        }
        let input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder(placeholder);
            s.set_value(start, window, cx);
            s
        });
        cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.finish_naming(window, cx);
            }
        })
        .detach();
        input.update(cx, |s, cx| s.focus(window, cx));
        self.explorer.naming = Some(Naming { what, input });
        cx.notify();
    }

    /// Enter in the name: it is made (or renamed), its folder read again, a new file opened.
    pub fn finish_naming(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(n) = self.explorer.naming.as_ref() else {
            return;
        };
        let what = n.what.clone();
        let name = n
            .input
            .read(cx)
            .value()
            .trim()
            .trim_matches('/')
            .to_string();
        let bad = name.is_empty()
            || name
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == "..")
            || (matches!(what, Named::Rename(_) | Named::Prompt | Named::Skill)
                && name.contains('/'));
        if bad {
            self.toast(
                NotificationType::Error,
                "A name (a new file may go in a folder: a/b.md)",
                window,
                cx,
            );
            return;
        }
        let project = self.explorer.project.clone();
        let Some((conn, eid)) = self.env_target(&project) else {
            return;
        };
        let dir = self
            .envs
            .get(&project)
            .map(|e| e.dir.clone())
            .unwrap_or_default();
        let profile = self.current_profile();
        // What is called, and what opens after: the new file, or nothing.
        let (call, opens): (
            Box<dyn FnOnce() -> anyhow::Result<()> + Send>,
            Option<FileRef>,
        ) = match &what {
            Named::File(d) => {
                let f = d.join(&name);
                let (m, p) = (f.machine.clone(), f.path.clone());
                (
                    Box::new(move || rest::write_text(&conn, &profile, m.as_deref(), &p, "")),
                    Some(f),
                )
            }
            Named::Folder(d) => {
                let f = d.join(&name);
                let body = json!({"path": f.path, "machine": f.machine});
                (
                    Box::new(move || {
                        rest::send_json(&conn, "POST", "/api/fs/mkdir", &profile, body).map(|_| ())
                    }),
                    None,
                )
            }
            Named::Rename(f) => {
                let to = f.parent().join(&name);
                let body = json!({"from": f.path, "to": to.path, "machine": f.machine});
                (
                    Box::new(move || {
                        rest::send_json(&conn, "POST", "/api/fs/rename", &profile, body).map(|_| ())
                    }),
                    None,
                )
            }
            Named::Prompt => {
                let f =
                    self.agent_file(format!("{dir}/prompts/{}.md", name.trim_end_matches(".md")));
                let p = f.path.clone();
                (
                    Box::new(move || rest::write_text(&conn, &profile, None, &p, "")),
                    Some(f),
                )
            }
            Named::Skill => {
                let text =
                    format!("---\nname: {name}\ndescription: \n---\n\nWhat to do, step by step.\n");
                let path = format!("/api/projects/{eid}/skills");
                let f = self.agent_file(format!("{dir}/skills/{name}/SKILL.md"));
                (
                    Box::new(move || {
                        rest::send_json(&conn, "POST", &path, &profile, json!({"text": text}))
                            .map(|_| ())
                    }),
                    Some(f),
                )
            }
        };
        self.background(window, cx, call, move |this, r, window, cx| match r {
            Ok(()) => {
                this.explorer.naming = None;
                let touched = match &what {
                    Named::File(d) | Named::Folder(d) => d.join(&name),
                    Named::Rename(f) => {
                        let to = f.parent().join(&name);
                        this.moved(f, &to);
                        to
                    }
                    _ => opens
                        .clone()
                        .unwrap_or_else(|| this.agent_file(dir.clone())),
                };
                this.list_folder(touched.parent(), window, cx);
                this.after_write(&touched, window, cx);
                if let Some(f) = opens {
                    this.open_doc(DocKey::File(f), window, cx);
                }
            }
            Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
        });
    }

    /// A rename: the open tabs follow the entry.
    fn moved(&mut self, from: &FileRef, to: &FileRef) {
        let follow = |f: &FileRef| -> Option<FileRef> {
            f.inside(from).then(|| FileRef {
                path: format!("{}{}", to.path, &f.path[from.path.len()..]),
                ..f.clone()
            })
        };
        let ex = &mut self.explorer;
        for t in ex.tabs.iter_mut() {
            if let DocKey::File(f) = t {
                if let Some(n) = follow(f) {
                    let doc = ex.docs.remove(&DocKey::File(f.clone()));
                    *f = n.clone();
                    if let Some(doc) = doc {
                        ex.docs.insert(DocKey::File(n), doc);
                    }
                }
            }
        }
        if let Some(DocKey::File(f)) = ex.active.as_mut() {
            if let Some(n) = follow(f) {
                *f = n;
            }
        }
    }

    /// A tree row's trash, twice: armed, then the file or the folder (with what is in it) goes,
    /// and what had it open closes.
    pub fn delete_entry(&mut self, f: FileRef, window: &mut Window, cx: &mut Context<Self>) {
        let armed = format!("del:{}", f.listing_key());
        if self.explorer.armed.as_deref() != Some(&armed) {
            self.explorer.armed = Some(armed);
            cx.notify();
            return;
        }
        self.explorer.armed = None;
        let Some((conn, _)) = self.env_target(&f.project) else {
            return;
        };
        let profile = self.current_profile();
        let body = json!({"path": f.path, "machine": f.machine});
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "POST", "/api/fs/delete", &profile, body),
            move |this, r, window, cx| match r {
                Ok(_) => {
                    let gone: Vec<DocKey> = this
                        .explorer
                        .tabs
                        .iter()
                        .filter(|t| matches!(t, DocKey::File(o) if o.inside(&f)))
                        .cloned()
                        .collect();
                    for k in gone {
                        this.explorer.docs.remove(&k);
                        this.close_doc(k, cx);
                    }
                    this.list_folder(f.parent(), window, cx);
                    this.after_write(&f, window, cx);
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }
}

impl Explorer {
    /// Whether a node of the tree is open.
    pub fn is_open(&self, node: &str) -> bool {
        self.open.contains(node)
    }
}
