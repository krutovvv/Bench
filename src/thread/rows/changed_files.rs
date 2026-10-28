//! Changed files, one quiet folded line with the totals: under each turn's answer what that turn
//! changed in its repository (`Row::Changes`, from git, `agent/src/runtime/turns.ts`); in a folder
//! that is not a repository, under the last answer, every file the thread's work edited or wrote.
//! Unfolded, a file per row; a click on a row shows its diff in place, under it — the thread is
//! where the person works, so the change is read there. "Open in Code" on a row's hover opens
//! the same diff as a tab.

use ::thread::{Row, Step};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::explorer::pane::doc::Turn;
use crate::thread::ThreadView;
use bench_ui::icons::Ico;
use bench_ui::plural;

/// A file in a "Changed files" block.
#[derive(Clone)]
pub(crate) struct Changed {
    pub path: String,
    pub add: usize,
    pub del: usize,
    /// Made by a write the thread has no diff of.
    pub new: bool,
    /// Its diff when the thread holds it already (the file tools' own); None: read from the
    /// turn's trees when first shown.
    pub diff: Option<String>,
}

/// The key the thread-wide block (no repository) keeps its folds under.
const ALL: &str = "files";

impl ThreadView {
    /// After the last turn, in a folder that is not a repository: every file this session edited.
    pub(crate) fn render_changed_files(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.thread.journal.running()
            || self.thread.tv.rows.iter().any(|r| matches!(r, Row::Changes { .. }))
        {
            return None;
        }
        let files: Vec<Changed> = self
            .changed_files()
            .into_iter()
            .map(|(path, diffs)| {
                let (add, del) = diffs
                    .iter()
                    .map(|d| crate::thread::rows::diff::stats(d))
                    .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
                Changed {
                    path,
                    add,
                    del,
                    new: diffs.is_empty(),
                    diff: (!diffs.is_empty()).then(|| diffs.join("\n")),
                }
            })
            .collect();
        if files.is_empty() {
            return None;
        }
        Some(self.changes_block(ALL, files, None, cx))
    }

    /// What one turn changed, under its answer.
    pub(crate) fn render_turn_changes(
        &mut self,
        key: &str,
        turn: Turn,
        files: &[protocol::FileChange],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let files = files
            .iter()
            .map(|f| Changed {
                path: f.path.clone(),
                add: f.add as usize,
                del: f.del as usize,
                new: false,
                diff: None,
            })
            .collect();
        let block = self.changes_block(key, files, Some(turn), cx);
        crate::thread::look::turn(Some(true), block, cx).into_any_element()
    }

    /// The block: its folded head with the totals, the files when open, a file's diff under
    /// its row when that row was clicked.
    fn changes_block(
        &mut self,
        key: &str,
        files: Vec<Changed>,
        turn: Option<Turn>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let open = self.thread.files_open.contains(key);
        let all = self.thread.files_all.contains(key);
        let count = files.len();
        let (all_add, all_del) = files.iter().fold((0, 0), |a, f| (a.0 + f.add, a.1 + f.del));
        // Paths as the repository has them (or under ~), the dir quieter than the name: as
        // Xcode's source control navigator lists changes. Ten, then "Show all".
        let base = turn
            .as_ref()
            .map(|t| t.root.clone())
            .or_else(|| self.thread.repo.as_ref().map(|r| r.dir.clone()))
            .map(|d| format!("{}/", d.trim_end_matches('/')));
        let home = std::env::var("HOME").ok().map(|h| format!("{h}/"));
        let short = |path: &str| -> String {
            if let Some(rest) = base.as_deref().and_then(|b| path.strip_prefix(b)) {
                return rest.to_string();
            }
            match home.as_deref().and_then(|h| path.strip_prefix(h)) {
                Some(rest) => format!("~/{rest}"),
                None => path.to_string(),
            }
        };
        const FIRST: usize = 10;
        let hidden = count.saturating_sub(FIRST);
        let faint = bench_ui::faint(&theme);
        let mut rows: Vec<AnyElement> = vec![];
        for (k, f) in files
            .into_iter()
            .filter(|_| open)
            .take(if all { usize::MAX } else { FIRST })
            .enumerate()
        {
            let shown = short(&f.path);
            let p = std::path::Path::new(&shown);
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let dir = p
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let (ico, color) = bench_ui::file_icon(&name, false, &theme);
            let slot = (key.to_string(), f.path.clone());
            let shows = self.thread.diffs_open.contains(&slot);
            let group: SharedString = format!("chg-{key}-{k}").into();
            let (toggle, turn_c, f_c) = (slot.clone(), turn.clone(), f.clone());
            let (code_path, code_turn, at) = (f.path.clone(), turn.clone(), self.thread_ref());
            rows.push(
                h_flex()
                    .id(SharedString::from(format!("chg-{key}-{k}")))
                    .group(group.clone())
                    .h(px(bench_ui::LIST_ROW_H))
                    .px_3()
                    .gap_2()
                    .items_center()
                    .text_size(px(bench_ui::THREAD_META))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.list_hover))
                    .child(ico.icon().xsmall().text_color(color))
                    .child(div().flex_none().child(name))
                    .child(div().flex_1().min_w_0().truncate().text_color(faint).child(dir))
                    // In Code, as a tab: the turn's diff, or the file itself.
                    .child(
                        div()
                            .invisible()
                            .group_hover(group, |s| s.visible())
                            .child(
                                bench_ui::head_icon_button(
                                    &format!("chg-code-{key}-{k}"),
                                    Ico::ExternalLink,
                                    "Open in Code",
                                    cx,
                                )
                                .on_click(self.app_listener(move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    let project = at.project_or_host();
                                    let machine = this.location(&project).files_machine();
                                    let file = crate::explorer::FileRef {
                                        project,
                                        machine,
                                        path: code_path.clone(),
                                    };
                                    let key = match code_turn.clone() {
                                        Some(turn) => crate::explorer::DocKey::TurnDiff { file, turn },
                                        None => crate::explorer::DocKey::File(file),
                                    };
                                    this.open_doc(key, window, cx);
                                    this.right_dock.activate(crate::workspace::Panel::Thread);
                                })),
                            ),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .gap_1p5()
                            .font_family(bench_ui::theme::MONO)
                            .text_size(theme.mono_font_size)
                            .when(f.add > 0, |d| d.child(div().text_color(theme.success).child(format!("+{}", f.add))))
                            .when(f.del > 0, |d| d.child(div().text_color(theme.danger).child(format!("−{}", f.del))))
                            .when(f.new, |d| d.child(div().text_color(faint).child("new"))),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.claim_click(cx);
                        this.toggle_diff(toggle.clone(), &f_c, turn_c.clone(), window, cx)
                    }))
                    .into_any_element(),
            );
            if shows {
                rows.push(self.render_inline_diff(&slot, &f, cx));
            }
        }
        if open && hidden > 0 {
            let k = key.to_string();
            rows.push(
                h_flex()
                    .id(SharedString::from(format!("chg-all-{key}")))
                    .h(px(bench_ui::LIST_ROW_H))
                    .px_3()
                    .items_center()
                    .text_size(px(bench_ui::THREAD_META))
                    .text_color(theme.muted_foreground)
                    .cursor_pointer()
                    .hover(|s| s.text_color(theme.foreground))
                    .child(if all { "Show fewer".to_string() } else { format!("Show all {count}") })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.thread.files_all.remove(&k) {
                            this.thread.files_all.insert(k.clone());
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        // The rows' own air under the head, as a list in a Mac sidebar.
        let rows = (!rows.is_empty()).then(|| v_flex().py_1().children(rows));
        let k = key.to_string();
        bench_ui::block_frame(cx)
            // Folded by default: one line with the totals; a click lists the files.
            .child(
                bench_ui::fold_head(
                    SharedString::from(format!("chg-head-{key}")),
                    open,
                    Some(Ico::FileDiff),
                    format!("Changed files · {}", plural(count, "file", "files", "files")),
                    cx,
                )
                // The thread's one text size: the head reads like the rows under it.
                .text_size(px(bench_ui::THREAD_META))
                .when(all_add > 0, |d| {
                    d.child(div().font_family(bench_ui::theme::MONO).text_size(theme.mono_font_size).text_color(theme.success).child(format!("+{all_add}")))
                })
                .when(all_del > 0, |d| {
                    d.child(div().font_family(bench_ui::theme::MONO).text_size(theme.mono_font_size).text_color(theme.danger).child(format!("−{all_del}")))
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !this.thread.files_open.remove(&k) {
                        this.thread.files_open.insert(k.clone());
                    }
                    cx.notify();
                })),
            )
            .children(rows)
            .into_any_element()
    }

    /// A file's diff shown under its row, or folded again; a turn's is read from its trees once.
    fn toggle_diff(
        &mut self,
        slot: (String, String),
        f: &Changed,
        turn: Option<Turn>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.thread.diffs_open.remove(&slot) {
            cx.notify();
            return;
        }
        self.thread.diffs_open.insert(slot.clone());
        if self.thread.diffs.contains_key(&slot) {
            cx.notify();
            return;
        }
        if let Some(d) = &f.diff {
            self.thread.diffs.insert(slot, Some(d.clone()));
            cx.notify();
            return;
        }
        let Some(turn) = turn else { return };
        self.thread.diffs.insert(slot.clone(), None);
        cx.notify();
        let (path, me, at) = (f.path.clone(), cx.entity().downgrade(), self.thread_ref());
        self.app
            .update(cx, |app, cx| {
                let project = at.project_or_host();
                let machine = app.location(&project).files_machine();
                if app.demo {
                    let text = crate::demo::turn_diff(&path);
                    me.update(cx, |v, cx| {
                        v.thread.diffs.insert(slot, Some(text));
                        cx.notify();
                    })
                    .ok();
                    return;
                }
                let Some((conn, _)) = app.env_target(&project) else {
                    return;
                };
                let file = crate::explorer::FileRef { project, machine, path };
                app.background(
                    window,
                    cx,
                    move || crate::explorer::code::turn_diff::turn_diff(&conn, &file, &turn),
                    move |_, r, _, cx| {
                        let text = r.unwrap_or_else(|e| format!("Could not read the turn's trees: {e:#}"));
                        me.update(cx, |v, cx| {
                            v.thread.diffs.insert(slot, Some(text));
                            cx.notify();
                        })
                        .ok();
                    },
                );
            })
            .ok();
    }

    /// The diff under a file's row: as a code block of the answer's, coloured as a diff.
    fn render_inline_diff(
        &mut self,
        slot: &(String, String),
        f: &Changed,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let body = match self.thread.diffs.get(slot).cloned().flatten() {
            Some(d) => {
                // The row above names the file: the diff's own `---`/`+++` head is left out.
                let hunks: Vec<&str> = d
                    .lines()
                    .skip_while(|l| l.starts_with("--- ") || l.starts_with("+++ "))
                    .collect();
                let text = self.thread.texts.markdown(
                    &format!("diff-{}-{}", slot.0, f.path),
                    &format!("```diff\n{}\n```", hunks.join("\n").trim_end()),
                    cx,
                );
                text.into_any_element()
            }
            None => div()
                .text_color(cx.theme().muted_foreground)
                .child("Reading…")
                .into_any_element(),
        };
        div()
            .px_3()
            .pb_1()
            .text_size(px(bench_ui::THREAD_META))
            .child(body)
            .into_any_element()
    }
}

impl ThreadView {
    /// Every file the thread's work edited or wrote: (path, its diffs).
    pub fn changed_files(&self) -> Vec<(String, Vec<String>)> {
        let mut out: Vec<(String, Vec<String>)> = vec![];
        for r in &self.thread.tv.rows {
            let Row::Work { steps, .. } = r else { continue };
            for s in steps {
                let Step::Tool {
                    name,
                    args,
                    diff,
                    failed: false,
                    output: Some(_),
                    ..
                } = s
                else {
                    continue;
                };
                if !matches!(name.as_str(), "edit" | "write") {
                    continue;
                }
                let Some(path) = args.get("path").and_then(|p| p.as_str()) else {
                    continue;
                };
                let slot = match out.iter().position(|(p, _)| p == path) {
                    Some(i) => &mut out[i].1,
                    None => {
                        out.push((path.to_string(), vec![]));
                        &mut out.last_mut().unwrap().1
                    }
                };
                slot.extend(diff.clone());
            }
        }
        out
    }
}
