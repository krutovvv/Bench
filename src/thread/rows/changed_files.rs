//! Changed files: under the last answer, every file the thread's work edited or wrote, with its
//! +/- counts.

use ::thread::{Row, Step};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::thread::ThreadView;
use bench_ui::icons::Ico;
use bench_ui::plural;

impl ThreadView {
    /// After the last turn: every file this session edited, with +/- counts.
    pub(crate) fn render_changed_files(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let files = self.changed_files();
        if files.is_empty() || self.thread.journal.running() {
            return None;
        }
        let theme = cx.theme().clone();
        let open = self.thread.files_open;
        let at = self.thread_ref();
        let count = files.len();
        let (all_add, all_del) = files
            .iter()
            .flat_map(|(_, diffs)| diffs.iter().map(|d| crate::thread::rows::diff::stats(d)))
            .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
        // Paths as the repository has them (or under ~), the dir quieter than the name: as
        // Xcode's source control navigator lists changes. Ten, then "Show all".
        let base = self.thread.repo.as_ref().map(|r| format!("{}/", r.dir.trim_end_matches('/')));
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
        let all = self.thread.files_all;
        let hidden = count.saturating_sub(FIRST);
        let faint = bench_ui::faint(&theme);
        let mut rows: Vec<AnyElement> = files
            .into_iter()
            .filter(|_| open)
            .take(if all { usize::MAX } else { FIRST })
            .enumerate()
            .map(|(k, (path, diffs))| {
                let (add, del) = diffs
                    .iter()
                    .map(|d| crate::thread::rows::diff::stats(d))
                    .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
                let shown = short(&path);
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
                let (open, at) = (path.clone(), at.clone());
                h_flex()
                    .id(("file", k))
                    .h(px(bench_ui::LIST_ROW_H))
                    .px_3()
                    .gap_2()
                    .items_center()
                    .text_size(px(bench_ui::THREAD_META))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.list_hover))
                    .child(ico.icon().xsmall().text_color(color))
                    .child(div().flex_none().child(name))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(faint)
                            .child(dir),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .gap_1p5()
                            .font_family(bench_ui::theme::MONO)
                            .text_size(theme.mono_font_size)
                            .when(add > 0, |d| d.child(div().text_color(theme.success).child(format!("+{add}"))))
                            .when(del > 0, |d| d.child(div().text_color(theme.danger).child(format!("−{del}"))))
                            .when(diffs.is_empty(), |d| d.child(div().text_color(faint).child("new"))),
                    )
                    // In the Explorer, the thread beside it.
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
        if open && hidden > 0 {
            rows.push(
                h_flex()
                    .id("files-all")
                    .h(px(bench_ui::LIST_ROW_H))
                    .px_3()
                    .items_center()
                    .text_size(px(bench_ui::THREAD_META))
                    .text_color(theme.muted_foreground)
                    .cursor_pointer()
                    .hover(|s| s.text_color(theme.foreground))
                    .child(if all { "Show fewer".to_string() } else { format!("Show all {count}") })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.thread.files_all = !this.thread.files_all;
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        // The rows' own air under the head, as a list in a Mac sidebar.
        let rows = (!rows.is_empty()).then(|| v_flex().py_1().children(rows));
        Some(
            bench_ui::block_frame(cx)
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
                    .text_size(px(bench_ui::THREAD_META))
                    .when(all_add > 0, |d| {
                        d.child(div().font_family(bench_ui::theme::MONO).text_size(theme.mono_font_size).text_color(theme.success).child(format!("+{all_add}")))
                    })
                    .when(all_del > 0, |d| {
                        d.child(div().font_family(bench_ui::theme::MONO).text_size(theme.mono_font_size).text_color(theme.danger).child(format!("−{all_del}")))
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
