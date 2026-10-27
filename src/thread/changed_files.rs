//! Changed files: under the last answer, every file the thread's work edited or wrote, with its
//! +/- counts.

use ::thread::{Row, Step};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::ThreadView;
use bench_ui::icons::Ico;
use bench_ui::plural;

impl ThreadView {
    /// After the last turn: every file this session edited, with +/- counts.
    pub(crate) fn render_session_files(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let files = self.session_files();
        if files.is_empty() || self.thread.journal.running() {
            return None;
        }
        let theme = cx.theme().clone();
        let open = self.thread.files_open;
        let at = self.thread_ref();
        let count = files.len();
        let (all_add, all_del) = files
            .iter()
            .flat_map(|(_, diffs)| diffs.iter().map(|d| crate::thread::diff::stats(d)))
            .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
        let rows: Vec<AnyElement> = files
            .into_iter()
            .filter(|_| open)
            .enumerate()
            .map(|(k, (path, diffs))| {
                let (add, del) = diffs
                    .iter()
                    .map(|d| crate::thread::diff::stats(d))
                    .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
                let p = std::path::Path::new(&path);
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let dir = p
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let selected = false;
                let (open, at) = (path.clone(), at.clone());
                h_flex()
                    .id(("file", k))
                    .gap_2()
                    .px_2()
                    .py_1()
                    .items_center()
                    .text_base()
                    .cursor_pointer()
                    .border_l_2()
                    .border_color(if selected {
                        theme.primary
                    } else {
                        theme.transparent
                    })
                    .when(selected, |d| d.bg(theme.list_active))
                    .hover(|s| s.bg(theme.list_hover))
                    .child(name)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_base()
                            .text_color(theme.muted_foreground)
                            .truncate()
                            .child(dir),
                    )
                    .when(add > 0, |d| {
                        d.child(
                            div()
                                .text_base()
                                .text_color(theme.success)
                                .child(format!("+{add}")),
                        )
                    })
                    .when(del > 0, |d| {
                        d.child(
                            div()
                                .text_base()
                                .text_color(theme.danger)
                                .child(format!("−{del}")),
                        )
                    })
                    .when(diffs.is_empty(), |d| {
                        d.child(
                            div()
                                .text_base()
                                .text_color(theme.muted_foreground)
                                .child("written"),
                        )
                    })
                    // In the editor, the thread beside it (Zed from the file's head).
                    .on_click(self.app_listener(move |this, _, window, cx| {
                        let project = at.project_or_host();
                        let machine = this.location(&project).files_machine();
                        let f = crate::explorer::FileRef {
                            project,
                            machine,
                            path: open.clone(),
                        };
                        this.open_doc(crate::explorer::DocKey::File(f), window, cx);
                        this.right_dock.activate(crate::workspace::Panel::Thread);
                    }))
                    .into_any_element()
            })
            .collect();
        Some(
            v_flex()
                .bg(theme.background)
                .border_1()
                .rounded_md()
                .border_color(theme.border)
                .overflow_hidden()
                // Folded by default: one line with the totals; a click lists the files.
                .child(
                    bench_ui::fold_head(
                        "files-head",
                        open,
                        Some(Ico::FileDiff),
                        format!(
                            "Changed files · {}",
                            plural(count, "file", "files", "files")
                        ),
                        cx,
                    )
                    // The thread's one text size: the head reads like the rows under it.
                    .text_base()
                    .when(all_add > 0, |d| {
                        d.child(div().text_color(theme.success).child(format!("+{all_add}")))
                    })
                    .when(all_del > 0, |d| {
                        d.child(div().text_color(theme.danger).child(format!("−{all_del}")))
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.thread.files_open = !this.thread.files_open;
                        cx.notify();
                    })),
                )
                .children(rows)
                .into_any_element(),
        )
    }
}

impl ThreadView {
    /// Every file the thread's work edited or wrote: (path, its diffs).
    pub fn session_files(&self) -> Vec<(String, Vec<String>)> {
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
