//! The project's Problems: every error and warning its language servers know now (the files the
//! editor or the agent showed them), as a tab of the middle, as Zed's project diagnostics are.
//! A problem opens its file at its line. Problems go to the thread as a reference — all of them
//! from here, a file's own from its tab's menu — the same way ⌘L sends code.

use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::*;

use crate::explorer::pane::item::Item;
use crate::explorer::{DocKey, FileRef};
use crate::app::BenchApp;
use crate::client::rest::lsp;
use crate::thread::composer::attachments::{ProblemsRef, Reference};
use bench_ui::icons::Ico;

/// A problem as one line, for the agent: `path:line:col error: message (source)`.
fn line_of(path: &str, d: &lsp_types::Diagnostic) -> String {
    let what = if d.severity == Some(lsp_types::DiagnosticSeverity::WARNING) {
        "warning"
    } else {
        "error"
    };
    let source = d
        .source
        .as_ref()
        .map_or(String::new(), |s| format!(" ({s})"));
    format!(
        "{path}:{}:{} {what}: {}{source}",
        d.range.start.line + 1,
        d.range.start.character + 1,
        d.message.lines().next().unwrap_or_default(),
    )
}

/// Errors and warnings only: hints and notes are an editor's, not a list's.
fn counted(d: &lsp_types::Diagnostic) -> bool {
    d.severity.is_none_or(|s| {
        s == lsp_types::DiagnosticSeverity::ERROR || s == lsp_types::DiagnosticSeverity::WARNING
    })
}

/// The project's Problems tab.
pub struct ProblemsItem(pub String);

impl Item for ProblemsItem {
    fn tab(&self, _app: &BenchApp, cx: &App) -> (String, (Ico, Hsla)) {
        (
            "Problems".into(),
            (Ico::CircleAlert, cx.theme().muted_foreground),
        )
    }

    fn open(&self, app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) -> bool {
        app.read_problems(self.0.clone(), window, cx);
        true
    }

    fn render(
        &self,
        app: &mut BenchApp,
        _window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> AnyElement {
        app.problems_pane(&self.0, cx)
    }
}

impl BenchApp {
    /// ⌘⇧M, the folder's menu: the Explorer's project's Problems in the middle.
    pub fn open_problems(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let project = self.explorer.project.clone();
        if project.is_empty() {
            return;
        }
        self.open_doc(DocKey::Problems { project }, window, cx);
    }

    /// What the project's servers know now, read into its tab.
    pub(crate) fn read_problems(
        &mut self,
        project: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.demo {
            let post = "/Users/demo/code/blog/content/posts/release-2-4.md";
            let found = vec![(post.to_string(), crate::demo::file_problems(post))];
            self.explorer.problems.insert(project, Some(found));
            return;
        }
        let Some((conn, id)) = self.env_target(&project) else {
            return;
        };
        self.explorer
            .problems
            .entry(project.clone())
            .or_insert(None);
        self.background(
            window,
            cx,
            move || lsp::problems(&conn, &id),
            move |this, r, window, cx| {
                match r {
                    Ok(found) => {
                        this.explorer.problems.insert(project, Some(found));
                    }
                    Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
                }
                cx.notify();
            },
        );
    }

    /// A file was checked: its project's Problems, when open, are read again.
    pub(crate) fn problems_changed(
        &mut self,
        project: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = DocKey::Problems {
            project: project.to_string(),
        };
        if self.explorer.tabs.contains(&key) {
            self.read_problems(project.to_string(), window, cx);
        }
    }

    /// The project's problems to the thread's composer, the thread beside.
    pub fn send_problems_to_thread(
        &mut self,
        project: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let lines: Vec<String> = self
            .explorer
            .problems
            .get(project)
            .cloned()
            .flatten()
            .unwrap_or_default()
            .iter()
            .flat_map(|(path, ds)| ds.iter().filter(|d| counted(d)).map(|d| line_of(path, d)))
            .collect();
        self.send_problem_lines(None, lines, window, cx);
    }

    /// A file's problems, as its editor underlines them now, to the thread.
    pub fn send_file_problems_to_thread(
        &mut self,
        f: &FileRef,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(d) = self.explorer.docs.get(&DocKey::File(f.clone())) else {
            return;
        };
        let lines: Vec<String> = d
            .editor
            .read(cx)
            .diagnostics()
            .into_iter()
            .flat_map(|set| set.iter())
            .map(|e| lsp_types::Diagnostic {
                range: lsp_types::Range {
                    start: e.diagnostic.range.start,
                    end: e.diagnostic.range.end,
                },
                severity: Some(severity_of(&e.diagnostic.severity)),
                source: e.diagnostic.source.as_ref().map(|s| s.to_string()),
                message: e.diagnostic.message.to_string(),
                ..Default::default()
            })
            .filter(counted)
            .map(|d| line_of(&f.path, &d))
            .collect();
        self.send_problem_lines(Some(f.path.clone()), lines, window, cx);
    }

    fn send_problem_lines(
        &mut self,
        of: Option<String>,
        lines: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if lines.is_empty() {
            self.toast(NotificationType::Info, "No problems to send", window, cx);
            return;
        }
        self.add_reference(Reference::Problems(ProblemsRef { of, lines }), cx);
        self.show_thread_beside(window, cx);
    }

    /// The Problems tab: the head (how many; read again; to the thread), a file's row and its
    /// problems under it, each opening the file at its line.
    pub(crate) fn problems_pane(&mut self, project: &str, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let found = self.explorer.problems.get(project).cloned().flatten();
        let all: Vec<(String, Vec<lsp_types::Diagnostic>)> = found
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|(p, ds)| (p, ds.into_iter().filter(counted).collect::<Vec<_>>()))
            .filter(|(_, ds)| !ds.is_empty())
            .collect();
        let warning =
            |d: &lsp_types::Diagnostic| d.severity == Some(lsp_types::DiagnosticSeverity::WARNING);
        let warnings = all
            .iter()
            .flat_map(|(_, ds)| ds)
            .filter(|d| warning(d))
            .count();
        let errors = all.iter().map(|(_, ds)| ds.len()).sum::<usize>() - warnings;
        let (p1, p2) = (project.to_string(), project.to_string());
        let actions = vec![
            bench_ui::head_icon_button("problems-read", Ico::RefreshCw, "Read again", cx)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.read_problems(p1.clone(), window, cx)
                }))
                .into_any_element(),
            bench_ui::form_action("problems-send", "Send to the thread", false, cx)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.send_problems_to_thread(&p2, window, cx)
                }))
                .into_any_element(),
        ];
        let count = |n: usize, one: &str| format!("{n} {one}{}", if n == 1 { "" } else { "s" });
        let note = format!(
            "{} · {}",
            count(errors, "error"),
            count(warnings, "warning")
        );
        let head = bench_ui::ide_head("Problems", Some(note.into()), actions, cx);
        let root = self.project_root(project);
        let machine = self.location(project).files_machine();
        let mut rows: Vec<AnyElement> = vec![];
        for (n, (path, ds)) in all.iter().enumerate() {
            let rel = root
                .as_ref()
                .and_then(|r| path.strip_prefix(&format!("{}/", r.path.trim_end_matches('/'))))
                .unwrap_or(path);
            let name = rel.rsplit('/').next().unwrap_or(rel).to_string();
            let dir = rel.rsplit_once('/').map(|(d, _)| d.to_string().into());
            rows.push(
                bench_ui::tree_row(
                    format!("problems-file-{n}"),
                    0,
                    None,
                    bench_ui::file_icon(&name, false, &theme),
                    name,
                    dir,
                    false,
                    false,
                    vec![],
                    cx,
                )
                .into_any_element(),
            );
            for (k, d) in ds.iter().enumerate() {
                let color = if warning(d) {
                    theme.warning
                } else {
                    theme.danger
                };
                let at = d.range.start;
                let source = d
                    .source
                    .as_ref()
                    .map_or(String::new(), |s| format!(" · {s}"));
                let f = FileRef {
                    project: project.to_string(),
                    machine: machine.clone(),
                    path: path.clone(),
                };
                rows.push(
                    bench_ui::tree_row(
                        format!("problem-{n}-{k}"),
                        1,
                        None,
                        (Ico::CircleAlert, color),
                        d.message.lines().next().unwrap_or_default().to_string(),
                        Some(format!("{}:{}{source}", at.line + 1, at.character + 1).into()),
                        false,
                        false,
                        vec![],
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_at(f.clone(), at.line as usize + 1, window, cx)
                    }))
                    .into_any_element(),
                );
            }
        }
        let body = match found {
            None => crate::explorer::pane::view::note("Reading…", cx),
            Some(_) if rows.is_empty() => crate::explorer::pane::view::note(
                "No problems in the files the language servers have seen",
                cx,
            ),
            Some(_) => v_flex()
                .id("problems")
                .size_full()
                .overflow_y_scroll()
                .py_1()
                .children(rows)
                .into_any_element(),
        };
        v_flex()
            .size_full()
            .child(head)
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }
}

/// The editor's severity back in LSP's words.
fn severity_of(
    s: &gpui_kit::component::highlighter::DiagnosticSeverity,
) -> lsp_types::DiagnosticSeverity {
    use gpui_kit::component::highlighter::DiagnosticSeverity as S;
    match s {
        S::Warning => lsp_types::DiagnosticSeverity::WARNING,
        S::Info => lsp_types::DiagnosticSeverity::INFORMATION,
        S::Hint => lsp_types::DiagnosticSeverity::HINT,
        _ => lsp_types::DiagnosticSeverity::ERROR,
    }
}
