//! The skills the project's threads can use, by where each lives. A skill's card is its folder
//! as a tree (`skills_view.rs`), what to do with the tree is in `skills_tree.rs`.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::input::{EditorState, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};

use super::section::{Card, Request, Section, add, empty, new_card, remove, row_with};
use super::{ProjectEnv, editor, enc, is_remote, on_change};
use crate::app::BenchApp;

/// A skill a project's threads can use: its name, its front matter's description, where it lives
/// ("project": its own, "repo": the folder's .agents/skills, "general": the general agent's) and
/// its folder.
#[derive(Clone, Debug, Default)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub scope: String,
    pub dir: String,
}

impl SkillInfo {
    /// The skill's folder holds `rel` (a path inside it, "" its top).
    pub fn path(&self, rel: &str) -> String {
        if rel.is_empty() {
            self.dir.clone()
        } else {
            format!("{}/{rel}", self.dir.trim_end_matches('/'))
        }
    }
}

pub struct SkillsEnv {
    /// Every skill its threads can use, with where each lives.
    pub list: Vec<SkillInfo>,
    /// The skill opened under its row (its folder as a tree), and the file of it being edited.
    pub open: Option<String>,
    pub text: Entity<EditorState>,
    pub dirty: bool,
    /// The open skill's folder: (path inside it, is a folder), folders first at each level.
    pub tree: Vec<(String, bool)>,
    /// Its folders shown open, and the one a new file or folder goes into ("" its top).
    pub expanded: std::collections::HashSet<String>,
    pub folder: String,
    /// The file of it open under the tree (a path inside the skill).
    pub file: Option<String>,
    /// A new file or folder, or a rename, being named: ("file" | "folder" | "rename", the
    /// folder it goes into or the entry renamed), and the name.
    pub form: Option<(&'static str, String)>,
    pub form_name: Entity<InputState>,
}

impl SkillsEnv {
    pub(super) fn new(window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let text = editor("markdown", window, cx);
        let form_name = cx.new(|cx| InputState::new(window, cx).placeholder("Its name"));
        on_change(&text, cx, |e| e.skills.dirty = true);
        Self {
            list: vec![],
            open: None,
            text,
            dirty: false,
            tree: vec![],
            expanded: Default::default(),
            folder: String::new(),
            file: None,
            form: None,
            form_name,
        }
    }
}

pub struct Skills;

impl Section for Skills {
    fn key(&self) -> &'static str {
        "skill"
    }
    fn block(&self, _general: bool) -> &'static str {
        "Skills"
    }
    fn head(&self, _app: &BenchApp, cx: &mut Context<BenchApp>) -> Option<AnyElement> {
        Some(add("env-skill-add", "skill", "", cx).into_any_element())
    }
    /// Every one its threads can use, by where it lives; Finder on a row's hover.
    fn rows(&self, app: &BenchApp, cx: &mut Context<BenchApp>) -> Vec<AnyElement> {
        let Some(e) = app.project_env.as_ref() else {
            return vec![];
        };
        let theme = cx.theme().clone();
        let general = e.id == "general";
        let remote = is_remote(app, &e.id);
        let mut rows = vec![];
        if e.skills.list.is_empty() {
            rows.push(empty(
                "No skills yet: + Add writes one for this project.",
                cx,
            ));
        }
        let groups: [(&str, &str); 3] = [
            ("project", "This project"),
            ("repo", "Its folder · .agents/skills, .claude/skills"),
            ("general", if general { "This Mac" } else { "General" }),
        ];
        for (scope, label) in groups {
            let skills: Vec<_> = e
                .skills
                .list
                .iter()
                .filter(|k| k.scope == scope)
                .cloned()
                .collect();
            if skills.is_empty() {
                continue;
            }
            rows.push(
                div()
                    .px_3()
                    .pt_2()
                    .pb_1()
                    .when(!rows.is_empty(), |d| {
                        d.border_t_1().border_color(theme.border.opacity(0.5))
                    })
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(label)
                    .into_any_element(),
            );
            for k in skills {
                let finder = (!remote).then(|| {
                    let dir = k.dir.clone();
                    crate::inserts::icon_button(
                        &format!("env-skill-finder-{}", k.name),
                        crate::icons::Ico::FolderOpen,
                        "Show in Finder",
                        &theme,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        cx.stop_propagation();
                        this.reveal_in_finder(&dir, window, cx)
                    }))
                    .into_any_element()
                });
                rows.push(row_with(
                    e,
                    "skill",
                    k.name.clone(),
                    None,
                    k.name,
                    k.description,
                    false,
                    false,
                    finder.into_iter().collect(),
                    cx,
                ));
            }
        }
        rows
    }
    /// A skill is a folder: its tree here, a new file's or folder's name and the open file as
    /// the next cards.
    fn card(&self, app: &BenchApp, name: &str, cx: &mut Context<BenchApp>) -> Option<Card> {
        let e = app.project_env.as_ref()?;
        if name == "new" {
            return Some(new_card(
                e,
                "skill",
                "New skill: paste its SKILL.md (its front matter names it)",
                "Write a skill for this project: ",
                &e.skills.text,
                false,
                cx,
            ));
        }
        let theme = cx.theme().clone();
        let k = e.skills.list.iter().find(|k| k.name == name)?.clone();
        let general = e.id == "general";
        let remote = is_remote(app, &e.id);
        let folder = e.skills.folder.clone();
        let mut actions = vec![];
        for (what, ico, tip) in [
            ("file", crate::icons::Ico::FilePlus, "New file"),
            ("folder", crate::icons::Ico::FolderPlus, "New folder"),
        ] {
            let folder = folder.clone();
            actions.push(
                crate::inserts::icon_button(&format!("env-skill-new-{what}"), ico, tip, &theme)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        this.skill_form(what, folder.clone(), window, cx)
                    }))
                    .into_any_element(),
            );
        }
        if !remote {
            let dir = k.dir.clone();
            actions.push(
                crate::inserts::icon_button(
                    "env-skill-finder",
                    crate::icons::Ico::FolderOpen,
                    "Show in Finder",
                    &theme,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.click_claimed = true;
                    this.reveal_in_finder(&dir, window, cx)
                }))
                .into_any_element(),
            );
        }
        // Only a skill of this environment's own goes as a whole; the folder's and the general
        // ones are removed where they live.
        if k.scope == "project" || (general && k.scope == "general") {
            actions.push(remove(e, "skill", name.to_string(), "Remove", cx));
        }
        let (tree, form, file) = app.skill_parts(&k, cx);
        let mut card = Card::new(format!("Skill · {name}"), tree, actions);
        card.follow.extend(form);
        card.follow.extend(file);
        Some(card)
    }
    fn apply(&self, e: &mut ProjectEnv, v: &Value, _window: &mut Window, _cx: &mut App) {
        e.skills.list = v["skills"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|k| {
                let s = |f: &str| k[f].as_str().unwrap_or_default().to_string();
                SkillInfo {
                    name: s("name"),
                    description: s("description"),
                    scope: s("scope"),
                    dir: s("dir"),
                }
            })
            .collect();
    }
    fn close(&self, e: &mut ProjectEnv) {
        e.skills.open = None;
        e.skills.form = None;
    }
    fn open(&self, app: &mut BenchApp, name: String, w: &mut Window, cx: &mut Context<BenchApp>) {
        app.env_open_skill(name, w, cx);
    }
    fn new_one(&self, e: &mut ProjectEnv) -> Option<(Entity<EditorState>, &'static str)> {
        e.skills.open = Some("new".into());
        Some((e.skills.text.clone(), "---\nname: \ndescription: \n---\n\n"))
    }
    /// A new skill is POSTed whole (its front matter names it); an existing one's open file is
    /// written where the skill lives.
    fn save(
        &self,
        app: &mut BenchApp,
        _id: &str,
        eid: &str,
        _window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<Request> {
        let e = app.project_env.as_ref()?;
        let name = e.skills.open.clone()?;
        let text = e.skills.text.read(cx).value().to_string();
        if name == "new" {
            return Some((
                "POST",
                format!("/api/projects/{eid}/skills"),
                json!({"text": text}),
            ));
        }
        let k = e.skills.list.iter().find(|k| k.name == name)?;
        let rel = e.skills.file.as_ref()?;
        Some((
            "POST",
            "/api/fs/write-text".to_string(),
            json!({"path": k.path(rel), "content": text}),
        ))
    }
    fn saved(&self, e: &mut ProjectEnv) {
        e.skills.dirty = false;
    }
    fn revert(&self, app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) -> bool {
        let Some(e) = app.project_env.as_mut() else {
            return true;
        };
        e.skills.dirty = false;
        match e.skills.file.clone() {
            Some(rel) => {
                app.skill_open_file(rel, window, cx);
                true
            }
            None => false,
        }
    }
    fn remove_path(&self, eid: &str, name: &str) -> Option<String> {
        Some(format!("/api/projects/{eid}/skills/{}", enc(name)))
    }
}

impl BenchApp {
    /// A skill's row: its folder opens under it as a tree, with its SKILL.md open to read and
    /// edit (again: closes).
    pub fn env_open_skill(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        let s = &mut e.skills;
        if s.open.as_deref() == Some(&name) && !s.dirty {
            s.open = None;
            cx.notify();
            return;
        }
        s.open = Some(name.clone());
        s.dirty = false;
        s.tree.clear();
        s.expanded.clear();
        s.folder.clear();
        s.form = None;
        s.file = Some("SKILL.md".into());
        if self.demo {
            let text = format!(
                "---\nname: {name}\ndescription: A made-up skill of the demo.\n---\n\nWhat to do, step by step."
            );
            s.tree = vec![
                ("references".into(), true),
                ("references/style.md".into(), false),
                ("scripts".into(), true),
                ("SKILL.md".into(), false),
            ];
            s.expanded.insert("references".into());
            s.text.update(cx, |s, cx| s.set_value(text, window, cx));
            s.dirty = false;
            cx.notify();
            return;
        }
        self.skill_load_tree(window, cx);
        self.skill_open_file("SKILL.md".into(), window, cx);
    }

    /// The open skill, as the environment lists it.
    pub(super) fn open_skill(&self) -> Option<SkillInfo> {
        let e = self.project_env.as_ref()?;
        let name = e.skills.open.as_ref()?;
        e.skills.list.iter().find(|k| &k.name == name).cloned()
    }
}
