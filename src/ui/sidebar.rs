//! Left column: where, which profile, navigation, sessions (grouped by source), status bar.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::plural;
use crate::app::{HermesApp, Panel};
use crate::icons::Ico;
use crate::model::source_label;

/// A session title: the sidebar's 14px.
pub(super) const SESSION_TITLE: f32 = 14.;

impl HermesApp {
    pub(super) fn render_sidebar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if self.tasks.on {
            return self.render_tasks_sidebar(cx);
        }
        if self.tools.on {
            return self.render_tools_sidebar(cx);
        }
        if self.ssh.on {
            return self.render_ssh_sidebar(cx);
        }
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;

        // Group visible sessions: by git project when they work in one, else by where they live
        // (Bench, Telegram, cron…). Plain Bench chats first, then projects, then the rest.
        let mut groups: Vec<(String, Vec<usize>)> = vec![];
        let mut rank: Vec<(String, u8)> = vec![];
        // A project/folder group's place on disk and whether it is a git repo (for its "+").
        let mut homes: Vec<(String, String, bool)> = vec![];
        for i in self.visible_sessions() {
            // A thread with a subject is reached through its subject's row (a task list…).
            if !self.sessions[i].subject.is_empty() {
                continue;
            }
            let id = &self.sessions[i].id;
            let folder = self
                .folder_of(id)
                .map(|f| f.rsplit('/').next().unwrap_or(&f).to_string());
            let (label, r) = match (self.project_of(id), folder) {
                // Pinned chats sit together at the very top, whatever they belong to.
                _ if self.mark_of(id).pinned => ("Pinned".to_string(), 0),
                (Some(p), _) => {
                    if !homes.iter().any(|(l, _, _)| *l == p.name()) {
                        homes.push((p.name(), p.root.clone(), true));
                    }
                    (p.name(), 2)
                }
                (None, Some(f)) => {
                    if let Some(path) = self.folder_of(id) {
                        if !homes.iter().any(|(l, _, _)| *l == f) {
                            homes.push((f.clone(), path, false));
                        }
                    }
                    (f, 2)
                }
                (None, None) => {
                    let l = source_label(&self.sessions[i].source);
                    let r = if l == "Bench" { 1 } else { 3 };
                    (l, r)
                }
            };
            match groups.iter_mut().find(|(l, _)| *l == label) {
                Some((_, rows)) => rows.push(i),
                None => {
                    rank.push((label.clone(), r));
                    groups.push((label, vec![i]));
                }
            }
        }
        let rank_of = |l: &str| {
            rank.iter()
                .find(|(x, _)| x == l)
                .map(|(_, r)| *r)
                .unwrap_or(3)
        };
        groups.sort_by(|(a, _), (b, _)| {
            (rank_of(a), a.to_lowercase()).cmp(&(rank_of(b), b.to_lowercase()))
        });
        // One plain group needs no header; a project always names itself.
        let single = groups.len() <= 1 && groups.first().is_none_or(|(l, _)| rank_of(l) == 1);

        let mut list: Vec<AnyElement> = vec![];
        let searching = !self.search_query.is_empty();
        if !searching {
            if crate::bench::TASKS {
                list.extend(self.render_task_lists(cx));
            }
            if crate::bench::SSH {
                list.extend(self.render_server_group(cx));
            }
            list.extend(self.render_files_group(cx));
            list.extend(self.render_sites_group(cx));
            if crate::bench::TOOLS {
                list.extend(self.render_tool_group(cx));
            }
        }
        if searching {
            match &self.search_hits {
                None => list.push(self.sidebar_note("Searching…", cx)),
                Some(hits) if hits.is_empty() => list.push(self.sidebar_note("Nothing found", cx)),
                Some(hits) => {
                    for (k, (id, snippet)) in hits.iter().enumerate() {
                        list.push(
                            self.render_search_hit(k, id, snippet, cx)
                                .into_any_element(),
                        );
                    }
                }
            }
            groups.clear();
        }
        for (g, (label, rows)) in groups.into_iter().enumerate() {
            if !single {
                let home = homes.iter().find(|(l, _, _)| *l == label).cloned();
                let group: SharedString = format!("ghead{g}").into();
                // Like a folder in a file tree: what kind of group it is, its name, a count;
                // its chats hang below on an indent guide.
                let icon = match (label.as_str(), &home) {
                    ("Pinned", _) => Ico::Pin,
                    (_, Some((_, _, true))) => Ico::FolderGit,
                    // A named group of chats (the composer's "Group" field), not a folder you made.
                    (_, Some((_, path, _))) if path.contains("/Research/") => Ico::Layers,
                    (_, Some(_)) => Ico::Folder,
                    ("Telegram", _) => Ico::Send,
                    ("Cron", _) => Ico::Clock,
                    ("API", _) => Ico::Globe,
                    _ => Ico::Chat,
                };
                let key = format!("s:{label}");
                let collapsed = self.ssh.collapsed.contains(&key);
                // Tasks, Servers… stand above: every group head keeps one step.
                let top = rems(0.75);
                list.push(
                    h_flex()
                        .id(("ghead", g))
                        .group(group.clone())
                        .cursor_pointer()
                        .px_4()
                        .pt(top)
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.toggle_group(key.clone(), cx)),
                        )
                        .pb_1()
                        .gap_1p5()
                        .items_center()
                        .text_sm()
                        .child(super::head_icon(icon, cx))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .font_weight(FontWeight::MEDIUM)
                                // Group names step back (muted, like the icon); the chats in
                                // them are what is read.
                                .text_color(muted)
                                .child(label.clone()),
                        )
                        .child(div().flex_1())
                        .child(super::group_chevron(collapsed, group.clone(), cx))
                        // "+", always shown (icon, name, "+": a balanced row): a new chat in
                        // the project/folder (a project's in a branch of its own), or a plain
                        // Bench chat. Telegram, cron and pinned groups can't start one here.
                        .when(!(home.is_some() || label == "Bench"), |d| d.child(super::head_plus_slot()))
                        .when(home.is_some() || label == "Bench", |d| {
                            let tip = match &home {
                                Some((_, _, true)) => "New chat in this project",
                                Some((_, path, _)) if path.contains("/Research/") => {
                                    "New chat in this group"
                                }
                                Some(_) => "New chat in this folder",
                                None => "New chat",
                            };
                            let home = home.clone();
                            d.child(
                                super::head_plus_slot()
                                    .id(("group-new", g))
                                    .cursor_pointer()
                                    .text_color(muted)
                                    .hover(|s| {
                                        s.bg(theme.sidebar_accent).text_color(theme.foreground)
                                    })
                                    .child(Ico::Plus.icon().xsmall())
                                    .tooltip(move |window, cx| {
                                        gpui_kit::component::tooltip::Tooltip::new(tip)
                                            .build(window, cx)
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        match &home {
                                            Some((_, path, repo)) => {
                                                this.new_chat_in(Some(path.clone()), *repo, cx);
                                                this.new_project_repo = *repo;
                                            }
                                            None => this.new_session(cx),
                                        }
                                    })),
                            )
                        })
                        .into_any_element(),
                );
            }
            // A folded group shows its header only.
            if !single && self.ssh.collapsed.contains(&format!("s:{label}")) {
                continue;
            }
            let n = rows.len();
            for (k, i) in rows.into_iter().enumerate() {
                let guide = (!single).then_some(k + 1 == n);
                list.push(self.render_session_row(i, guide, cx).into_any_element());
            }
        }

        v_flex()
            .size_full()
            .bg(theme.sidebar)
            // Under the window buttons: the search as a band, then the groups as an IDE's tree
            // (a new chat: "+" on a group, or in the top strip).
            .child(self.render_sidebar_top(cx))
            .child(super::search_band(&self.search, cx))
            .child(
                v_flex()
                    .id("sessions")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .py_1()
                    .children(list)
                    .when(self.connecting, |d| {
                        d.child(
                            div()
                                .px_4()
                                .py_2()
                                .text_sm()
                                .text_color(muted)
                                .child("Connecting…"),
                        )
                    }),
            )
            .into_any_element()
    }

    fn sidebar_note(&self, text: &'static str, cx: &mut Context<Self>) -> AnyElement {
        div()
            .px_4()
            .py_2()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(text)
            .into_any_element()
    }

    /// A search result: the session's title (if it is in the list) and the matching snippet.
    fn render_search_hit(
        &self,
        k: usize,
        id: &str,
        snippet: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let selected = self.panel == Panel::Chat && self.session.as_deref() == Some(id);
        let title = self
            .sessions
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.title.clone())
            .unwrap_or_else(|| "Untitled session".into());
        let open_id = id.to_string();
        v_flex()
            .id(("hit", k))
            .pl(px(14.))
            .pr_3()
            .py_2()
            .cursor_pointer()
            .border_l_2()
            .border_color(if selected {
                theme.primary
            } else {
                theme.transparent
            })
            .when(selected, |d| d.bg(theme.sidebar_accent))
            .hover(|s| s.bg(theme.sidebar_accent))
            .child(div().text_size(px(SESSION_TITLE)).truncate().child(title))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .line_clamp(2)
                    .child(snippet.replace(">>>", "").replace("<<<", "")),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.open_session(open_id.clone(), cx)))
    }

    /// `guide`: under a group header, indented on the group's guide line (`Some(last row)`).
    fn render_session_row(
        &self,
        i: usize,
        guide: Option<bool>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let s = &self.sessions[i];
        let selected = self.panel == Panel::Chat && self.session.as_deref() == Some(&s.id);
        let armed = self.confirm_delete.as_deref() == Some(&s.id);
        let group: SharedString = format!("srow{i}").into();
        let count = if s.running {
            "working…".to_string()
        } else {
            plural(s.messages as usize, "message", "messages", "messages")
        };
        // A chat in a branch of its own says which.
        let meta = match self.project_of(&s.id).filter(|p| p.in_branch_folder()) {
            Some(p) if !p.branch.is_empty() => format!("{} · {count}", p.branch),
            _ => count,
        };
        let (open_id, del_id) = (s.id.clone(), s.id.clone());
        let compact = self.appearance.compact_sidebar;

        let delete = if armed {
            h_flex()
                .gap_1()
                .text_xs()
                .child(
                    div()
                        .id(("del-yes", i))
                        .px_1p5()
                        .py_0p5()
                        .bg(theme.danger)
                        .text_color(theme.danger_foreground)
                        .cursor_pointer()
                        .child("Delete")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.delete_session(del_id.clone(), window, cx)
                        })),
                )
                .child(
                    div()
                        .id(("del-no", i))
                        .px_1()
                        .py_0p5()
                        .cursor_pointer()
                        .text_color(muted)
                        .hover(|s| s.text_color(theme.foreground))
                        .child("Cancel")
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.cancel_delete(cx)
                        })),
                )
                .into_any_element()
        } else {
            // "⋯" on hover: pin, color, its files (the mini IDE; Zed is one click from there),
            // delete.
            let view = cx.entity();
            let id = s.id.clone();
            let mark = self.mark_of(&s.id);
            div()
                .opacity(0.)
                .group_hover(group.clone(), |s| s.opacity(1.))
                // Opening the menu must not also open the chat.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    Button::new(("more", i))
                        .ghost()
                        .xsmall()
                        .icon(Ico::Ellipsis.icon())
                        .dropdown_menu(move |menu, _, _| {
                            let act = |f: Box<
                                dyn Fn(&mut HermesApp, &mut Window, &mut Context<HermesApp>),
                            >| {
                                let view = view.clone();
                                move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                                    view.update(cx, |this, cx| f(this, window, cx))
                                }
                            };
                            let (pin_id, del) = (id.clone(), del_id.clone());
                            let files_id = id.clone();
                            let (swatch_view, swatch_id) = (view.clone(), id.clone());
                            menu.item(
                                PopupMenuItem::new(if mark.pinned { "Unpin" } else { "Pin" })
                                    .icon(Ico::Pin.icon())
                                    .on_click(act(Box::new(move |this, _, cx| {
                                        this.toggle_pin(pin_id.clone(), cx)
                                    }))),
                            )
                            .separator()
                            // Reads the color live, so a click shows at once in the open menu.
                            .item(PopupMenuItem::element(move |_, cx| {
                                let current = swatch_view.read(cx).mark_of(&swatch_id).color;
                                color_row(&swatch_view, &swatch_id, current, cx)
                            }))
                            .separator()
                            .item(
                                PopupMenuItem::new("Open files")
                                    .icon(Ico::FolderTree.icon())
                                    .on_click(act(Box::new(move |this, window, cx| {
                                        this.open_ide_for_session(files_id.clone(), window, cx)
                                    }))),
                            )
                            .separator()
                            .item(
                                PopupMenuItem::new("Delete")
                                    .icon(Ico::Trash.icon())
                                    .on_click(act(Box::new(move |this, window, cx| {
                                        this.delete_session(del.clone(), window, cx)
                                    }))),
                            )
                        }),
                )
                .into_any_element()
        };
        // A chosen color marks the row with a quiet stripe (the open chat keeps the accent).
        let working =
            s.running || (self.live.is_some() && self.session.as_deref() == Some(s.id.as_str()));
        let stripe = self
            .mark_of(&s.id)
            .color
            .and_then(|c| crate::app::marks::SESSION_COLORS.get(c))
            .map(|c| -> Hsla { rgb(*c).into() });

        h_flex()
            .id(("session", i))
            .group(group)
            .relative()
            .gap_1()
            .pl(if guide.is_some() {
                super::nest_pl(cx)
            } else {
                px(14.)
            })
            .when_some(guide, |d, last| d.child(super::nest_guide(cx, last)))
            .pr_3()
            .map(|d| if compact { d.py_1p5() } else { d.py(px(5.)) })
            .cursor_pointer()
            .border_l_2()
            // A chosen color wins over the accent; it also tints the row lightly.
            .border_color(stripe.unwrap_or(if selected {
                theme.primary
            } else {
                theme.transparent
            }))
            .when_some(stripe, |d, c| {
                d.bg(Hsla {
                    a: if selected { 0.22 } else { 0.12 },
                    ..c
                })
            })
            // The open chat in the accent's own tint, not one more grey.
            .when(selected && stripe.is_none(), |d| {
                d.bg(theme.primary.opacity(0.12))
                    .text_color(theme.foreground)
            })
            // Hover keeps a chosen color, half a step denser; plain rows get the usual grey.
            .hover(move |h| match stripe {
                Some(c) => h.bg(Hsla {
                    a: if selected { 0.30 } else { 0.20 },
                    ..c
                }),
                None => h.bg(theme.sidebar_accent),
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    // A working session's title carries a moving highlight, to find it at a glance.
                    .child(div().text_size(px(SESSION_TITLE)).truncate().map(|d| {
                        if working {
                            d.child(ShimmerText::new(s.title.clone()))
                        } else {
                            d.child(s.title.clone())
                        }
                    }))
                    // Compact: the title alone (a working session still shimmers).
                    .when(!compact, |d| {
                        d.child(div().text_xs().text_color(muted).child(meta))
                    }),
            )
            .child(delete)
            .on_click(cx.listener(move |this, _, _, cx| this.open_session(open_id.clone(), cx)))
    }
}

/// The swatches row of a session's menu: the colors and "none".
fn color_row(view: &Entity<HermesApp>, id: &str, current: Option<usize>, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let swatch = |k: Option<usize>| {
        let (view, id) = (view.clone(), id.to_string());
        let on = current == k;
        div()
            .id(SharedString::from(format!("swatch-{k:?}")))
            .size(px(16.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .border_1()
            .border_color(if on {
                theme.foreground
            } else {
                theme.transparent
            })
            .map(|d| match k {
                Some(c) => d.child(
                    div()
                        .size(px(12.))
                        .bg(Hsla::from(rgb(crate::app::marks::SESSION_COLORS[c]))),
                ),
                None => d
                    .text_color(theme.muted_foreground)
                    .child(Ico::X.icon().xsmall()),
            })
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                view.update(cx, |this, cx| this.set_color(id.clone(), k, cx));
            })
    };
    h_flex()
        .gap_1p5()
        .py_1()
        .children((0..crate::app::marks::SESSION_COLORS.len()).map(|c| swatch(Some(c))))
        .child(swatch(None))
        .into_any_element()
}

impl HermesApp {
    /// The Tasks group: a folder of the task lists, each opening its thread (the list live on
    /// top, the conversation under it).
    fn render_task_lists(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let key = "s:Tasks".to_string();
        let collapsed = self.ssh.collapsed.contains(&key);
        let current = self.current_subject();
        let compact = self.appearance.compact_sidebar;
        let mut out = vec![h_flex()
            .id("tasks-head")
            .group("tasks-head")
            .cursor_pointer()
            .px_4()
            .pt(rems(0.5))
            .pb_1()
            .gap_1p5()
            .items_center()
            .text_sm()
            .child(super::head_icon(Ico::ListTodo, cx))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(muted)
                    .child("Tasks"),
            )
            .child(div().flex_1())
            .child(super::group_chevron(collapsed, "tasks-head".into(), cx))
            .child(super::head_plus_slot())
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_group(key.clone(), cx)))
            .into_any_element()];
        if collapsed {
            return out;
        }
        let n = crate::tasks::LISTS.len();
        for (k, list) in crate::tasks::LISTS.iter().enumerate() {
            let key = HermesApp::tasks_subject(list);
            let selected = self.panel == Panel::Chat && current == key && self.session.is_none();
            let threads = self.thread_rows(&key, k + 1 == n, cx);
            let group: SharedString = format!("task-list-{k}").into();
            let new_key = key.clone();
            let count = match list {
                crate::tasks::List::Today
                | crate::tasks::List::Upcoming
                | crate::tasks::List::Anytime => self.tasks.count(list),
                _ => 0,
            };
            let l = list.clone();
            out.push(
                h_flex()
                    .id(("task-list", k))
                    .group(group.clone())
                    .relative()
                    .gap_1()
                    .pl(super::nest_pl(cx))
                    .child(super::nest_guide(cx, k + 1 == n && threads.is_empty()))
                    .pr_4()
                    .map(|d| if compact { d.py_1p5() } else { d.py(px(5.)) })
                    .cursor_pointer()
                    .border_l_2()
                    .border_color(if selected { theme.primary } else { theme.transparent })
                    .when(selected, |d| d.bg(theme.primary.opacity(0.12)))
                    .hover(|h| h.bg(theme.sidebar_accent))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(SESSION_TITLE))
                            .child(list.name()),
                    )
                    .when(count > 0, |d| {
                        d.child(div().text_sm().text_color(muted).child(count.to_string()))
                    })
                    // "+" on hover: a new thread about this list.
                    .child(
                        row_plus(format!("task-list-new-{k}").into(), group, cx).on_click(cx.listener(
                            move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.new_subject_thread(new_key.clone(), window, cx)
                            },
                        )),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_task_list(l.clone(), window, cx)
                    }))
                    .into_any_element(),
            );
            out.extend(threads);
        }
        out
    }

    /// A subject group's header: an icon (in its square), the name, the chevron that folds it,
    /// then the "+" (`plus`) or the room of one — the same columns as the project groups'.
    fn subject_head(
        &mut self,
        id: &'static str,
        icon: Ico,
        name: &'static str,
        plus: Option<Box<dyn Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static>>,
        cx: &mut Context<Self>,
    ) -> (AnyElement, bool) {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let key = format!("s:{name}");
        let collapsed = self.ssh.collapsed.contains(&key);
        let plus_el = match plus {
            Some(f) => super::head_plus_slot()
                .id(SharedString::from(format!("{id}-plus")))
                .cursor_pointer()
                .text_color(muted)
                .hover(|s| s.bg(theme.sidebar_accent).text_color(theme.foreground))
                .child(Ico::Plus.icon().xsmall())
                .on_click(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    f(this, window, cx)
                }))
                .into_any_element(),
            None => super::head_plus_slot().into_any_element(),
        };
        let head = h_flex()
            .id(id)
            .group(id)
            .cursor_pointer()
            .px_4()
            .pt(rems(0.75))
            .pb_1()
            .gap_1p5()
            .items_center()
            .text_sm()
            .child(super::head_icon(icon, cx))
            .child(div().min_w_0().truncate().font_weight(FontWeight::MEDIUM).text_color(muted).child(name))
            .child(div().flex_1())
            .child(super::group_chevron(collapsed, id.into(), cx))
            .child(plus_el)
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_group(key.clone(), cx)))
            .into_any_element();
        (head, collapsed)
    }

    /// A row under a subject group: the name, a quiet note, the open thread tinted. With
    /// `plus` (the subject's key), a "+" on hover starts a new thread about it.
    #[allow(clippy::too_many_arguments)]
    fn subject_row(
        &self,
        id: SharedString,
        name: String,
        note: String,
        selected: bool,
        last: bool,
        plus: Option<String>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = cx.theme().clone();
        let compact = self.appearance.compact_sidebar;
        let group = id.clone();
        let plus_el = plus.map(|key| {
            row_plus(SharedString::from(format!("{id}-new")), group.clone(), cx).on_click(cx.listener(
                move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.new_subject_thread(key.clone(), window, cx)
                },
            ))
        });
        h_flex()
            .id(id)
            .group(group)
            .relative()
            .gap_2()
            .pl(super::nest_pl(cx))
            .child(super::nest_guide(cx, last))
            // The right column of the group heads ("+" at 16 from the edge).
            .pr_4()
            .map(|d| if compact { d.py_1p5() } else { d.py(px(5.)) })
            .cursor_pointer()
            .border_l_2()
            .border_color(if selected { theme.primary } else { theme.transparent })
            .when(selected, |d| d.bg(theme.primary.opacity(0.12)))
            .hover(|h| h.bg(theme.sidebar_accent))
            .child(div().flex_none().text_size(px(SESSION_TITLE)).child(name))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(note),
            )
            .children(plus_el)
    }

    /// The threads about `key`, listed under its row while it is the open subject: one job
    /// each, the latest first; the open one tinted. `last`: the subject's row ends its group.
    fn thread_rows(&self, key: &str, last: bool, cx: &mut Context<Self>) -> Vec<AnyElement> {
        if self.current_subject() != key || self.panel != Panel::Chat {
            return vec![];
        }
        let theme = cx.theme().clone();
        let compact = self.appearance.compact_sidebar;
        let rows = self.threads_of(key);
        let n = rows.len();
        rows.into_iter()
            .enumerate()
            .map(|(k, i)| {
                let s = &self.sessions[i];
                let selected = self.session.as_deref() == Some(&s.id);
                let title = if s.title.is_empty() { "New thread".to_string() } else { s.title.clone() };
                let note = if s.running { "working…".to_string() } else { plural(s.messages as usize, "message", "messages", "messages") };
                let (key, id) = (key.to_string(), s.id.clone());
                h_flex()
                    .id(SharedString::from(format!("thr-{}", s.id)))
                    .relative()
                    .gap_2()
                    .pl(super::nest_pl(cx) + px(12.))
                    .child(super::nest_guide(cx, last && k + 1 == n))
                    .pr_4()
                    .map(|d| if compact { d.py_1() } else { d.py(px(4.)) })
                    .cursor_pointer()
                    .border_l_2()
                    .border_color(if selected { theme.primary } else { theme.transparent })
                    .when(selected, |d| d.bg(theme.primary.opacity(0.12)))
                    .hover(|h| h.bg(theme.sidebar_accent))
                    .child(div().flex_1().min_w_0().truncate().text_size(px(SESSION_TITLE)).child(title))
                    .child(div().flex_none().text_sm().text_color(theme.muted_foreground).child(note))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_subject_thread(key.clone(), id.clone(), window, cx)
                    }))
                    .into_any_element()
            })
            .collect()
    }

    /// The Servers group: every host of ~/.ssh/config; a click opens its thread, the terminal on top.
    fn render_server_group(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let (head, collapsed) = self.subject_head(
            "servers-head",
            Ico::Server,
            "Servers",
            // "+": a new server (its form takes the chat area).
            Some(Box::new(|this: &mut Self, window, cx| {
                this.panel = Panel::Chat;
                this.edit_server(None, None, window, cx)
            })),
            cx,
        );
        let mut out = vec![head];
        if collapsed {
            return out;
        }
        let current = self.current_subject();
        let servers = self.ssh.servers.clone();
        let n = servers.len() + 1;
        let mut rows: Vec<(String, String)> = vec![(crate::term::LOCAL.to_string(), "this Mac".into())];
        rows.extend(servers.iter().map(|s| {
            let at = if s.user.is_empty() { s.hostname.clone() } else { format!("{}@{}", s.user, s.hostname) };
            (s.alias.clone(), at)
        }));
        for (k, (alias, note)) in rows.into_iter().enumerate() {
            let key = format!("ssh:{alias}");
            let selected = self.panel == Panel::Chat && current == key && self.session.is_none();
            let name = if alias == crate::term::LOCAL { "Terminal".to_string() } else { alias.clone() };
            let a = alias.clone();
            let threads = self.thread_rows(&key, k + 1 == n, cx);
            out.push(
                self.subject_row(format!("srv-{k}").into(), name, note, selected, k + 1 == n && threads.is_empty(), Some(key), cx)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_server_thread(a.clone(), window, cx)
                    }))
                    .into_any_element(),
            );
            out.extend(threads);
        }
        out
    }

    /// Tools: a folder of them, each tool a thread (the tool on top); folded by the chevron.
    fn render_tool_group(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let (head, collapsed) = self.subject_head("tools-head", Ico::Wrench, "Tools", None, cx);
        let mut out = vec![head];
        if collapsed {
            return out;
        }
        let current = self.current_subject();
        let tools: Vec<(crate::devtools::Tool, &'static str)> = crate::devtools::GROUPS
            .iter()
            .flat_map(|g| g.tools.iter().map(move |t| (*t, g.name)))
            .collect();
        let n = tools.len();
        for (k, (tool, group)) in tools.into_iter().enumerate() {
            let key = format!("tool:{}", crate::app::subjects::tool_key(tool));
            let selected = self.panel == Panel::Chat && current == key && self.session.is_none();
            let threads = self.thread_rows(&key, k + 1 == n, cx);
            out.push(
                self.subject_row(format!("tool-{k}").into(), tool.name().to_string(), group.to_string(), selected, k + 1 == n && threads.is_empty(), Some(key), cx)
                    .on_click(cx.listener(move |this, _, window, cx| this.open_tool_thread(tool, window, cx)))
                    .into_any_element(),
            );
            out.extend(threads);
        }
        out
    }

    /// The Files group: FTP, FTPS and SFTP hosts (from Transmit and the agent's store); a click
    /// opens the host's thread, its files on top. Folded at first: there are many.
    fn render_files_group(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let (head, collapsed) = self.subject_head(
            "files-head",
            Ico::FolderOpen,
            "Files",
            // "+": a new FTP / FTPS / SFTP host (its form takes the chat area).
            Some(Box::new(|this: &mut Self, window, cx| this.new_host_form(window, cx))),
            cx,
        );
        let mut out = vec![head];
        if collapsed {
            return out;
        }
        let current = self.current_subject();
        let hosts = self.files_remote.hosts.clone();
        let n = hosts.len();
        for (k, h) in hosts.into_iter().enumerate() {
            let key = format!("files:{}", h.id);
            let selected = self.panel == Panel::Chat && current == key && self.session.is_none();
            let note = if h.user.is_empty() { h.host.clone() } else { format!("{}@{}", h.user, h.host) };
            let id = h.id.clone();
            let threads = self.thread_rows(&key, k + 1 == n, cx);
            out.push(
                self.subject_row(format!("fh-{k}").into(), h.name, note, selected, k + 1 == n && threads.is_empty(), Some(key), cx)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_files_thread(id.clone(), window, cx)
                    }))
                    .into_any_element(),
            );
            out.extend(threads);
        }
        out
    }

    /// Browser: sessions that work in the server's browser, step by step; "+" starts one.
    fn render_sites_group(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let (head, collapsed) = self.subject_head(
            "browser-head",
            Ico::Globe,
            "Browser",
            Some(Box::new(|this: &mut Self, _, cx| this.new_browser_thread(cx))),
            cx,
        );
        let mut out = vec![head];
        if collapsed {
            return out;
        }
        let rows: Vec<(String, String, u64)> = self
            .sessions
            .iter()
            .filter(|s| s.subject.starts_with("browser:"))
            .map(|s| (s.id.clone(), if s.title.is_empty() { "Browser session".into() } else { s.title.clone() }, s.messages))
            .collect();
        let n = rows.len();
        for (k, (id, title, messages)) in rows.into_iter().enumerate() {
            let selected = self.panel == Panel::Chat && self.session.as_deref() == Some(&id);
            let note = format!("{messages} messages");
            out.push(
                self.subject_row(format!("browser-{k}").into(), title, note, selected, k + 1 == n, None, cx)
                    .on_click(cx.listener(move |this, _, _, cx| this.open_browser_thread(id.clone(), cx)))
                    .into_any_element(),
            );
        }
        out
    }
}

/// A row's "+" (a new thread about it): over the row's right end on hover, in the column of the
/// group heads' "+" (16 from the edge), so the row's count or note keeps its place.
fn row_plus(id: SharedString, group: SharedString, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    // Seen only on a hovered row: its hover color, covering what is under it.
    let (muted, fg, bg) = (theme.muted_foreground, theme.foreground, theme.sidebar_accent);
    div()
        .id(id)
        .absolute()
        .top_0()
        .bottom_0()
        .right(px(16.))
        .flex()
        .items_center()
        .opacity(0.)
        .group_hover(group, |s| s.opacity(1.))
        .cursor_pointer()
        .child(
            super::head_plus_slot()
                .bg(bg)
                .text_color(muted)
                .hover(move |s| s.text_color(fg))
                .child(Ico::Plus.icon().xsmall()),
        )
        .tooltip(|window, cx| Tooltip::new("New thread").build(window, cx))
}
