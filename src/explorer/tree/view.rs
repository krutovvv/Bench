//! The sidebar in the Explorer view: the project chosen at the top, then its tree — the folder
//! (on its machine, listed as folders open), Instructions (the soul, prompts, skills: the agent's
//! own files), and the environment's parts that are not files, by their `Section`'s group
//! (Tools: MCP and language servers; Schedule). ⌘⇧F's results take the tree's place while open.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::explorer::{DocKey, FileRef, Named};
use crate::app::BenchApp;
use crate::env::PARTS;
use bench_ui::icons::Ico;

impl BenchApp {
    /// The left dock's Explorer panel (`Panel::Explorer`): the project's tree, scrolling.
    pub(crate) fn render_explorer_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rows = self.render_explorer(window, cx);
        v_flex()
            .id("explorer")
            .size_full()
            .overflow_y_scroll()
            .pb_1()
            .children(rows)
            .into_any_element()
    }

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
            let menu = crate::explorer::tree::menus::root_menu(cx.entity(), root.clone());
            out.push(
                self.heading_el("files", &name, vec![], cx)
                    .context_menu(menu)
                    .into_any_element(),
            );
            if self.explorer.is_open("files") {
                out.extend(self.folder_rows(&root, 0, cx));
            }
        }
        // The agent's: its texts (the soul, prompts, skills), then its tools — the parts whose
        // group is "Agent". (The fold keeps its old key, "instructions".)
        out.push(self.heading("instructions", "Agent", vec![], cx));
        let open = self.explorer.is_open("instructions");
        if open {
            out.extend(self.instruction_rows(cx));
        }
        // The environment's parts, by the group their sections name.
        let Some(e) = self.envs.get(&self.explorer.project) else {
            return out;
        };
        if open {
            out.extend(self.part_rows(e, "Agent", false, cx));
        }
        let mut groups: Vec<&'static str> = vec![];
        for s in PARTS.iter().filter(|s| s.shown(self, e)) {
            if s.group() != "Agent" && !groups.contains(&s.group()) {
                groups.push(s.group());
            }
        }
        let general = crate::app::model::ProjectKey::parse(&e.id).is_host();
        for g in groups {
            let node = format!("g:{g}");
            // A group of one section named as it (Schedule): its actions on the group's heading,
            // its parts right under it.
            let only: Vec<_> = PARTS
                .iter()
                .filter(|s| s.group() == g && s.shown(self, e))
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
        // Last, what the project is set to: its settings, a document like the rest — in the
        // Project group, after what a part put there (its schedule); a heading of its own when
        // no part did.
        let settings = DocKey::Settings {
            project: self.explorer.project.clone(),
        };
        let theme = cx.theme().clone();
        let project_group = PARTS
            .iter()
            .any(|s| s.group() == "Project" && s.shown(self, e));
        if !project_group {
            out.push(bench_ui::tree_heading("Project", vec![], cx).into_any_element());
        } else if !self.explorer.is_open("g:Project") {
            return out;
        }
        out.push(
            bench_ui::tree_row(
                "ex-settings",
                0,
                None,
                (Ico::Settings, theme.muted_foreground),
                "Settings",
                Some("name, folder, model, Telegram".into()),
                false,
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
            .places.projects
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| crate::app::LOCAL_HOST.into());
        let choices: Vec<(String, String)> = std::iter::once((
            crate::app::model::ProjectKey::HOST.to_string(),
            crate::app::LOCAL_HOST.to_string(),
        ))
        .chain(self.places.projects.iter().map(|p| (p.id.clone(), p.name.clone())))
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
    pub(crate) fn folder_rows(
        &self,
        dir: &FileRef,
        depth: usize,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut out = vec![];
        out.extend(self.naming_row(&crate::explorer::Named::File(dir.clone()), depth));
        out.extend(self.naming_row(&crate::explorer::Named::Folder(dir.clone()), depth));
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
    pub(crate) fn entry_row(
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
            bench_ui::file_icon(f.name(), dir, &theme)
        };
        // Its work is in its menu; only a delete asked for shows here, to be confirmed.
        let armed =
            self.armed.is(&format!("del:{}", f.listing_key()));
        let acts = if armed {
            vec![self.trash(f, cx)]
        } else {
            vec![]
        };
        let target = f.clone();
        let dim = self.explorer.is_ignored(&f.path);
        bench_ui::tree_row(
            format!("ex-e-{key}"),
            depth,
            dir.then_some(open),
            icon,
            f.name().to_string(),
            note,
            dim,
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
        .context_menu(crate::explorer::tree::menus::entry_menu(
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
            self.armed.is(&format!("del:{}", f.listing_key()));
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
    pub(crate) fn naming_row(&self, what: &Named, depth: usize) -> Option<AnyElement> {
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
}

/// A small icon action on a row (shown on its hover) or a heading.
pub(crate) fn act(
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
pub(crate) fn quiet(text: &str, depth: usize, cx: &App) -> AnyElement {
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
