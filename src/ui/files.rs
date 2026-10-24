//! The mini IDE (right panel): a file tree (or what this session changed) beside a small
//! viewer/editor — Diff / Before / After, save, add to chat, open in Zed. Its files are the
//! gateway's or a server's (`app/ide.rs`); it is shown and hidden as one piece.

use gpui_kit::component::diff::{Diff, DiffChangeIndicator, DiffHunkSeparator, DiffState};
use gpui_kit::component::input::{Editor, Input};
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::resizable::{h_resizable, resizable_panel};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::ide_ops::EditKind;
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
        // A server's tree has no session changes.
        let server = self.ide.server.clone();
        let tab = if server.is_some() {
            RightTab::Files
        } else {
            self.right.unwrap_or(RightTab::Changes)
        };

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

        let quick = self.render_quick(cx);
        // ⌘P's list floats over the panel: drawn last, on top.
        let panel = v_flex()
            .size_full()
            .bg(theme.background)
            .child(
                h_flex()
                    .h(px(36.))
                    .flex_none()
                    .bg(theme.title_bar)
                    .border_b_1()
                    .border_color(theme.border)
                    .when(server.is_none(), |d| {
                        d.child(
                            tab_button("rt-changes", "Changes", tab == RightTab::Changes).on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.toggle_right_tab(RightTab::Changes, window, cx)
                                }),
                            ),
                        )
                    })
                    .child(
                        tab_button("rt-files", "Files", tab == RightTab::Files).on_click(
                            cx.listener(|this, _, window, cx| {
                                this.toggle_right_tab(RightTab::Files, window, cx)
                            }),
                        ),
                    )
                    .when_some(server, |d, alias| {
                        d.child(
                            h_flex()
                                .flex_1()
                                .min_w_0()
                                .px_3()
                                .gap_1p5()
                                .items_center()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(Ico::Server.icon().xsmall())
                                .child(div().truncate().child(alias)),
                        )
                    })
                    .child(div().flex_1())
                    .child(self.ide_window_buttons(cx)),
            )
            .child(div().flex_1().min_h_0().child(
                // No file open: the list is the whole panel (a 240px cell with blank space
                // beside it was the "white strip"); with a file, editor | list, the list
                // fixed so only the editor grows with the panel.
                if self.file.is_some() {
                    h_resizable("right-split")
                        .child(resizable_panel().child(self.render_viewer(cx)))
                        .child(
                            resizable_panel()
                                .size(px(240.))
                                .size_range(px(160.)..px(600.))
                                .fixed()
                                .child(
                                    div()
                                        .size_full()
                                        .border_l_1()
                                        .border_color(theme.border)
                                        .bg(theme.sidebar)
                                        .child(list),
                                ),
                        )
                        .into_any_element()
                } else {
                    div()
                        .size_full()
                        .bg(theme.sidebar)
                        .child(list)
                        .into_any_element()
                },
            ));
        div().size_full().relative().child(panel).children(quick)
    }

    fn render_viewer(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        // Maximized, the toolbar is the window's top strip: header height, window buttons.
        let expanded = self.ide.expanded;
        let win_buttons = expanded.then(|| self.ide_window_buttons(cx).into_any_element());
        let Some(view) = self.file.as_ref() else {
            return div().into_any_element();
        };
        let mode = view.mode;
        let missing = view.missing;
        let has_diff = view.diff_view.is_some();
        let has_before = view.before_view.is_some();
        let can_save = view.loaded && !view.truncated && !missing;
        let remote = self.ide.remote_file.is_some();

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
            .h(px(if expanded { 36. } else { 32. }))
            .flex_none()
            .when(expanded, |d| d.bg(theme.title_bar))
            .px_2()
            .gap_1()
            .items_center()
            .border_b_1()
            .border_color(theme.border)
            .child(self.render_tabs(cx))
            .when(missing, |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .px_1()
                        .child("no longer on disk"),
                )
            })
            .when(has_diff, |d| {
                d.child(
                    mode_button("m-diff", "Diff", FileMode::Diff).on_click(
                        cx.listener(|this, _, _, cx| this.set_file_mode(FileMode::Diff, cx)),
                    ),
                )
            })
            .when(has_before, |d| {
                d.child(
                    mode_button("m-before", "Before", FileMode::Before).on_click(
                        cx.listener(|this, _, _, cx| this.set_file_mode(FileMode::Before, cx)),
                    ),
                )
            })
            .when(!missing, |d| {
                d.child(
                    mode_button(
                        "m-after",
                        if has_diff { "After" } else { "Edit" },
                        FileMode::After,
                    )
                    .on_click(
                        cx.listener(|this, _, _, cx| this.set_file_mode(FileMode::After, cx)),
                    ),
                )
            })
            .when(mode == FileMode::After && can_save, |d| {
                d.child(
                    icon_action("save", Ico::Save, "Save  ⌘S")
                        .on_click(cx.listener(|this, _, window, cx| this.save_file(window, cx))),
                )
            })
            .child(
                icon_action("attach", Ico::Paperclip, "Add to chat")
                    .on_click(cx.listener(|this, _, window, cx| this.attach_open_file(window, cx))),
            )
            .children(win_buttons);

        let diff_of = |state: &Entity<DiffState>| {
            Diff::new(state)
                .line_number(true)
                .syntax_highlight(true)
                .header_visible(false)
                .change_indicator(DiffChangeIndicator::None)
                .hunk_separator(DiffHunkSeparator::Simple)
                .into_any_element()
        };
        let note = |text: &'static str| {
            div()
                .p_3()
                .text_sm()
                .text_color(muted)
                .child(text)
                .into_any_element()
        };
        let body = match (mode, &view.diff_view, &view.before_view) {
            (FileMode::Diff, Some(state), _) => diff_of(state),
            (FileMode::Before, _, Some(state)) => diff_of(state),
            _ if missing => note("This file is no longer on disk."),
            (FileMode::After, _, _) => {
                let warn = view.truncated.then(|| {
                    div()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(theme.warning)
                        .child(if remote {
                            "Large file: the beginning is shown, read-only. The agent can still \
                             change the whole file (Add to chat); changes go to the server."
                        } else {
                            "Large file: only the beginning is loaded, saving is disabled."
                        })
                });
                // The server's file changed since it was opened: nothing is written until the
                // user picks a side.
                let conflict = self.ide.remote_file.as_ref().is_some_and(|rf| rf.conflict);
                let choice = |id: &'static str, label: &'static str| {
                    div()
                        .id(id)
                        .px_2()
                        .py_0p5()
                        .cursor_pointer()
                        .border_1()
                        .border_color(theme.border)
                        .text_color(theme.foreground)
                        .hover(|s| s.bg(theme.list_hover))
                        .child(label)
                };
                let conflict = conflict.then(|| {
                    h_flex()
                        .px_2()
                        .py_1()
                        .gap_2()
                        .items_center()
                        .text_xs()
                        .text_color(theme.warning)
                        .border_b_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child("Changed on the server since it was opened. Not saved."),
                        )
                        .child(choice("conflict-reload", "Reload").on_click(
                            cx.listener(|this, _, window, cx| this.reload_remote_file(window, cx)),
                        ))
                        .child(
                            choice("conflict-overwrite", "Overwrite").on_click(cx.listener(
                                |this, _, window, cx| this.overwrite_remote_file(window, cx),
                            )),
                        )
                });
                v_flex()
                    .size_full()
                    .children(conflict)
                    .children(warn)
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .child(Editor::new(&self.file_editor).appearance(false).h_full()),
                    )
                    .into_any_element()
            }
            _ => note("Loading…"),
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
            let name = file_name(root.trim_end_matches('/'));
            let r = root.clone();
            // The root as the first row (like Zed): its name, the full path on hover.
            rows.push(
                h_flex()
                    .id("tree-root")
                    .h(rems(TREE_ROW_REMS))
                    .pl(px(TREE_PAD))
                    .pr_2()
                    .gap_1p5()
                    .items_center()
                    .text_sm()
                    .child(
                        Ico::FolderOpen
                            .icon()
                            .small()
                            .text_color(theme.muted_foreground),
                    )
                    .child(div().truncate().child(if name.is_empty() {
                        root.clone()
                    } else {
                        name
                    }))
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("tree-refresh")
                            .p_0p5()
                            .cursor_pointer()
                            .text_color(theme.muted_foreground)
                            .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                            .child(Ico::RefreshCw.icon().xsmall())
                            .tooltip(|window, cx| Tooltip::new("Refresh").build(window, cx))
                            .on_click(cx.listener(|this, _, window, cx| {
                                cx.stop_propagation();
                                this.refresh_tree(window, cx)
                            })),
                    )
                    .tooltip(move |window, cx| Tooltip::new(r.clone()).build(window, cx))
                    .context_menu(tree_menu(cx.entity(), root.clone(), true, false, true))
                    .into_any_element(),
            );
            rows.extend(self.render_tree_edit(&root, 1, cx));
            self.tree_rows(&root, 1, &mut rows, cx);
        }
        v_flex()
            .id("tree")
            .size_full()
            .overflow_y_scroll()
            .py_1()
            .children(rows)
            .child(div().h_2().flex_none())
    }

    /// One row per entry, Zed-style: indent guides, an icon by kind, folders whose only entry is
    /// a folder shown as one row (`a / b`), the open file in the accent, the path on hover.
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
            // Fold a chain of single folders into this row.
            let (mut label, mut path) = (name.clone(), path.clone());
            if is_dir {
                while self.tree_open.contains(&path) {
                    match self.tree.get(&path).map(|e| e.as_slice()) {
                        Some([(child, child_path, true)]) => {
                            label = format!("{label} / {child}");
                            path = child_path.clone();
                        }
                        _ => break,
                    }
                }
            }
            let open = is_dir && self.tree_open.contains(&path);
            let selected = self.file.as_ref().is_some_and(|f| f.path == path);
            let icon = if is_dir {
                folder_icon(&name, open)
            } else {
                file_icon(&name)
            };
            let (target, tip) = (path.clone(), path.clone());
            let view = cx.entity();
            let armed = self.ide.armed_delete.as_deref() == Some(path.as_str());
            let guides = (1..depth).map(|level| {
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(
                        TREE_PAD + TREE_INDENT * level as f32 - TREE_INDENT / 2. + 1.
                    ))
                    .w(px(1.))
                    .bg(theme.border)
            });
            rows.push(
                h_flex()
                    .id(SharedString::from(format!("t:{path}")))
                    .relative()
                    .h(rems(TREE_ROW_REMS))
                    .flex_none()
                    .gap_1p5()
                    .pl(px(TREE_PAD
                        + TREE_INDENT * (depth - 1) as f32
                        + TREE_INDENT / 2.))
                    .pr_2()
                    .items_center()
                    .text_sm()
                    .cursor_pointer()
                    .border_1()
                    .border_color(if selected {
                        theme.primary
                    } else {
                        theme.transparent
                    })
                    .when(selected, |d| d.bg(theme.list_active))
                    .when(!selected, |d| d.hover(|s| s.bg(theme.list_hover)))
                    .children(guides)
                    .child(icon.icon().small().text_color(if selected {
                        theme.primary
                    } else {
                        theme.muted_foreground
                    }))
                    .map(|d| match self.ide.edit.as_ref() {
                        // Renaming this one: its name becomes a field.
                        Some(e) if e.kind == EditKind::Rename(path.clone()) => {
                            d.child(div().flex_1().child(Input::new(&e.input).xsmall()))
                        }
                        _ => d.child(div().min_w_0().truncate().child(label)),
                    })
                    .when(armed, |d| d.text_color(theme.danger))
                    .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if is_dir {
                            this.open_dir(target.clone(), window, cx)
                        } else {
                            this.open_file(target.clone(), window, cx)
                        }
                    }))
                    .context_menu(tree_menu(view, path.clone(), is_dir, armed, false))
                    .into_any_element(),
            );
            // A new file/folder being named in this folder: a field as its first row.
            if open || self.edit_in(&path) {
                rows.extend(self.render_tree_edit(&path, depth + 1, cx));
            }
            if open {
                self.tree_rows(&path, depth + 1, rows, cx);
            }
        }
    }

    /// Maximize/restore and close: at the right end of the IDE's top strip (or, maximized, of
    /// the editor's).
    fn ide_window_buttons(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let expanded = self.ide.expanded;
        // The header's geometry (24px squares, 4px apart, 8px from the edge): the buttons that
        // take turns in this corner (Open files / Close files, the mode switch) stay in place.
        h_flex()
            .h_full()
            .flex_none()
            .items_center()
            .gap_1()
            .pr_2()
            // When the mini IDE is not enough: the same folder (and file) in Zed.
            .child(
                div()
                    .id("ide-zed")
                    .p_1()
                    .cursor_pointer()
                    .text_color(theme.muted_foreground)
                    .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                    .child(Ico::Zed.icon().small())
                    .tooltip(|window, cx| Tooltip::new("Open in Zed").build(window, cx))
                    .on_click(cx.listener(|this, _, window, cx| this.open_in_zed(window, cx))),
            )
            // Hides the whole IDE (tree and editor). Second from the right, where "Open files"
            // sits in the header: open and close without moving the pointer.
            .child(
                div()
                    .id("ide-close")
                    .p_1()
                    .cursor_pointer()
                    .text_color(theme.primary)
                    .hover(|s| s.bg(theme.list_hover))
                    .child(Ico::PanelRight.icon().small())
                    .tooltip(|window, cx| Tooltip::new("Close files  ⌘B").build(window, cx))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.request_close_ide(window, cx)),
                    ),
            )
            // The whole window for the IDE, and back (⌘⇧B): last, at the window's edge.
            .child(
                div()
                    .id("ide-expand")
                    .p_1()
                    .cursor_pointer()
                    .text_color(if expanded {
                        theme.primary
                    } else {
                        theme.muted_foreground
                    })
                    .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                    .child(
                        if expanded {
                            Ico::Minimize
                        } else {
                            Ico::Maximize
                        }
                        .icon()
                        .small(),
                    )
                    .tooltip(move |window, cx| {
                        Tooltip::new(if expanded {
                            "Restore  ⌘⇧B"
                        } else {
                            "Maximize  ⌘⇧B"
                        })
                        .build(window, cx)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.ide.expanded = !this.ide.expanded;
                        cx.notify();
                    })),
            )
    }

    /// Maximized (⌘⇧B), in place of the sidebar and at its width: just the tree (or the
    /// session's changes, if that tab was open) under the window buttons — an immersive view
    /// needs no label saying where it is.
    pub(super) fn render_ide_sidebar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let tab = if self.ide.server.is_some() {
            RightTab::Files
        } else {
            self.right.unwrap_or(RightTab::Changes)
        };
        let list = match tab {
            RightTab::Changes => self.render_changes_list(cx).into_any_element(),
            RightTab::Files => self.render_tree(cx).into_any_element(),
        };
        v_flex()
            .size_full()
            .bg(theme.sidebar)
            .child(self.render_sidebar_top(cx))
            .child(div().flex_1().min_h_0().child(list))
            .into_any_element()
    }

    /// Maximized, in place of the chat/terminal: the open files' tabs and the editor; ⌘P floats
    /// over it.
    pub(super) fn render_ide_main(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let quick = self.render_quick(cx);
        let body = if self.file.is_some() {
            self.render_viewer(cx)
        } else {
            v_flex()
                .size_full()
                .child(
                    h_flex()
                        .h(px(36.))
                        .flex_none()
                        .bg(theme.title_bar)
                        .border_b_1()
                        .border_color(theme.border)
                        .child(div().flex_1())
                        .child(self.ide_window_buttons(cx)),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("Pick a file on the left  ·  ⌘P finds one"),
                )
                .into_any_element()
        };
        div()
            .size_full()
            .relative()
            .bg(theme.background)
            .child(body)
            .children(quick)
            .into_any_element()
    }

    /// ⌘P: a field and the best matches, over the IDE's top (↑↓ pick, Enter opens, Esc closes).
    fn render_quick(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let q = self.ide.quick.as_ref()?;
        let theme = cx.theme().clone();
        let root = self.tree_root.clone().unwrap_or_default();
        let matches = self.quick_matches(12, cx);
        let loading = q.files.is_none();
        let sel = q.sel;
        Some(
            v_flex()
                .id("quick-open")
                .absolute()
                .top(px(36.))
                .left_0()
                .right_0()
                .bg(theme.popover)
                .border_b_1()
                .border_color(theme.border)
                .capture_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                    match ev.keystroke.key.as_str() {
                        "up" => this.quick_move(-1, cx),
                        "down" => this.quick_move(1, cx),
                        _ => return,
                    }
                    cx.stop_propagation();
                }))
                .child(
                    div()
                        .p_2()
                        .child(Input::new(&q.input).prefix(Ico::Search.icon().small())),
                )
                .when(loading, |d| {
                    d.child(
                        div()
                            .px_3()
                            .pb_2()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("Listing files…"),
                    )
                })
                .children(matches.into_iter().enumerate().map(|(k, path)| {
                    let rel = path
                        .strip_prefix(&root)
                        .unwrap_or(&path)
                        .trim_start_matches('/')
                        .to_string();
                    let name = file_name(&path);
                    let dir = rel
                        .strip_suffix(&name)
                        .unwrap_or("")
                        .trim_end_matches('/')
                        .to_string();
                    let p = path.clone();
                    h_flex()
                        .id(("quick", k))
                        .px_3()
                        .py_1()
                        .gap_2()
                        .items_center()
                        .text_sm()
                        .cursor_pointer()
                        .when(k == sel, |d| d.bg(theme.list_active))
                        .hover(|s| s.bg(theme.list_hover))
                        .child(name)
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(dir),
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.quick_pick(Some(p.clone()), window, cx)
                        }))
                }))
                .into_any_element(),
        )
    }

    /// The open files as tabs (the toolbar's left part): name, a dot for unsaved edits, ×.
    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let active = self.file.as_ref().map(|f| f.path.clone());
        h_flex()
            .id("ide-tabs")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_x_scroll()
            .children(self.ide.tabs.iter().enumerate().map(|(k, path)| {
                let on = active.as_deref() == Some(path.as_str());
                let dirty = self.is_dirty(path);
                let armed = self.ide.armed_close.as_deref() == Some(path.as_str());
                let group = SharedString::from(format!("ide-tab-{k}"));
                let (p_open, p_close, tip) = (path.clone(), path.clone(), path.clone());
                h_flex()
                    .id(("ide-tab", k))
                    .group(group.clone())
                    .flex_none()
                    .h_full()
                    .pl_2()
                    .pr_1()
                    .gap_1()
                    .items_center()
                    .text_sm()
                    .cursor_pointer()
                    .border_r_1()
                    .border_color(theme.border)
                    .when(on, |d| d.bg(theme.background).text_color(theme.foreground))
                    .when(!on, |d| {
                        d.text_color(theme.muted_foreground)
                            .hover(|s| s.text_color(theme.foreground))
                    })
                    .child(file_name(path))
                    // Unsaved: a dot where the × goes; the × shows on hover.
                    .child(
                        div()
                            .id(("ide-tab-x", k))
                            .size(px(16.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(if armed {
                                theme.danger
                            } else {
                                theme.muted_foreground
                            })
                            .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                            .map(|d| {
                                if dirty && !armed {
                                    // Unsaved edits: a dot (clicking it closes, asking again).
                                    d.child(div().size(px(6.)).rounded_full().bg(theme.foreground))
                                } else {
                                    d.child(Ico::X.icon().xsmall())
                                        .opacity(if on || armed { 1. } else { 0. })
                                        .group_hover(group.clone(), |s| s.opacity(1.))
                                }
                            })
                            .tooltip(move |window, cx| {
                                Tooltip::new(if armed {
                                    "Unsaved edits — click again to close"
                                } else if dirty {
                                    "Unsaved edits (⌘S saves)"
                                } else {
                                    "Close"
                                })
                                .build(window, cx)
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.close_tab(p_close.clone(), window, cx)
                            })),
                    )
                    .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.switch_tab(p_open.clone(), window, cx)
                    }))
            }))
    }

    /// Is a new file/folder being named directly in `dir`?
    fn edit_in(&self, dir: &str) -> bool {
        matches!(self.ide.edit.as_ref().map(|e| &e.kind),
            Some(EditKind::NewFile(d) | EditKind::NewDir(d)) if d == dir)
    }

    /// The field of a new file/folder in `dir`, as a tree row (or nothing).
    fn render_tree_edit(
        &self,
        dir: &str,
        depth: usize,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let theme = cx.theme().clone();
        let e = self.ide.edit.as_ref()?;
        let icon = match &e.kind {
            EditKind::NewFile(d) if d == dir => Ico::File,
            EditKind::NewDir(d) if d == dir => Ico::Folder,
            _ => return None,
        };
        Some(
            h_flex()
                .h(rems(TREE_ROW_REMS))
                .flex_none()
                .gap_1p5()
                .pl(px(TREE_PAD
                    + TREE_INDENT * (depth - 1) as f32
                    + TREE_INDENT / 2.))
                .pr_2()
                .items_center()
                .child(icon.icon().small().text_color(theme.muted_foreground))
                .child(div().flex_1().child(Input::new(&e.input).xsmall()))
                .into_any_element(),
        )
    }
}

/// A tree row's right-click menu: new file/folder (inside a folder, else beside the file),
/// rename, delete (twice). The root (`root`) offers only the new ones: it is never renamed or
/// deleted from here.
fn tree_menu(
    view: Entity<HermesApp>,
    path: String,
    is_dir: bool,
    armed: bool,
    root: bool,
) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
    move |menu, _, _| {
        let dir = if is_dir {
            path.clone()
        } else {
            path.rsplit_once('/')
                .map_or(path.clone(), |(d, _)| d.to_string())
        };
        let act = |f: Box<dyn Fn(&mut HermesApp, &mut Window, &mut Context<HermesApp>)>| {
            let view = view.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                view.update(cx, |this, cx| f(this, window, cx))
            }
        };
        let (d1, d2, p1, p2) = (dir.clone(), dir.clone(), path.clone(), path.clone());
        let menu = menu
            .item(
                PopupMenuItem::new("New file")
                    .icon(Ico::File.icon())
                    .on_click(act(Box::new(move |this, window, cx| {
                        this.start_tree_edit(EditKind::NewFile(d1.clone()), window, cx)
                    }))),
            )
            .item(
                PopupMenuItem::new("New folder")
                    .icon(Ico::Folder.icon())
                    .on_click(act(Box::new(move |this, window, cx| {
                        this.start_tree_edit(EditKind::NewDir(d2.clone()), window, cx)
                    }))),
            );
        if root {
            return menu;
        }
        menu.separator()
            .item(
                PopupMenuItem::new("Rename")
                    .icon(Ico::Pencil.icon())
                    .on_click(act(Box::new(move |this, window, cx| {
                        this.start_tree_edit(EditKind::Rename(p1.clone()), window, cx)
                    }))),
            )
            .item(
                PopupMenuItem::new(if armed { "Confirm delete" } else { "Delete" })
                    .icon(Ico::Trash.icon())
                    .on_click(act(Box::new(move |this, window, cx| {
                        this.delete_path(p2.clone(), window, cx)
                    }))),
            )
    }
}

/// Tree rows: a little taller than the text, like Zed's project panel.
const TREE_ROW_REMS: f32 = 1.75;
const TREE_PAD: f32 = 8.;
const TREE_INDENT: f32 = 16.;

fn folder_icon(name: &str, open: bool) -> Ico {
    let n = name.trim_start_matches('.').to_lowercase();
    match n.as_str() {
        "git" => Ico::FolderGit,
        "config" | "conf" | "etc" | "settings" => Ico::FolderCog,
        "ssh" | "gnupg" | "certs" | "ssl" | "secrets" | "pki" => Ico::FolderLock,
        "cache" | "node_modules" | "vendor" | "target" | "dist" | "build" => Ico::FolderArchive,
        "src" | "lib" | "app" | "bin" | "scripts" => Ico::FolderCode,
        _ if open => Ico::FolderOpen,
        _ => Ico::Folder,
    }
}

fn file_icon(name: &str) -> Ico {
    let lower = name.to_lowercase();
    let ext = lower.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    // Dotfiles like .bashrc, .env, .npmrc are configuration.
    if lower.starts_with('.') && !lower[1..].contains('.') {
        return Ico::FileCog;
    }
    match ext {
        "sh" | "bash" | "zsh" | "fish" => Ico::FileTerminal,
        "rs" | "py" | "js" | "mjs" | "ts" | "tsx" | "jsx" | "go" | "c" | "h" | "cpp" | "hpp"
        | "java" | "kt" | "rb" | "php" | "swift" | "lua" | "html" | "css" | "scss" | "vue" => {
            Ico::FileCode
        }
        "json" | "yaml" | "yml" | "toml" | "xml" => Ico::FileJson,
        "conf" | "cfg" | "ini" | "env" | "properties" | "service" => Ico::FileCog,
        "md" | "txt" | "log" | "rst" | "csv" => Ico::FileText,
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "ico" | "bmp" => Ico::FileImage,
        "zip" | "tar" | "gz" | "tgz" | "xz" | "bz2" | "7z" | "rar" => Ico::FileArchive,
        "sql" | "db" | "sqlite" | "sqlite3" => Ico::Database,
        "key" | "pem" | "crt" | "cer" | "p12" | "lock" => Ico::FileLock,
        "mp4" | "mov" | "mkv" | "webm" | "avi" => Ico::FileVideo,
        "mp3" | "wav" | "flac" | "ogg" | "m4a" => Ico::FileAudio,
        _ => Ico::File,
    }
}
