//! Instructions: what the agent reads as text, in a small editor — on the left a tree of the
//! soul (SYSTEM.md), the project's prompts and the skills it can use (each a folder: its files
//! shown when it is open), on the right the chosen text in the code editor with Save and Revert.
//! What is open is the page's `card` ("soul", "prompt", "skill"); the texts and what is done to
//! a skill's folder are the sections' own (`soul.rs`, `prompts.rs`, `skills.rs`,
//! `skills_tree.rs`).

use gpui_kit::component::input::{Editor, EditorState, Input};
use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::is_remote;
use super::section::{head_act, remove, revert, save};
use crate::app::BenchApp;
use crate::icons::Ico;

impl BenchApp {
    /// The tree beside the editor, both filling the room the page gives them.
    pub(super) fn render_env_ide(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tree = self.env_tree(cx);
        let (head, body) = self.env_editor(cx);
        bench_ui::ide(bench_ui::IDE_TREE_W, tree, head, body, cx).into_any_element()
    }

    /// Soul, Prompts, Skills (by where each lives); the open skill's files under it.
    fn env_tree(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let Some(e) = self.project_env.as_ref() else {
            return div().into_any_element();
        };
        let open = e.card.clone();
        let is = |k: &str, n: &str| open.as_ref().is_some_and(|(ok, on)| *ok == k && on == n);
        let text_ico = (Ico::FileText, theme.muted_foreground);
        let mut rows: Vec<AnyElement> = vec![];

        rows.push(bench_ui::tree_heading("Soul", vec![], cx).into_any_element());
        rows.push(
            bench_ui::tree_row(
                "env-t-soul",
                0,
                None,
                text_ico,
                "SYSTEM.md",
                is("soul", ""),
                vec![],
                cx,
            )
            .on_click(
                cx.listener(|this, _, window, cx| this.env_open("soul", String::new(), window, cx)),
            )
            .into_any_element(),
        );

        let add_prompt =
            head_act("env-t-prompt-add".into(), "+ New", cx)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.env_open("prompt", "new".into(), window, cx)
                }))
                .into_any_element();
        rows.push(bench_ui::tree_heading("Prompts", vec![add_prompt], cx).into_any_element());
        if e.prompts.names.is_empty() && !is("prompt", "new") {
            rows.push(quiet("None of its own: @ offers the general ones", cx));
        }
        if is("prompt", "new") {
            rows.push(
                bench_ui::tree_row(
                    "env-t-prompt-new",
                    0,
                    None,
                    text_ico,
                    "New prompt",
                    true,
                    vec![],
                    cx,
                )
                .into_any_element(),
            );
        }
        for (k, name) in e.prompts.names.iter().enumerate() {
            let pick = name.clone();
            rows.push(
                bench_ui::tree_row(
                    format!("env-t-prompt-{k}"),
                    0,
                    None,
                    text_ico,
                    name.clone(),
                    is("prompt", name),
                    vec![],
                    cx,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.env_open("prompt", pick.clone(), window, cx)
                }))
                .into_any_element(),
            );
        }

        let add_skill = head_act("env-t-skill-add".into(), "+ New", cx)
            .on_click(
                cx.listener(|this, _, window, cx| this.env_open("skill", "new".into(), window, cx)),
            )
            .into_any_element();
        rows.push(bench_ui::tree_heading("Skills", vec![add_skill], cx).into_any_element());
        if e.skills.list.is_empty() && !is("skill", "new") {
            rows.push(quiet("No skills yet", cx));
        }
        if is("skill", "new") {
            rows.push(
                bench_ui::tree_row(
                    "env-t-skill-new",
                    0,
                    None,
                    text_ico,
                    "New skill",
                    true,
                    vec![],
                    cx,
                )
                .into_any_element(),
            );
        }
        let remote = is_remote(self, &e.id);
        let groups: [(&str, &str); 3] = [
            ("project", "This project"),
            ("repo", "Its folder"),
            ("general", if remote { "The server's" } else { "This Mac" }),
        ];
        let open_skill = e.skills.open.clone().filter(|n| n != "new");
        for (scope, label) in groups {
            let skills: Vec<_> = e.skills.list.iter().filter(|k| k.scope == scope).collect();
            if skills.is_empty() {
                continue;
            }
            rows.push(quiet(label, cx));
            for (n, k) in skills.into_iter().enumerate() {
                let here = open_skill.as_deref() == Some(k.name.as_str());
                let pick = k.name.clone();
                rows.push(
                    bench_ui::tree_row(
                        format!("env-t-skill-{scope}-{n}"),
                        0,
                        Some(here),
                        (if here { Ico::FolderOpen } else { Ico::Folder }, theme.blue),
                        k.name.clone(),
                        false,
                        vec![],
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.env_open("skill", pick.clone(), window, cx)
                    }))
                    .into_any_element(),
                );
                if here {
                    rows.extend(self.env_skill_files(cx));
                }
            }
        }
        v_flex().w_full().children(rows).into_any_element()
    }

    /// The open skill's files, under its row: folders open in place, a file opens in the editor;
    /// rename and delete on hover.
    fn env_skill_files(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let theme = cx.theme().clone();
        let Some(e) = self.project_env.as_ref() else {
            return vec![];
        };
        let s = &e.skills;
        let shown = |rel: &str| {
            let mut up = rel;
            while let Some((p, _)) = up.rsplit_once('/') {
                if !s.expanded.contains(p) {
                    return false;
                }
                up = p;
            }
            true
        };
        if s.tree.is_empty() {
            return vec![quiet("Reading the folder…", cx)];
        }
        let mut rows = vec![];
        for (n, (rel, dir)) in s.tree.iter().filter(|(r, _)| shown(r)).enumerate() {
            let (rel, dir) = (rel.clone(), *dir);
            let depth = rel.matches('/').count() + 1;
            let name = rel.rsplit('/').next().unwrap_or_default().to_string();
            let chosen = !dir && s.file.as_deref() == Some(rel.as_str());
            let armed = e.armed.as_deref() == Some(format!("skillfile:{rel}").as_str());
            let (r1, r2) = (rel.clone(), rel.clone());
            let rename =
                crate::inserts::icon_button(&format!("env-t-mv{n}"), Ico::Pencil, "Rename", &theme)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.skill_form("rename", r1.clone(), window, cx)
                    }))
                    .into_any_element();
            let trash = div()
                .id(SharedString::from(format!("env-t-rm{n}")))
                .flex_none()
                .px_1()
                .cursor_pointer()
                .text_sm()
                .when(!armed, |d| {
                    d.text_color(theme.muted_foreground)
                        .hover(|s| s.text_color(theme.danger))
                        .child(Ico::Trash.icon())
                })
                .when(armed, |d| d.text_color(theme.danger).child("Delete?"))
                .on_click(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.skill_delete(r2.clone(), window, cx)
                }))
                .into_any_element();
            let ico = if dir && s.expanded.contains(&rel) {
                (Ico::FolderOpen, theme.blue)
            } else {
                crate::ui::file_icon(&name, dir, &theme)
            };
            rows.push(
                bench_ui::tree_row(
                    format!("env-t-f{n}"),
                    depth,
                    dir.then(|| s.expanded.contains(&rel)),
                    ico,
                    name,
                    chosen,
                    vec![rename, trash],
                    cx,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.skill_entry(rel.clone(), dir, window, cx)
                }))
                .into_any_element(),
            );
        }
        rows
    }

    /// The open text's head (its name, where it is, its actions) and the editor.
    fn env_editor(&self, cx: &mut Context<Self>) -> (AnyElement, AnyElement) {
        let theme = cx.theme().clone();
        let Some(e) = self.project_env.as_ref() else {
            return (div().into_any_element(), div().into_any_element());
        };
        let editor = |state: &Entity<EditorState>| {
            Editor::new(state)
                .appearance(false)
                .h_full()
                .into_any_element()
        };
        let head = |title: String, note: Option<String>, actions: Vec<AnyElement>, cx: &App| {
            bench_ui::ide_head(title, note.map(Into::into), actions, cx).into_any_element()
        };
        let general = e.id == "general";
        match e.card.clone() {
            Some(("soul", _)) => {
                let mut actions: Vec<AnyElement> =
                    revert("soul", e.soul.dirty, cx).into_iter().collect();
                actions.push(save("soul", e.soul.dirty, cx));
                (
                    head(
                        "SYSTEM.md".into(),
                        Some(
                            "what the project is and how to work in it: every thread reads it"
                                .into(),
                        ),
                        actions,
                        cx,
                    ),
                    editor(&e.soul.text),
                )
            }
            Some(("prompt", name)) if name == "new" => (
                head(
                    "New prompt".into(),
                    Some("@ in the composer offers it".into()),
                    vec![save("prompt", true, cx)],
                    cx,
                ),
                v_flex()
                    .size_full()
                    .child(
                        div()
                            .flex_none()
                            .px_3()
                            .pt_2()
                            .pb(px(crate::ui::RING_CLEAR))
                            .child(Input::new(&e.new_name)),
                    )
                    .child(div().flex_1().min_h_0().child(editor(&e.prompts.text)))
                    .into_any_element(),
            ),
            Some(("prompt", name)) => {
                let mut actions: Vec<AnyElement> =
                    revert("prompt", e.prompts.dirty, cx).into_iter().collect();
                actions.push(remove(e, "prompt", name.clone(), "Remove", cx));
                actions.push(save("prompt", e.prompts.dirty, cx));
                (
                    head(name, Some("@ in the composer".into()), actions, cx),
                    editor(&e.prompts.text),
                )
            }
            Some(("skill", name)) if name == "new" => (
                head(
                    "New skill".into(),
                    Some("its SKILL.md: the front matter names it".into()),
                    vec![save("skill", true, cx)],
                    cx,
                ),
                editor(&e.skills.text),
            ),
            Some(("skill", name)) => {
                let Some(k) = e.skills.list.iter().find(|k| k.name == name).cloned() else {
                    return (div().into_any_element(), self.env_nothing(cx));
                };
                let remote = is_remote(self, &e.id);
                let folder = e.skills.folder.clone();
                let mut actions: Vec<AnyElement> = vec![];
                for (what, ico, tip) in [
                    ("file", Ico::FilePlus, "New file"),
                    ("folder", Ico::FolderPlus, "New folder"),
                ] {
                    let folder = folder.clone();
                    actions.push(
                        crate::inserts::icon_button(&format!("env-t-new-{what}"), ico, tip, &theme)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.skill_form(what, folder.clone(), window, cx)
                            }))
                            .into_any_element(),
                    );
                }
                if !remote {
                    let dir = k.dir.clone();
                    actions.push(
                        crate::inserts::icon_button(
                            "env-t-finder",
                            Ico::FolderOpen,
                            "Show in Finder",
                            &theme,
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.reveal_in_finder(&dir, window, cx)
                        }))
                        .into_any_element(),
                    );
                }
                // Only a skill of this environment's own goes as a whole; the folder's and the
                // general ones are removed where they live.
                if k.scope == "project" || (general && k.scope == "general") {
                    actions.push(remove(e, "skill", name.clone(), "Remove skill", cx));
                }
                let file = e.skills.file.clone();
                if file.is_some() {
                    actions.extend(revert("skill", e.skills.dirty, cx));
                    actions.push(save("skill", e.skills.dirty, cx));
                }
                let title = match &file {
                    Some(rel) => format!("{name}/{rel}"),
                    None => name.clone(),
                };
                let body = v_flex()
                    .size_full()
                    .children(self.env_skill_form(&k.name, cx))
                    .child(div().flex_1().min_h_0().child(match file {
                        Some(_) => editor(&e.skills.text),
                        None => self.env_nothing(cx),
                    }))
                    .into_any_element();
                let note = if k.description.is_empty() {
                    k.dir.clone()
                } else {
                    k.description.clone()
                };
                (head(title, Some(note), actions, cx), body)
            }
            _ => (head(String::new(), None, vec![], cx), self.env_nothing(cx)),
        }
    }

    /// The field naming a new file or folder of the open skill, or a rename, over the editor.
    fn env_skill_form(&self, skill: &str, cx: &mut Context<Self>) -> Option<AnyElement> {
        let e = self.project_env.as_ref()?;
        let (what, target) = e.skills.form.clone()?;
        let title = match what {
            "file" => "New file",
            "folder" => "New folder",
            _ => "Rename",
        };
        let place = if what == "rename" {
            target.clone()
        } else if target.is_empty() {
            format!("In {skill}")
        } else {
            format!("In {skill}/{target}")
        };
        let actions = vec![
            crate::ui::form_action(
                "env-t-form-ok",
                if what == "rename" { "Rename" } else { "Create" },
                true,
                cx,
            )
            .on_click(cx.listener(|this, _, window, cx| this.skill_form_done(window, cx)))
            .into_any_element(),
            crate::ui::form_action("env-t-form-cancel", "Cancel", false, cx)
                .on_click(cx.listener(|this, _, _, cx| {
                    if let Some(e) = this.project_env.as_mut() {
                        e.skills.form = None;
                    }
                    cx.notify();
                }))
                .into_any_element(),
        ];
        Some(
            div()
                .flex_none()
                .p_3()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(crate::ui::inline_form(
                    title,
                    vec![("Name", Input::new(&e.skills.form_name).into_any_element())],
                    Some(place.into()),
                    actions,
                    cx,
                ))
                .into_any_element(),
        )
    }

    /// Nothing open in the editor: one line.
    fn env_nothing(&self, cx: &App) -> AnyElement {
        div()
            .p_3()
            .text_color(cx.theme().muted_foreground)
            .child("Choose a text on the left")
            .into_any_element()
    }
}

/// A quiet line of the tree (an empty group, where a group of skills lives).
fn quiet(text: &str, cx: &App) -> AnyElement {
    div()
        .px_3()
        .pt_1()
        .pb_0p5()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_string())
        .into_any_element()
}
