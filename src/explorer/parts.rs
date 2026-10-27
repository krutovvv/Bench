//! The project's parts in the tree that are not its folder: the agent's texts (soul, prompts,
//! skills) and each environment section's items (MCP servers, jobs…), as nodes that fold.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::*;

use super::tree::quiet;
use super::{DocKey, FileRef, Named};
use crate::app::BenchApp;
use crate::env::{Home, SECTIONS};
use bench_ui::icons::Ico;

impl BenchApp {
    /// The soul, the prompts, the skills (each a folder of its own).
    pub(super) fn instruction_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
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
            let general = crate::app::model::ProjectKey::parse(&e.id).is_host();
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
    pub(super) fn part_rows(
        &self,
        e: &crate::env::ProjectEnv,
        group: &'static str,
        alone: bool,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut out = vec![];
        let general = crate::app::model::ProjectKey::parse(&e.id).is_host();
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
}
