//! The editor's language features from the project's language servers, through its agent
//! (`client/rest/lsp.rs`, `agent/src/modules/lsp.ts`): problems underlined as the file is typed,
//! the hover over a symbol, ⌘-click or F12 to where it is defined — in this file, or in another
//! one opened as a tab. The servers are the ones the project's threads use (its `lsp.json`).

use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::input::{DefinitionProvider, HoverProvider, Rope, RopeExt as _};
use gpui_kit::*;
use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, utf8_percent_encode};

use super::{DocKey, FileRef};
use crate::app::BenchApp;
use crate::client::connection::Connection;
use crate::client::rest::lsp::{self, Asked};

/// How long typing pauses before the file is checked again.
const CHECK_AFTER: Duration = Duration::from_millis(600);

/// What a path keeps of itself in a `file://` URI.
const PATH: &AsciiSet = &CONTROLS.add(b' ').add(b'#').add(b'?').add(b'%');

/// One open file's way to its project's servers.
#[derive(Clone)]
struct Servers {
    conn: Arc<Connection>,
    profile: String,
    /// The project's id at its agent.
    project: String,
    file: FileRef,
}

impl Servers {
    fn asked<'a>(&'a self, text: &'a str) -> Asked<'a> {
        Asked {
            project: &self.project,
            machine: self.file.machine.as_deref(),
            path: &self.file.path,
            text,
        }
    }
}

impl HoverProvider for Servers {
    fn hover(
        &self,
        text: &Rope,
        offset: usize,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Option<lsp_types::Hover>>> {
        let (s, at, text) = (
            self.clone(),
            text.offset_to_position(offset),
            text.to_string(),
        );
        cx.background_spawn(async move {
            let md = lsp::hover(&s.conn, &s.profile, &s.asked(&text), at)?;
            Ok((!md.is_empty()).then(|| lsp_types::Hover {
                contents: lsp_types::HoverContents::Markup(lsp_types::MarkupContent {
                    kind: lsp_types::MarkupKind::Markdown,
                    value: md,
                }),
                range: None,
            }))
        })
    }
}

impl DefinitionProvider for Servers {
    fn definitions(
        &self,
        text: &Rope,
        offset: usize,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Vec<lsp_types::LocationLink>>> {
        let (s, at, text) = (
            self.clone(),
            text.offset_to_position(offset),
            text.to_string(),
        );
        cx.background_spawn(async move {
            let places = lsp::definition(&s.conn, &s.profile, &s.asked(&text), at)?;
            Ok(places
                .into_iter()
                .filter_map(|(path, range)| {
                    let uri = format!("file://{}", utf8_percent_encode(&path, PATH));
                    Some(lsp_types::LocationLink {
                        origin_selection_range: None,
                        target_uri: uri.parse().ok()?,
                        target_range: range,
                        target_selection_range: range,
                    })
                })
                .collect())
        })
    }
}

/// The path a `file://` URI names.
fn path_of(uri: &lsp_types::Uri) -> String {
    let s = uri.as_str();
    let p = s.strip_prefix("file://").unwrap_or(s);
    percent_decode_str(p).decode_utf8_lossy().into_owned()
}

impl BenchApp {
    /// Give a file's editor its project's servers: the hover, the definition (another file opens
    /// as a tab of the same project and machine). None in the demo: it has no agent.
    pub(super) fn attach_servers(&mut self, f: &FileRef, cx: &mut Context<Self>) {
        let Some((conn, project)) = self.env_target(&f.project).filter(|_| !self.demo) else {
            return;
        };
        let Some(d) = self.explorer.docs.get(&DocKey::File(f.clone())) else {
            return;
        };
        let servers = Rc::new(Servers {
            conn,
            profile: crate::app::model::GENERAL_AGENT.to_string(),
            project,
            file: f.clone(),
        });
        let app = cx.entity().downgrade();
        let here = f.clone();
        d.editor.update(cx, |s, cx| {
            let lsp = s.lsp_mut();
            lsp.hover_provider = Some(servers.clone());
            lsp.definition_provider = Some(servers);
            lsp.show_document = Some(Rc::new(move |params, window, cx| {
                let path = path_of(&params.uri);
                // In this file the editor moves the caret itself.
                if path == here.path {
                    return false;
                }
                let line = params.selection.map_or(0, |r| r.start.line as usize);
                let f = FileRef {
                    path,
                    ..here.clone()
                };
                app.update(cx, |this, cx| this.open_at(f, line + 1, window, cx))
                    .is_ok()
            }));
            s.refresh(cx);
        });
    }

    /// Ask the project's servers about the file as it is now (after `after`: typing pauses), and
    /// underline what they find. A newer check replaces this one.
    pub(super) fn check_doc(
        &mut self,
        f: &FileRef,
        after: Duration,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = DocKey::File(f.clone());
        let Some(editor) = self
            .explorer
            .docs
            .get(&key)
            .filter(|d| !d.binary)
            .map(|d| d.editor.clone())
        else {
            return;
        };
        if self.demo {
            let problems = crate::demo::file_problems(&f.path);
            editor.update(cx, |s, cx| underline(s, problems, cx));
            return;
        }
        let Some((conn, project)) = self.env_target(&f.project) else {
            return;
        };
        let profile = crate::app::model::GENERAL_AGENT.to_string();
        let (machine, path) = (f.machine.clone(), f.path.clone());
        let project_key = f.project.clone();
        let task = cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(after).await;
            let text = editor.update(cx, |s, _| s.value().to_string());
            let sent = text.clone();
            let found = cx
                .background_spawn(async move {
                    let a = Asked {
                        project: &project,
                        machine: machine.as_deref(),
                        path: &path,
                        text: &sent,
                    };
                    lsp::diagnostics(&conn, &profile, &a)
                })
                .await;
            // No server for this kind of file, or the agent did not answer: nothing underlined.
            let problems = found.ok().flatten().unwrap_or_default();
            editor.update(cx, |s, cx| {
                // Typed since it was sent: the next check underlines the new text.
                if s.value().as_ref() == text {
                    underline(s, problems, cx);
                }
            });
            let _ = this.update_in(cx, |this, window, cx| {
                this.problems_changed(&project_key, window, cx)
            });
        });
        if let Some(d) = self.explorer.docs.get_mut(&key) {
            d.checking = task;
        }
    }

    /// Typing in a file: its check waits for a pause.
    pub(super) fn check_doc_soon(
        &mut self,
        f: &FileRef,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.check_doc(f, CHECK_AFTER, window, cx);
    }
}

/// The editor's underlines become `problems`.
fn underline(
    s: &mut gpui_kit::component::input::EditorState,
    problems: Vec<lsp_types::Diagnostic>,
    cx: &mut Context<gpui_kit::component::input::EditorState>,
) {
    let text = s.text().clone();
    if let Some(set) = s.diagnostics_mut() {
        set.reset(&text);
        set.extend(problems);
    }
    cx.notify();
}
