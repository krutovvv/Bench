//! The sidebar in the Explorer view: the project chosen at the top, then its tree — the folder
//! (on its machine, listed as folders open), Instructions (the soul, prompts, skills: the agent's
//! own files), and the environment's parts that are not files, by their `Section`'s group
//! (Tools: MCP and language servers; Schedule). ⌘⇧F's results take the tree's place while open.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{DocKey, FileRef, Named};
use crate::app::BenchApp;
use crate::env::{Home, SECTIONS};
use crate::icons::Ico;

impl BenchApp {
    /// The sidebar's list in the Explorer view.
    pub(crate) fn render_explorer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        // The demo opens on the blog (`demo::load`), read here once.
        if self.demo && !self.envs.contains_key(&self.explorer.project) {
            let id = self.explorer.project.clone();
            self.load_env(&id, window, cx);
        }
        let mut out = vec![self.explorer_picker(cx)];
        if self.explorer.find.is_some() {
            out.extend(self.find_rows(cx));
            return out;
        }
        if let Some(root) = self.explorer_root() {
            let name = root.name().to_string();
            let menu = super::menus::root_menu(cx.entity(), root.clone());
            out.push(
                self.heading_el("files", &name, vec![], cx)
                    .context_menu(menu)
                    .into_any_element(),
            );
            if self.explorer.is_open("files") {
                out.extend(self.folder_rows(&root, 0, cx));
            }
        }
        out.push(self.heading("instructions", "Instructions", vec![], cx));
        if self.explorer.is_open("instructions") {
            out.extend(self.instruction_rows(cx));
        }
        // The environment's parts, by the group their sections name.
        let Some(e) = self.envs.get(&self.explorer.project) else {
            return out;
        };
        let mut groups: Vec<&'static str> = vec![];
        for s in SECTIONS.iter().filter(|s| s.shown(self, e)) {
            if let Home::Explorer(g) = s.home() {
                if !groups.contains(&g) {
                    groups.push(g);
                }
            }
        }
        let general = crate::model::ProjectKey::parse(&e.id).is_host();
        for g in groups {
            let node = format!("g:{g}");
            // A group of one section named as it (Schedule): its actions on the group's heading,
            // its parts right under it.
            let only: Vec<_> = SECTIONS
                .iter()
                .filter(|s| s.home() == Home::Explorer(g) && s.shown(self, e))
                .collect();
            let alone = only.len() == 1 && only[0].block(general) == g;
            let acts = if alone {
                only[0].head(self, e, cx).into_iter().collect()
            } else {
                vec![]
            };
            out.push(self.heading(&node, g, acts, cx));
            if self.explorer.is_open(&node) {
                out.extend(self.part_rows(e, g, alone, cx));
            }
        }
        // Last, what the project is set to: its settings, a document like the rest.
        let settings = DocKey::Settings {
            project: self.explorer.project.clone(),
        };
        let theme = cx.theme().clone();
        out.push(bench_ui::tree_heading("Project", vec![], cx).into_any_element());
        out.push(
            bench_ui::tree_row(
                "ex-settings",
                0,
                None,
                (Ico::Settings, theme.muted_foreground),
                "Settings",
                Some("name, folder, model, Telegram".into()),
                self.explorer.active.as_ref() == Some(&settings),
                vec![],
                cx,
            )
            .on_click(
                cx.listener(move |this, _, window, cx| this.open_doc(settings.clone(), window, cx)),
            )
            .into_any_element(),
        );
        out
    }

    /// The top of the Explorer: the project shown, another chosen from a menu; reading it again.
    fn explorer_picker(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let id = self.explorer.project.clone();
        let name = self
            .projects
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| crate::app::LOCAL_HOST.into());
        let choices: Vec<(String, String)> = std::iter::once((
            crate::model::ProjectKey::HOST.to_string(),
            crate::app::LOCAL_HOST.to_string(),
        ))
        .chain(self.projects.iter().map(|p| (p.id.clone(), p.name.clone())))
        .collect();
        let view = cx.entity();
        let picker = Button::new("ex-project")
            .ghost()
            .small()
            .label(name)
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu;
                for (pid, pname) in &choices {
                    let (view, pid) = (view.clone(), pid.clone());
                    let checked = pid == id;
                    menu = menu.item(PopupMenuItem::new(pname.clone()).checked(checked).on_click(
                        move |_, window, cx| {
                            let pid = pid.clone();
                            view.update(cx, |this, cx| this.show_explorer(pid, window, cx))
                        },
                    ));
                }
                menu
            });
        // Its buttons end where the strip's above do: the same buttons, the same inset.
        h_flex()
            .flex_none()
            .pl_1()
            .pr(px(bench_ui::SIDEBAR_STRIP_PR))
            .pb_1()
            .gap_1()
            .items_center()
            .child(div().min_w_0().child(picker))
            .child(div().flex_1())
            .child(
                bench_ui::icon_button("ex-refresh", Ico::RefreshCw, "Read it again", false, cx)
                    .on_click(cx.listener(|this, _, w, cx| this.refresh_explorer(w, cx))),
            )
            .child(
                bench_ui::icon_button("ex-quick", Ico::Search, "Open a file  ⌘P", false, cx)
                    .on_click(cx.listener(|this, _, w, cx| this.open_quick(w, cx))),
            )
            .into_any_element()
    }

    /// A group's heading: a click folds it.
    fn heading(
        &self,
        node: &str,
        title: &str,
        actions: Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.heading_el(node, title, actions, cx).into_any_element()
    }

    fn heading_el(
        &self,
        node: &str,
        title: &str,
        actions: Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let node = node.to_string();
        bench_ui::tree_heading(title.to_string(), actions, cx)
            .id(SharedString::from(format!("ex-h-{node}")))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_node(&node, cx)))
    }

    /// A folder's entries under it, `depth` deep; its open folders' entries under them.
    fn folder_rows(&self, dir: &FileRef, depth: usize, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut out = vec![];
        out.extend(self.naming_row(&super::Named::File(dir.clone()), depth));
        out.extend(self.naming_row(&super::Named::Folder(dir.clone()), depth));
        let Some(entries) = self.explorer.listings.get(&dir.listing_key()) else {
            out.push(quiet("Reading…", depth, cx));
            return out;
        };
        if entries.is_empty() {
            out.push(quiet("Empty", depth, cx));
        }
        for e in entries.clone() {
            let f = FileRef {
                path: e.path.clone(),
                ..dir.clone()
            };
            if self.renaming(&f) {
                out.extend(self.naming_row(&Named::Rename(f.clone()), depth));
                continue;
            }
            out.push(self.entry_row(&f, e.dir, depth, true, None, cx));
            if e.dir && self.explorer.is_open(&format!("d:{}", f.listing_key())) {
                out.extend(self.folder_rows(&f, depth + 1, cx));
            }
        }
        out
    }

    /// One file or folder of the tree; its menu (`menus.rs`); `editable`: it can be renamed and
    /// deleted.
    fn entry_row(
        &self,
        f: &FileRef,
        dir: bool,
        depth: usize,
        editable: bool,
        note: Option<SharedString>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let key = f.listing_key();
        let open = dir && self.explorer.is_open(&format!("d:{key}"));
        let chosen = !dir && self.explorer.active.as_ref() == Some(&DocKey::File(f.clone()));
        let icon = if open {
            (Ico::FolderOpen, theme.blue)
        } else {
            crate::ui::file_icon(f.name(), dir, &theme)
        };
        // Its work is in its menu; only a delete asked for shows here, to be confirmed.
        let armed =
            self.explorer.armed.as_deref() == Some(format!("del:{}", f.listing_key()).as_str());
        let acts = if armed {
            vec![self.trash(f, cx)]
        } else {
            vec![]
        };
        let target = f.clone();
        bench_ui::tree_row(
            format!("ex-e-{key}"),
            depth,
            dir.then_some(open),
            icon,
            f.name().to_string(),
            note,
            chosen,
            acts,
            cx,
        )
        .on_click(cx.listener(move |this, _, window, cx| {
            if dir {
                this.toggle_folder(target.clone(), window, cx)
            } else {
                this.open_doc(DocKey::File(target.clone()), window, cx)
            }
        }))
        .context_menu(super::menus::entry_menu(
            cx.entity(),
            f.clone(),
            dir,
            editable,
        ))
        .into_any_element()
    }

    /// A delete asked for in the menu, to confirm (Esc drops it).
    fn trash(&self, f: &FileRef, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let armed =
            self.explorer.armed.as_deref() == Some(format!("del:{}", f.listing_key()).as_str());
        let target = f.clone();
        div()
            .id(SharedString::from(format!("ex-rm-{}", f.listing_key())))
            .flex_none()
            .px_1()
            .cursor_pointer()
            .text_xs()
            .when(!armed, |d| {
                d.text_color(theme.muted_foreground)
                    .hover(|s| s.text_color(theme.danger))
                    .child(Ico::Trash.icon().small())
            })
            .when(armed, |d| d.text_color(theme.danger).child("Delete?"))
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.delete_entry(target.clone(), window, cx)
            }))
            .into_any_element()
    }

    fn renaming(&self, f: &FileRef) -> bool {
        matches!(&self.explorer.naming, Some(n) if n.what == Named::Rename(f.clone()))
    }

    /// The field a name is typed in, where `what` puts it.
    fn naming_row(&self, what: &Named, depth: usize) -> Option<AnyElement> {
        let n = self.explorer.naming.as_ref().filter(|n| n.what == *what)?;
        let indent = match what {
            Named::Rename(_) => depth,
            _ => depth + 1,
        };
        Some(
            div()
                .flex_none()
                .pl(bench_ui::tree_pl(indent))
                .pr_2()
                .py_0p5()
                .child(Input::new(&n.input).small())
                .into_any_element(),
        )
    }

    /// The soul, the prompts, the skills (each a folder of its own).
    fn instruction_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let theme = cx.theme().clone();
        let Some(e) = self.envs.get(&self.explorer.project) else {
            return vec![quiet("Reading…", 0, cx)];
        };
        let mut out = vec![];
        if !e.dir.is_empty() {
            let soul = self.agent_file(format!("{}/SYSTEM.md", e.dir));
            let chosen = self.explorer.active.as_ref() == Some(&DocKey::File(soul.clone()));
            out.push(
                bench_ui::tree_row(
                    "ex-soul",
                    0,
                    None,
                    (Ico::FileText, theme.muted_foreground),
                    "SYSTEM.md",
                    Some("the soul".into()),
                    chosen,
                    vec![],
                    cx,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_doc(DocKey::File(soul.clone()), window, cx)
                }))
                .into_any_element(),
            );
        }
        // Prompts: files of its own prompts folder.
        out.push(
            self.node_row("prompts", "Prompts", Ico::Folder, None, vec![], cx)
                .context_menu(super::menus::new_menu(
                    cx.entity(),
                    Named::Prompt,
                    "New prompt",
                ))
                .into_any_element(),
        );
        if self.explorer.is_open("prompts") {
            out.extend(self.naming_row(&Named::Prompt, 0));
            if e.instructions.prompts.is_empty() {
                out.push(quiet("None of its own", 1, cx));
            }
            for name in e.instructions.prompts.clone() {
                let f = self.agent_file(format!("{}/prompts/{name}.md", e.dir));
                out.push(self.entry_row(&f, false, 1, true, None, cx));
            }
        }
        // Skills, by where each lives; each its folder.
        out.push(
            self.node_row("skills", "Skills", Ico::Folder, None, vec![], cx)
                .context_menu(super::menus::new_menu(
                    cx.entity(),
                    Named::Skill,
                    "New skill",
                ))
                .into_any_element(),
        );
        if self.explorer.is_open("skills") {
            out.extend(self.naming_row(&Named::Skill, 0));
            let general = crate::model::ProjectKey::parse(&e.id).is_host();
            let remote = crate::env::is_remote(self, &e.id);
            let machine = self.location(&e.id).files_machine();
            let scopes: [(&str, &str); 3] = [
                ("project", "This project"),
                ("repo", "Its folder"),
                ("general", if remote { "The server's" } else { "This Mac" }),
            ];
            for (scope, label) in scopes {
                let skills: Vec<_> = e
                    .instructions
                    .skills
                    .iter()
                    .filter(|k| k.scope == scope)
                    .cloned()
                    .collect();
                if skills.is_empty() {
                    continue;
                }
                out.push(quiet(label, 1, cx));
                for k in skills {
                    let f = FileRef {
                        project: self.explorer.project.clone(),
                        // A skill of the project's folder is on its machine.
                        machine: if scope == "repo" {
                            machine.clone()
                        } else {
                            None
                        },
                        path: k.dir.clone(),
                    };
                    // Only a skill of this environment's own goes as a whole here.
                    let own = scope == "project" || (general && scope == "general");
                    out.push(self.entry_row(
                        &f,
                        true,
                        1,
                        own,
                        Some(k.description.clone().into()),
                        cx,
                    ));
                    if self.explorer.is_open(&format!("d:{}", f.listing_key())) {
                        out.extend(self.folder_rows(&f, 2, cx));
                    }
                }
            }
            if e.instructions.skills.is_empty() {
                out.push(quiet("No skills yet", 1, cx));
            }
        }
        out
    }

    /// A node that is not a folder on disk ("Prompts", "MCP servers"): a click folds it.
    fn node_row(
        &self,
        node: &str,
        label: &str,
        icon: Ico,
        note: Option<AnyElement>,
        actions: Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = cx.theme().clone();
        let open = self.explorer.is_open(node);
        let n = node.to_string();
        let mut acts: Vec<AnyElement> = note.into_iter().collect();
        acts.extend(actions);
        bench_ui::tree_row(
            format!("ex-n-{node}"),
            0,
            Some(open),
            (
                if open && matches!(icon, Ico::Folder) {
                    Ico::FolderOpen
                } else {
                    icon
                },
                theme.blue,
            ),
            label.to_string(),
            None,
            false,
            acts,
            cx,
        )
        .on_click(cx.listener(move |this, _, _, cx| this.toggle_node(&n, cx)))
    }

    /// A group's parts: each section of it a node (its heading actions on it), its items under it.
    fn part_rows(
        &self,
        e: &crate::env::ProjectEnv,
        group: &'static str,
        alone: bool,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut out = vec![];
        let general = crate::model::ProjectKey::parse(&e.id).is_host();
        for s in SECTIONS
            .iter()
            .filter(|s| s.home() == Home::Explorer(group) && s.shown(self, e))
        {
            let node = format!("s:{}", s.key());
            let depth = if alone { 0 } else { 1 };
            if !alone {
                let title = s.block(general);
                out.push(
                    self.node_row(
                        &node,
                        title,
                        Ico::Folder,
                        None,
                        s.head(self, e, cx).into_iter().collect(),
                        cx,
                    )
                    .into_any_element(),
                );
                if !self.explorer.is_open(&node) {
                    continue;
                }
            }
            let items = s.items(self, e, cx);
            if items.is_empty() {
                out.push(quiet("None yet", depth, cx));
            }
            let kind = s.key();
            for it in items {
                let key = DocKey::Part {
                    project: e.id.clone(),
                    kind,
                    name: it.name.clone(),
                };
                let chosen = self.explorer.active.as_ref() == Some(&key);
                out.push(
                    bench_ui::tree_row(
                        format!("ex-p-{kind}-{}", it.name),
                        depth,
                        None,
                        it.icon,
                        it.label,
                        Some(it.note.into()),
                        chosen,
                        vec![],
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_doc(key.clone(), window, cx)
                    }))
                    .into_any_element(),
                );
            }
        }
        out
    }

    /// ⌘⇧F in place of the tree: the field, then the lines found by file; a line opens its file
    /// there.
    fn find_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let theme = cx.theme().clone();
        let Some(f) = self.explorer.find.as_ref() else {
            return vec![];
        };
        let close = act(
            "ex-find-close",
            Ico::X,
            "Back to the tree  Esc",
            cx,
            |this, _, cx| {
                this.explorer.find = None;
                cx.notify();
            },
        );
        let mut out = vec![
            h_flex()
                .flex_none()
                .px_2()
                .pb_1()
                .gap_1()
                .items_center()
                .child(div().flex_1().child(Input::new(&f.input).small()))
                .child(close)
                .into_any_element(),
        ];
        let Some(root) = self.explorer_root() else {
            return out;
        };
        match &f.hits {
            None => out.push(quiet("Looking…", 0, cx)),
            Some(h) if h.is_empty() && !f.asked.is_empty() => {
                out.push(quiet("Nothing found", 0, cx))
            }
            Some(hits) => {
                let mut last = String::new();
                for (n, (path, line, text)) in hits.iter().enumerate() {
                    if *path != last {
                        last = path.clone();
                        let (ico, color) = crate::ui::file_icon(path, false, &theme);
                        out.push(
                            h_flex()
                                .flex_none()
                                .px_3()
                                .pt_1()
                                .gap_1p5()
                                .items_center()
                                .text_sm()
                                .child(ico.icon().small().text_color(color))
                                .child(div().min_w_0().truncate().child(path.clone()))
                                .into_any_element(),
                        );
                    }
                    let (file, line) = (root.join(path), *line);
                    out.push(
                        h_flex()
                            .id(("ex-hit", n))
                            .flex_none()
                            .pl(bench_ui::tree_pl(2))
                            .pr_2()
                            .gap_2()
                            .text_xs()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.list_hover))
                            .child(
                                div()
                                    .flex_none()
                                    .text_color(theme.muted_foreground)
                                    .child(line.to_string()),
                            )
                            .child(div().min_w_0().truncate().child(text.trim().to_string()))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_at(file.clone(), line, window, cx)
                            }))
                            .into_any_element(),
                    );
                }
            }
        }
        out
    }
}

/// A small icon action on a row (shown on its hover) or a heading.
fn act(
    id: &str,
    ico: Ico,
    tip: &'static str,
    cx: &mut Context<BenchApp>,
    f: impl Fn(&mut BenchApp, &mut Window, &mut Context<BenchApp>) + 'static,
) -> AnyElement {
    bench_ui::head_icon_button(id, ico, tip, cx)
        .on_click(cx.listener(move |this, _, window, cx| {
            cx.stop_propagation();
            f(this, window, cx)
        }))
        .into_any_element()
}

/// A muted line of the tree, `depth` deep (an empty folder, a group of skills).
fn quiet(text: &str, depth: usize, cx: &App) -> AnyElement {
    div()
        .flex_none()
        .pl(bench_ui::tree_pl(depth + 1))
        .pr_2()
        .py_0p5()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_string())
        .into_any_element()
}
