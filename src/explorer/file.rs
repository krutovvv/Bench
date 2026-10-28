//! A file of the Explorer as a document (`DocKey::File`): read into an editor of its own — on the
//! project's machine, or the agent's own (its soul, prompts, skills are files there) — saved with
//! ⌘S, reverted, drawn with its head (where it is, Save, its changes, Finder).

use gpui_kit::component::input::{Editor, InputEvent};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::item::Item;
use super::view::note;
use super::{Doc, DocKey, FileRef};
use crate::app::BenchApp;
use crate::client::rest;
use bench_ui::icons::Ico;

/// A file's tab.
pub struct FileItem(pub FileRef);

impl Item for FileItem {
    fn tab(&self, _app: &BenchApp, cx: &App) -> (String, (Ico, Hsla)) {
        let name = self.0.name().to_string();
        let icon = bench_ui::file_icon(&name, false, cx.theme());
        (name, icon)
    }

    fn dirty(&self, app: &BenchApp, _cx: &App) -> bool {
        app.explorer
            .docs
            .get(&DocKey::File(self.0.clone()))
            .is_some_and(|d| d.dirty)
    }

    fn open(&self, app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) -> bool {
        app.ensure_doc(self.0.clone(), window, cx);
        true
    }

    fn render(
        &self,
        app: &mut BenchApp,
        _window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> AnyElement {
        app.file_pane(&self.0, cx)
    }

    fn save(&self, app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) {
        app.save_file(self.0.clone(), window, cx);
    }
}

impl BenchApp {
    /// A file's editor, made and read once.
    pub(super) fn ensure_doc(&mut self, f: FileRef, window: &mut Window, cx: &mut Context<Self>) {
        let key = DocKey::File(f.clone());
        if self.explorer.docs.contains_key(&key) {
            return;
        }
        let editor = crate::env::editor("text", window, cx);
        let file = f.clone();
        cx.subscribe_in(
            &editor,
            window,
            move |this, _, ev: &InputEvent, window, cx| {
                let k = DocKey::File(file.clone());
                if let (InputEvent::Change, Some(d)) = (ev, this.explorer.docs.get_mut(&k)) {
                    if !d.loading {
                        d.dirty = true;
                        this.check_doc_soon(&file, window, cx);
                        this.mark_changes(&file, window, cx);
                        cx.notify();
                    }
                }
            },
        )
        .detach();
        self.explorer.docs.insert(key, Doc::new(editor));
        self.attach_servers(&f, cx);
        self.read_doc(f, window, cx);
    }

    /// Read the file into its editor (again: what was typed is dropped).
    pub fn read_doc(&mut self, f: FileRef, window: &mut Window, cx: &mut Context<Self>) {
        if self.demo {
            if let Some(d) = self.explorer.docs.get_mut(&DocKey::File(f.clone())) {
                let text = crate::demo::file_text(&f.path);
                let language = crate::explorer::lang::language_for_text(&f.path, &text);
                d.editor.update(cx, |s, cx| {
                    s.set_highlighter(language, cx);
                    s.set_value(text.clone(), window, cx)
                });
                d.saved = text;
                d.loading = false;
            }
            self.check_doc(&f, Default::default(), window, cx);
            self.read_head(&f, window, cx);
            return;
        }
        let Some((conn, _)) = self.env_target(&f.project) else {
            return;
        };
        let profile = crate::app::model::GENERAL_AGENT.to_string();
        let (machine, path) = (f.machine.clone(), f.path.clone());
        self.background(
            window,
            cx,
            move || rest::read_text(&conn, &profile, machine.as_deref(), &path),
            move |this, r, window, cx| {
                let key = DocKey::File(f.clone());
                let Some(d) = this.explorer.docs.get_mut(&key) else {
                    return;
                };
                match r {
                    Ok(t) => {
                        d.binary = t.binary || t.truncated;
                        d.loading = true;
                        let language = crate::explorer::lang::language_for_text(&f.path, &t.text);
                        d.editor.update(cx, |s, cx| {
                            s.set_highlighter(language, cx);
                            s.set_value(t.text.clone(), window, cx)
                        });
                        d.saved = t.text;
                        d.dirty = false;
                        d.loading = false;
                        if let Some(line) = d.goto.take() {
                            let at = gpui_kit::component::input::Position::new(line, 0);
                            d.editor
                                .update(cx, |s, cx| s.set_cursor_position(at, window, cx));
                        }
                        this.check_doc(&f, Default::default(), window, cx);
                        this.read_head(&f, window, cx);
                    }
                    Err(err) => {
                        d.loading = false;
                        this.toast(NotificationType::Error, format!("{err:#}"), window, cx);
                    }
                }
                cx.notify();
            },
        );
    }

    /// A file written as its editor has it.
    fn save_file(&mut self, f: FileRef, window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = self
            .explorer
            .docs
            .get(&DocKey::File(f.clone()))
            .filter(|d| !d.binary && !d.loading)
        else {
            return;
        };
        let text = d.editor.read(cx).value().to_string();
        let Some((conn, _)) = self.env_target(&f.project) else {
            return;
        };
        let profile = crate::app::model::GENERAL_AGENT.to_string();
        let (machine, path, body) = (f.machine.clone(), f.path.clone(), text.clone());
        self.background(
            window,
            cx,
            move || rest::write_text(&conn, &profile, machine.as_deref(), &path, &body),
            move |this, r, window, cx| match r {
                Ok(()) => {
                    if let Some(d) = this.explorer.docs.get_mut(&DocKey::File(f.clone())) {
                        d.saved = text;
                        d.dirty = false;
                    }
                    this.check_doc(&f, Default::default(), window, cx);
                    this.after_write(&f, window, cx);
                    cx.notify();
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// The text as saved, back in the editor.
    pub fn revert_doc(&mut self, f: FileRef, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(d) = self.explorer.docs.get_mut(&DocKey::File(f)) {
            let text = d.saved.clone();
            d.loading = true;
            d.editor.update(cx, |s, cx| s.set_value(text, window, cx));
            d.loading = false;
            d.dirty = false;
        }
        cx.notify();
    }

    /// A file written here (or made, renamed, removed): what shows it is read again — its folder's
    /// listing, and the environment when it is the agent's text (the soul, a prompt, a skill).
    pub fn after_write(&mut self, f: &FileRef, window: &mut Window, cx: &mut Context<Self>) {
        let env = self.envs.get(&f.project);
        let env_dir = env.map(|e| e.dir.clone());
        let skills: Vec<String> = env
            .map(|e| {
                e.instructions
                    .skills
                    .iter()
                    .map(|k| k.dir.clone())
                    .collect()
            })
            .unwrap_or_default();
        let agents = env_dir.is_some_and(|d| !d.is_empty() && f.within(&d))
            || skills.iter().any(|d| f.within(d));
        if agents && f.machine.is_none() {
            let id = f.project.clone();
            self.load_env(&id, window, cx);
            self.load_prompts(window, cx);
        }
    }

    /// A file: its head (where it is; Revert and Save while changed; Finder) and its editor.
    fn file_pane(&mut self, f: &FileRef, cx: &mut Context<Self>) -> AnyElement {
        let key = DocKey::File(f.clone());
        let Some(d) = self.explorer.docs.get(&key) else {
            return div().into_any_element();
        };
        let (dirty, editor, binary, loading) = (d.dirty, d.editor.clone(), d.binary, d.loading);
        let mut actions: Vec<AnyElement> = vec![];
        if dirty {
            let r = f.clone();
            actions.push(
                bench_ui::form_action("doc-revert", "Revert", false, cx)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.revert_doc(r.clone(), window, cx)
                    }))
                    .into_any_element(),
            );
        }
        let s = key.clone();
        actions.push(
            save_button("doc-save".into(), dirty, cx)
                .on_click(
                    cx.listener(move |this, _, window, cx| this.save_doc(s.clone(), window, cx)),
                )
                .into_any_element(),
        );
        // Its changes since HEAD, when it is in a repository.
        if d.head.is_some() {
            let g = f.clone();
            actions.push(
                bench_ui::head_icon_button("doc-diff", Ico::FileDiff, "Changes since HEAD", cx)
                    .on_click(
                        cx.listener(move |this, _, window, cx| {
                            this.open_diff(g.clone(), window, cx)
                        }),
                    )
                    .into_any_element(),
            );
        }
        // On this Mac, Finder can show it.
        let at = self.location(&f.project);
        let host = f
            .machine
            .clone()
            .or_else(|| (!at.agent.is_empty()).then(|| at.agent.clone()));
        let finder = f.path.clone();
        if host.is_none() {
            actions.push(
                bench_ui::head_icon_button("doc-finder", Ico::FolderOpen, "Show in Finder", cx)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.reveal_in_finder(&finder, window, cx)
                    }))
                    .into_any_element(),
            );
        }
        let where_ = match &f.machine {
            Some(m) => format!("{m}:{}", f.path),
            None => f.path.clone(),
        };
        let head = bench_ui::ide_head(f.name().to_string(), Some(where_.into()), actions, cx);
        let body = if binary {
            note("Not text, or too large to edit here", cx)
        } else if loading {
            note("Reading…", cx)
        } else {
            Editor::new(&editor)
                .appearance(false)
                .h_full()
                .into_any_element()
        };
        v_flex()
            .size_full()
            .child(head)
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }
}

/// A document's Save (⌘S): filled once something changed.
fn save_button(id: SharedString, dirty: bool, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    div()
        .id(id)
        .flex_none()
        .px_2()
        .rounded_md()
        .cursor_pointer()
        .when(dirty, |d| {
            d.bg(theme.primary).text_color(theme.primary_foreground)
        })
        .when(!dirty, |d| d.text_color(theme.muted_foreground))
        .child("Save")
}
