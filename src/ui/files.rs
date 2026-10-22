//! Right panel: what this session changed (or the project tree) and a small viewer/editor —
//! Diff / Before / After, save back to the gateway's disk, or open in Zed.

use gpui_kit::component::input::Editor;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{FileMode, HermesApp, RightTab};
use crate::icons::Ico;

fn file_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

impl HermesApp {
    pub(super) fn render_right(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let tab = self.right.unwrap_or(RightTab::Changes);

        let tab_button = |id: &'static str, label: &'static str, active: bool| {
            div()
                .id(id)
                .h_full()
                .px_3()
                .flex()
                .items_center()
                .text_sm()
                .cursor_pointer()
                .border_r_1()
                .border_color(theme.border)
                .when(active, |d| {
                    d.bg(theme.background).text_color(theme.foreground)
                })
                .when(!active, |d| {
                    d.text_color(theme.muted_foreground)
                        .hover(|s| s.text_color(theme.foreground))
                })
                .child(label)
        };

        let list = match tab {
            RightTab::Changes => self.render_changes_list(cx).into_any_element(),
            RightTab::Files => self.render_tree(cx).into_any_element(),
        };

        v_flex()
            .size_full()
            .bg(theme.background)
            .child(
                h_flex()
                    .h(px(36.))
                    .flex_none()
                    .bg(theme.title_bar)
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        tab_button("rt-changes", "Changes", tab == RightTab::Changes).on_click(
                            cx.listener(|this, _, window, cx| {
                                this.toggle_right_tab(RightTab::Changes, window, cx)
                            }),
                        ),
                    )
                    .child(
                        tab_button("rt-files", "Files", tab == RightTab::Files).on_click(
                            cx.listener(|this, _, window, cx| {
                                this.toggle_right_tab(RightTab::Files, window, cx)
                            }),
                        ),
                    )
                    .child(div().flex_1()),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(self.render_viewer(cx)),
                    )
                    .child(
                        div()
                            .w(px(230.))
                            .flex_none()
                            .h_full()
                            .border_l_1()
                            .border_color(theme.border)
                            .bg(theme.sidebar)
                            .child(list),
                    ),
            )
    }

    fn render_viewer(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let Some(view) = self.file.as_ref() else {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(muted)
                .child("Select a file")
                .into_any_element();
        };
        let has_diff = !view.diffs.is_empty();
        let mode = view.mode;
        let can_save = view.loaded && !view.truncated;

        let mode_button = |id: &'static str, label: &'static str, m: FileMode| {
            let active = mode == m;
            div()
                .id(id)
                .px_2()
                .py_0p5()
                .text_xs()
                .cursor_pointer()
                .border_1()
                .border_color(if active { theme.primary } else { theme.border })
                .text_color(if active { theme.foreground } else { muted })
                .hover(|s| s.text_color(theme.foreground))
                .child(label)
        };
        let icon_action = |id: &'static str, icon: Ico, tip: &'static str| {
            div()
                .id(id)
                .p_1()
                .cursor_pointer()
                .text_color(muted)
                .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                .child(icon.icon().small())
                .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
        };

        let toolbar = h_flex()
            .h(px(32.))
            .flex_none()
            .px_2()
            .gap_1()
            .items_center()
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_sm()
                    .truncate()
                    .child(file_name(&view.path))
                    .id("viewer-path")
                    .tooltip({
                        let p: SharedString = view.path.clone().into();
                        move |window, cx| Tooltip::new(p.clone()).build(window, cx)
                    }),
            )
            .when(has_diff, |d| {
                d.child(
                    mode_button("m-diff", "Diff", FileMode::Diff).on_click(
                        cx.listener(|this, _, _, cx| this.set_file_mode(FileMode::Diff, cx)),
                    ),
                )
                .child(
                    mode_button("m-before", "Before", FileMode::Before).on_click(
                        cx.listener(|this, _, _, cx| this.set_file_mode(FileMode::Before, cx)),
                    ),
                )
            })
            .child(
                mode_button(
                    "m-after",
                    if has_diff { "After" } else { "Edit" },
                    FileMode::After,
                )
                .on_click(cx.listener(|this, _, _, cx| this.set_file_mode(FileMode::After, cx))),
            )
            .when(mode == FileMode::After && can_save, |d| {
                d.child(
                    icon_action("save", Ico::Save, "Save to disk")
                        .on_click(cx.listener(|this, _, window, cx| this.save_file(window, cx))),
                )
            })
            .child(
                icon_action("zed", Ico::ExternalLink, "Open in Zed")
                    .on_click(cx.listener(|this, _, window, cx| this.open_in_zed(window, cx))),
            );

        let body = match mode {
            FileMode::Diff => render_diff(&crate::diff::joined(&view.diffs), cx).into_any_element(),
            FileMode::Before => match &view.before {
                Ok(text) => v_flex()
                    .id("before")
                    .size_full()
                    .overflow_scroll()
                    .p_2()
                    .font_family("Menlo")
                    .text_xs()
                    .child(text.clone())
                    .into_any_element(),
                Err(why) => div()
                    .p_3()
                    .text_sm()
                    .text_color(muted)
                    .child(why.clone())
                    .into_any_element(),
            },
            FileMode::After => {
                let note = view.truncated.then(|| {
                    div()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(theme.warning)
                        .child("Large file: only the beginning is loaded, saving is disabled.")
                });
                v_flex()
                    .size_full()
                    .children(note)
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .child(Editor::new(&self.file_editor).appearance(false).h_full()),
                    )
                    .into_any_element()
            }
        };

        v_flex()
            .size_full()
            .child(toolbar)
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }

    fn render_changes_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let files = self.session_files();
        let empty = files.is_empty();
        let rows: Vec<AnyElement> = files
            .into_iter()
            .enumerate()
            .map(|(k, (path, diffs))| {
                let (add, del) = diffs
                    .iter()
                    .map(|d| crate::diff::stats(d))
                    .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
                let selected = self.file.as_ref().is_some_and(|f| f.path == path);
                let open = path.clone();
                h_flex()
                    .id(("chg", k))
                    .gap_1p5()
                    .px_2()
                    .py_1()
                    .items_center()
                    .text_sm()
                    .cursor_pointer()
                    .when(selected, |d| d.bg(theme.list_active))
                    .hover(|s| s.bg(theme.list_hover))
                    .child(Ico::File.icon().xsmall().text_color(theme.muted_foreground))
                    .child(div().flex_1().min_w_0().truncate().child(file_name(&path)))
                    .when(add > 0, |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(theme.success)
                                .child(format!("+{add}")),
                        )
                    })
                    .when(del > 0, |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(theme.danger)
                                .child(format!("−{del}")),
                        )
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_file(open.clone(), window, cx)
                    }))
                    .into_any_element()
            })
            .collect();
        v_flex()
            .id("changes-list")
            .size_full()
            .overflow_y_scroll()
            .py_1()
            .children(rows)
            .when(empty, |d| {
                d.child(
                    div()
                        .p_3()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("No file edits in this session."),
                )
            })
    }

    fn render_tree(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut rows: Vec<AnyElement> = vec![];
        if let Some(root) = self.tree_root.clone() {
            rows.push(
                div()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .truncate()
                    .child(root.clone())
                    .into_any_element(),
            );
            self.tree_rows(&root, 0, &mut rows, cx);
        }
        v_flex()
            .id("tree")
            .size_full()
            .overflow_y_scroll()
            .py_1()
            .children(rows)
    }

    fn tree_rows(
        &self,
        dir: &str,
        depth: usize,
        rows: &mut Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) {
        let theme = cx.theme().clone();
        let Some(entries) = self.tree.get(dir) else {
            return;
        };
        for (name, path, is_dir) in entries.clone() {
            let open = self.tree_open.contains(&path);
            let selected = self.file.as_ref().is_some_and(|f| f.path == path);
            let target = path.clone();
            rows.push(
                h_flex()
                    .id(SharedString::from(format!("t:{path}")))
                    .gap_1p5()
                    .pl(px(8. + depth as f32 * 14.))
                    .pr_2()
                    .py_0p5()
                    .items_center()
                    .text_sm()
                    .cursor_pointer()
                    .when(selected, |d| d.bg(theme.list_active))
                    .hover(|s| s.bg(theme.list_hover))
                    .child(
                        (if is_dir {
                            if open { Ico::FolderOpen } else { Ico::Folder }
                        } else {
                            Ico::File
                        })
                        .icon()
                        .xsmall()
                        .text_color(theme.muted_foreground),
                    )
                    .child(div().truncate().child(name))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if is_dir {
                            this.open_dir(target.clone(), window, cx)
                        } else {
                            this.open_file(target.clone(), window, cx)
                        }
                    }))
                    .into_any_element(),
            );
            if is_dir && open {
                self.tree_rows(&path, depth + 1, rows, cx);
            }
        }
    }
}

/// A unified diff, colored line by line.
fn render_diff(diff: &str, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let lines: Vec<AnyElement> = diff
        .lines()
        .map(|l| {
            let (bg, fg) = if l.starts_with("+++") || l.starts_with("---") {
                (theme.secondary, theme.muted_foreground)
            } else if l.starts_with("@@") {
                (theme.secondary, theme.muted_foreground)
            } else if l.starts_with('+') {
                (theme.success.opacity(0.14), theme.foreground)
            } else if l.starts_with('-') {
                (theme.danger.opacity(0.14), theme.foreground)
            } else {
                (theme.transparent, theme.foreground)
            };
            div()
                .px_2()
                .bg(bg)
                .text_color(fg)
                .whitespace_nowrap()
                .child(if l.is_empty() {
                    " ".to_string()
                } else {
                    l.to_string()
                })
                .into_any_element()
        })
        .collect();
    v_flex()
        .id("diff")
        .size_full()
        .overflow_scroll()
        .py_1()
        .font_family("Menlo")
        .text_xs()
        .children(lines)
}
