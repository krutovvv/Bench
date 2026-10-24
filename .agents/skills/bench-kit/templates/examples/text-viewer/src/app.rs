//! The app's whole state and its actions. Views (`ui/`) read it and call these methods; I/O
//! goes in its own modules. Here: a text viewer — Recent files on the left, the open file in an
//! editor (line numbers, highlighting, search), Ln/Col in the status bar.

use std::path::PathBuf;

use gpui_kit::component::input::{EditorState, InputEvent, InputState};
use gpui_kit::*;

use crate::state::state_dir;
use crate::theme::Appearance;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Main,
    Settings,
}

/// The files opened lately, newest first: the sidebar's list.
pub struct MainApp {
    pub appearance: Appearance,
    pub panel: Panel,
    pub recent: Vec<PathBuf>,
    pub search: Entity<InputState>,
    pub query: String,
    pub editor: Entity<EditorState>,
    pub file: Option<PathBuf>,
    _subscriptions: Vec<Subscription>,
}

fn recent_path() -> PathBuf {
    state_dir().join("recent.json")
}

impl MainApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search files"));
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .line_number(true)
                .soft_wrap(false)
                .searchable(true)
        });
        let subscriptions = vec![
            // "System" follows macOS light/dark live.
            cx.observe_window_appearance(window, |this, window, cx| {
                if this.appearance.mode == crate::theme::Mode::System {
                    crate::theme::apply(this.appearance, Some(window), cx);
                }
            }),
            cx.subscribe_in(&search, window, |this, state, ev: &InputEvent, _, cx| {
                if let InputEvent::Change = ev {
                    this.query = state.read(cx).value().trim().to_lowercase();
                    cx.notify();
                }
            }),
            // The cursor moved or the text changed: the status bar's Ln/Col follows.
            cx.subscribe_in(&editor, window, |_, _, _: &InputEvent, _, cx| cx.notify()),
        ];
        let recent: Vec<PathBuf> = std::fs::read(recent_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let mut this = Self {
            appearance: Appearance::load(),
            panel: Panel::Main,
            recent,
            search,
            query: String::new(),
            editor,
            file: None,
            _subscriptions: subscriptions,
        };
        // `text-view path/to/file` opens it (also how to check the app with a screenshot).
        if let Some(arg) = std::env::args().nth(1) {
            this.load_file(arg.into(), window, cx);
        }
        this
    }

    fn save_recent(&self) {
        let _ = std::fs::create_dir_all(state_dir());
        let _ = std::fs::write(
            recent_path(),
            serde_json::to_vec_pretty(&self.recent).unwrap_or_default(),
        );
    }

    /// Recent files matching the search, by index.
    pub fn visible(&self) -> Vec<usize> {
        (0..self.recent.len())
            .filter(|i| {
                self.query.is_empty()
                    || self.recent[*i]
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&self.query)
            })
            .collect()
    }

    /// Put a file's text in the editor with the right highlighting; it goes to the top of Recent.
    pub fn load_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        const MAX: usize = 2 * 1024 * 1024;
        let read = std::fs::read(&path);
        let ok = read.is_ok();
        let mut text = match read {
            Ok(bytes) => String::from_utf8_lossy(&bytes[..bytes.len().min(MAX)]).into_owned(),
            Err(e) => format!("Cannot read {}: {e}", path.display()),
        };
        if std::fs::metadata(&path).map_or(0, |m| m.len() as usize) > MAX {
            text.push_str("\n\n[only the first 2 MB is shown]");
        }
        let language = crate::lang::language_for_text(&path.to_string_lossy(), &text);
        self.editor.update(cx, |s, cx| {
            s.set_highlighter(language, cx);
            s.set_value(text, window, cx);
        });
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        // Only files that opened go to Recent (a typo'd path shows its error, nothing more).
        if ok {
            self.recent.retain(|p| *p != path);
            self.recent.insert(0, path.clone());
            self.recent.truncate(30);
            self.save_recent();
        }
        self.file = Some(path);
        self.panel = Panel::Main;
        cx.notify();
    }

    pub fn open_file_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update_in(cx, |this, window, cx| this.load_file(path, window, cx));
        })
        .detach();
    }

    /// Off the list (the file itself stays where it is).
    pub fn forget(&mut self, i: usize, cx: &mut Context<Self>) {
        if i < self.recent.len() {
            let gone = self.recent.remove(i);
            if self.file.as_ref() == Some(&gone) {
                self.file = None;
            }
            self.save_recent();
            cx.notify();
        }
    }

    pub fn toggle_settings(&mut self, cx: &mut Context<Self>) {
        self.panel = if self.panel == Panel::Settings {
            Panel::Main
        } else {
            Panel::Settings
        };
        cx.notify();
    }

    pub fn set_appearance(&mut self, a: Appearance, window: &mut Window, cx: &mut Context<Self>) {
        self.appearance = a;
        a.save();
        crate::theme::apply(a, Some(window), cx);
        cx.notify();
    }
}
