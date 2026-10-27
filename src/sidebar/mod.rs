//! Left column: where, which profile, navigation, sessions (grouped by source), status bar.

pub(crate) mod items;
pub(crate) mod marks;
mod project_list;
mod telegram;
mod top;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use bench_ui::plural;
use crate::sidebar::items::Item;
use crate::app::BenchApp;
use crate::workspace::Center;
use bench_ui::icons::Ico;

/// A session title: the sidebar's 14px.
pub(crate) const SESSION_TITLE: f32 = 14.;

impl BenchApp {
    pub(crate) fn render_sidebar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        // The left dock's other panel: the Explorer's tree (`src/explorer/tree.rs`).
        if self.sidebar_panel == crate::workspace::SidebarPanel::Explorer {
            let rows = self.render_explorer(window, cx);
            return v_flex()
                .size_full()
                .bg(theme.sidebar)
                .child(self.render_sidebar_top(cx))
                .child(
                    v_flex()
                        .id("explorer")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .pb_1()
                        .children(rows),
                )
                .into_any_element();
        }
        let mut list: Vec<AnyElement> = vec![];
        let searching = !self.search_query.is_empty();
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
        } else {
            // Projects, each with its threads: the sidebar's only unit. Host first, always: this
            // machine's own project, with the threads of no project, Telegram's and cron's; then
            // the projects, pinned ones first (`sidebar_items.rs` — ↑ ↓ walk the same order).
            let (loose, groups) = self.sidebar_groups();
            list.extend(self.render_loose(loose, cx));
            for (g, (p, rows)) in groups.into_iter().enumerate() {
                list.extend(self.render_project(g + 1, &p, rows, cx));
            }
            if self.projects.is_empty() && !self.connecting {
                list.push(self.sidebar_note("No projects yet. “+” above starts one.", cx));
            }
        }

        v_flex()
            .size_full()
            .bg(theme.sidebar)
            // Under the window buttons: the search as a band, then the groups as an IDE's tree
            // (a new chat: "+" on a group, or in the top strip).
            .child(self.render_sidebar_top(cx))
            .child(bench_ui::search_band(&self.search, cx))
            .child(
                v_flex()
                    .id("sessions")
                    // After a click on a thread: ↑ ↓ open the next, ⌫ deletes (`sidebar_items.rs`).
                    .track_focus(&self.sidebar_focus)
                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                        this.sidebar_key(&ev.keystroke, window, cx)
                    }))
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

    /// A project: its head (an icon for what it is, the name, the fold, "+" for a new thread in
    /// it), then its threads on the head's guide.
    fn render_project(
        &mut self,
        g: usize,
        p: &crate::app::model::ProjectEntry,
        rows: Vec<usize>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let key = format!("p:{}", p.id);
        let collapsed = self.ssh.collapsed.contains(&key);
        let group: SharedString = format!("phead{g}").into();
        let remote = !p.agent.is_empty();
        let icon = match (remote, &p.folder) {
            // A server's Host is the server; a project on it is a folder there, or none.
            (true, _) if p.is_host() => Ico::Server,
            (true, Some(_)) => Ico::Folder,
            (_, Some(f)) if std::path::Path::new(f).join(".git").exists() => Ico::FolderGit,
            (_, Some(_)) => Ico::Folder,
            _ => Ico::Layers,
        };
        // A remote project's agent: connecting, or why it is not there.
        let remote_note =
            Some(&p.agent)
                .filter(|_| remote)
                .and_then(|h| match self.agents.get(h) {
                    Some(r) if r.conn.is_some() => None,
                    Some(r) => Some(
                        r.error
                            .clone()
                            .map(|e| format!("{h}: {e}"))
                            .unwrap_or_else(|| format!("Connecting to {h}…")),
                    ),
                    None => Some(format!("{h}: not connected")),
                });
        let (new_id, menu_id) = (p.id.clone(), p.id.clone());
        let mark = self.mark_of(&format!("p:{}", p.id));
        let pinned = mark.pinned;
        // A chosen color is the project's icon and name, no background: it marks the group.
        let color: Option<Hsla> = mark
            .color
            .and_then(|c| crate::sidebar::marks::MARK_COLORS.get(c))
            .map(|c| rgb(*c).into());
        let armed = self.confirm_delete.as_deref() == Some(format!("p:{}", p.id).as_str());
        let renaming = self.renaming_field(&Item::Project(p.id.clone()));
        let zed = p
            .folder
            .clone()
            .filter(|f| !f.is_empty())
            // Zed opens it where its files are: this Mac, or that machine over SSH.
            .map(|f| {
                (
                    f,
                    Some(p.location().machine_here()).filter(|m| !m.is_empty()),
                )
            });
        let mut out = vec![
            h_flex()
                .id(("phead", g))
                .group(group.clone())
                .cursor_pointer()
                .px_4()
                .pt(rems(if g == 0 { 0.5 } else { 0.75 }))
                .pb_1()
                .gap_1p5()
                .items_center()
                .text_sm()
                .child(bench_ui::head_icon_in(icon, color, cx))
                .child(match renaming {
                    Some(field) => div()
                        .flex_1()
                        .min_w_0()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(Input::new(&field).xsmall())
                        .into_any_element(),
                    None => div()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(color.unwrap_or(muted))
                        .child(p.name.clone())
                        .into_any_element(),
                })
                .child(div().flex_1())
                // Removing asks first: its environment goes with it.
                .when(armed, |d| {
                    let yes = p.id.clone();
                    d.child(
                        h_flex()
                            .gap_1()
                            .text_xs()
                            .child(
                                div()
                                    .id(("premove-yes", g))
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_md()
                                    .bg(theme.danger)
                                    .text_color(theme.danger_foreground)
                                    .child("Remove")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        cx.stop_propagation();
                                        this.delete_project(yes.clone(), window, cx)
                                    })),
                            )
                            .child(
                                div()
                                    .id(("premove-no", g))
                                    .px_1()
                                    .py_0p5()
                                    .text_color(muted)
                                    .hover(|s| s.text_color(theme.foreground))
                                    .child("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.cancel_delete(cx)
                                    })),
                            ),
                    )
                })
                // "⋯" (on hover): the same menu as a right click.
                .child(
                    div()
                        .flex_none()
                        .opacity(0.)
                        .group_hover(group.clone(), |s| s.opacity(1.))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            Button::new(("pmore", g))
                                .ghost()
                                .xsmall()
                                .icon(Ico::Ellipsis.icon())
                                .dropdown_menu(item_menu(
                                    cx.entity(),
                                    Item::Project(menu_id.clone()),
                                    pinned,
                                    zed.clone(),
                                )),
                        ),
                )
                .child(
                    bench_ui::head_plus_slot()
                        .id(("pnew", g))
                        .cursor_pointer()
                        .text_color(muted)
                        // On hover, as the pencil.
                        .opacity(0.)
                        .group_hover(group.clone(), |s| s.opacity(1.))
                        .hover(|s| s.bg(theme.sidebar_accent).text_color(theme.foreground))
                        .child(Ico::Plus.icon().xsmall())
                        .tooltip(|window, cx| {
                            Tooltip::new("New thread in this project").build(window, cx)
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.new_thread_in(new_id.clone(), cx)
                        })),
                )
                // The fold, always the rightmost and always shown (open: down, folded: right).
                .child(
                    div().flex_none().p_0p5().text_color(muted).child(
                        if collapsed {
                            Ico::ChevronRight
                        } else {
                            Ico::ChevronDown
                        }
                        .icon()
                        .xsmall(),
                    ),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.toggle_group(key.clone(), cx)))
                .context_menu(item_menu(
                    cx.entity(),
                    Item::Project(menu_id.clone()),
                    pinned,
                    zed.clone(),
                ))
                .into_any_element(),
        ];
        if collapsed {
            return out;
        }
        if let Some(note) = remote_note {
            out.push(
                div()
                    .pl(bench_ui::nest_pl(cx))
                    .pr_4()
                    .py_1()
                    .text_sm()
                    .text_color(muted)
                    .truncate()
                    .child(note)
                    .into_any_element(),
            );
        }
        let n = rows.len();
        for (k, i) in rows.into_iter().enumerate() {
            out.push(
                self.render_session_row(i, Some(k + 1 == n), cx)
                    .into_any_element(),
            );
        }
        out
    }

    /// Host: the machine's own agent and environment, for looking after the machine itself (what every project's threads get too) and
    /// the threads of no project; first in the sidebar, folded until opened.
    fn render_loose(&mut self, rows: Vec<usize>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let muted = cx.theme().muted_foreground;
        let host_color: Option<Hsla> = self
            .mark_of(&format!("p:{}", crate::app::model::ProjectKey::HOST))
            .color
            .and_then(|c| crate::sidebar::marks::MARK_COLORS.get(c))
            .map(|c| rgb(*c).into());
        let key = "open:loose".to_string();
        let open = self.ssh.collapsed.contains(&key);
        let group: SharedString = "loose-head".into();
        let mut out = vec![
            h_flex()
                .id("loose-head")
                .group(group.clone())
                .cursor_pointer()
                .px_4()
                .pt(rems(0.5))
                .pb_1()
                .gap_1p5()
                .items_center()
                .text_sm()
                .child(bench_ui::head_icon_in(Ico::Server, host_color, cx))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(host_color.unwrap_or(muted))
                        .child(crate::app::LOCAL_HOST),
                )
                .child(div().flex_1())
                // "⋯" (on hover), as a right click: its environment, its color — the same menu as
                // a project's, without pin and remove (it is always first, and always there).
                .child(
                    div()
                        .flex_none()
                        .opacity(0.)
                        .group_hover(group.clone(), |s| s.opacity(1.))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            Button::new("general-more")
                                .ghost()
                                .xsmall()
                                .icon(Ico::Ellipsis.icon())
                                .dropdown_menu(item_menu(
                                    cx.entity(),
                                    Item::Project(crate::app::model::ProjectKey::HOST.into()),
                                    false,
                                    None,
                                )),
                        ),
                )
                .child(
                    bench_ui::head_plus_slot()
                        .id("general-new")
                        .cursor_pointer()
                        .text_color(muted)
                        .opacity(0.)
                        .group_hover(group.clone(), |s| s.opacity(1.))
                        .hover(|s| {
                            s.bg(cx.theme().sidebar_accent)
                                .text_color(cx.theme().foreground)
                        })
                        .child(Ico::Plus.icon().xsmall())
                        .tooltip(|window, cx| Tooltip::new("New thread").build(window, cx))
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.new_thread_in(String::new(), cx)
                        })),
                )
                .child(bench_ui::group_chevron(!open, group, cx))
                // Folded by default: the key marks it open.
                .on_click(cx.listener(move |this, _, _, cx| this.toggle_group(key.clone(), cx)))
                .context_menu(item_menu(
                    cx.entity(),
                    Item::Project(crate::app::model::ProjectKey::HOST.into()),
                    false,
                    None,
                ))
                .into_any_element(),
        ];
        if !open {
            return out;
        }
        let n = rows.len();
        for (k, i) in rows.into_iter().enumerate() {
            out.push(
                self.render_session_row(i, Some(k + 1 == n), cx)
                    .into_any_element(),
            );
        }
        out
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
        let selected = self.center == Center::Chat && self.session.as_deref() == Some(id);
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
        let selected = self.center == Center::Chat && self.session.as_deref() == Some(&s.id);
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
        // Deleting asks once (⌫, or the menu): the row shows Delete · Cancel; ⌫ again deletes.
        let armed = self.confirm_delete.as_deref() == Some(s.id.as_str());
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
                        .rounded_md()
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
                        .dropdown_menu(item_menu(
                            view.clone(),
                            Item::Thread(id.clone()),
                            mark.pinned,
                            None,
                        )),
                )
                .into_any_element()
        };
        // A chosen color marks the row with a quiet stripe (the open chat keeps the accent).
        let working =
            s.running || (self.live.is_some() && self.session.as_deref() == Some(s.id.as_str()));
        let stripe = self
            .mark_of(&s.id)
            .color
            .and_then(|c| crate::sidebar::marks::MARK_COLORS.get(c))
            .map(|c| -> Hsla { rgb(*c).into() });

        let pinned = self.mark_of(&s.id).pinned;
        let menu_id = s.id.clone();
        h_flex()
            .id(("session", i))
            .group(group)
            .relative()
            .gap_1()
            .pl(if guide.is_some() {
                bench_ui::nest_pl(cx)
            } else {
                px(14.)
            })
            .when_some(guide, |d, last| d.child(bench_ui::nest_guide(cx, last)))
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
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .when(pinned, |d| {
                                d.child(Ico::Pin.icon().xsmall().text_color(muted))
                            })
                            .child(match self.renaming_field(&Item::Thread(s.id.clone())) {
                                // Renamed in place: the field where the title was.
                                // Over the title, in its place and size: the title stays (unseen) and
                                // keeps the row's height, so nothing moves.
                                Some(field) => div()
                                    .relative()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(SESSION_TITLE))
                                    .child(
                                        div()
                                            .truncate()
                                            .text_color(gpui_kit::transparent_black())
                                            .child(s.title.clone()),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .inset_0()
                                            .flex()
                                            .items_center()
                                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                cx.stop_propagation()
                                            })
                                            .child(
                                                bench_ui::bare_field(&field)
                                                    // Input draws at 0.875 of its size: the title's.
                                                    .with_size(gpui_kit::component::Size::Size(px(
                                                        SESSION_TITLE / 0.875,
                                                    )))
                                                    .px_0(),
                                            ),
                                    )
                                    .into_any_element(),
                                None => div()
                                    .min_w_0()
                                    .text_size(px(SESSION_TITLE))
                                    .truncate()
                                    .map(|d| {
                                        if working {
                                            d.child(ShimmerText::new(s.title.clone()))
                                        } else {
                                            d.child(s.title.clone())
                                        }
                                    })
                                    .into_any_element(),
                            }),
                    )
                    // Compact: the title alone (a working session still shimmers).
                    .when(!compact, |d| {
                        d.child(div().text_xs().text_color(muted).child(meta))
                    }),
            )
            .child(delete)
            // Opened, and the sidebar takes the keys: ↑ ↓ to the next, ⌫ deletes it.
            .on_click(cx.listener(move |this, _, window, cx| {
                this.open_session(open_id.clone(), cx);
                window.focus(&this.sidebar_focus, cx);
            }))
            .context_menu(item_menu(cx.entity(), Item::Thread(menu_id), pinned, None))
    }
}

/// The swatches row of a session's menu: the colors and "none".
fn color_row(view: &Entity<BenchApp>, id: &str, current: Option<usize>, cx: &App) -> AnyElement {
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
            .rounded_md()
            .border_color(if on {
                theme.foreground
            } else {
                theme.transparent
            })
            .map(|d| match k {
                Some(c) => d.child(
                    div()
                        .size(px(12.))
                        .bg(Hsla::from(rgb(crate::sidebar::marks::MARK_COLORS[c]))),
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
    // The menu reserves a 16px icon column on the left (the other items have icons), which
    // pushes this row off-centre; pull it back by that width so the swatches sit symmetric.
    h_flex()
        .flex_1()
        .justify_center()
        .gap_2()
        .ml(px(-16.))
        .py_1()
        .children((0..crate::sidebar::marks::MARK_COLORS.len()).map(|c| swatch(Some(c))))
        .child(swatch(None))
        .into_any_element()
}

/// The menu of a sidebar item (its "⋯" and a right click), the same for a thread and a
/// project: a project's own entries (environment, Zed), then pin, color, rename, delete.
fn item_menu(
    view: Entity<BenchApp>,
    item: Item,
    pinned: bool,
    zed: Option<(String, Option<String>)>,
) -> impl Fn(
    gpui_kit::component::menu::PopupMenu,
    &mut Window,
    &mut Context<gpui_kit::component::menu::PopupMenu>,
) -> gpui_kit::component::menu::PopupMenu
+ 'static {
    move |menu, _, _| {
        let act = |f: Box<dyn Fn(&mut BenchApp, &mut Window, &mut Context<BenchApp>)>| {
            let view = view.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                view.update(cx, |this, cx| f(this, window, cx))
            }
        };
        let key = item.mark_key();
        let (pin_key, swatch_view, swatch_key) = (key.clone(), view.clone(), key.clone());
        let (rename, del) = (item.clone(), item.clone());
        let menu = match &item {
            Item::Project(id) => {
                let env_id = id.clone();
                menu.item(
                    PopupMenuItem::new("Settings")
                        .icon(Ico::Settings.icon())
                        .on_click(act(Box::new(move |this, window, cx| {
                            this.open_project_settings(env_id.clone(), window, cx)
                        }))),
                )
                // Its files, prompts, skills and tools in the Explorer, here.
                .item({
                    let id = id.clone();
                    PopupMenuItem::new("Explorer")
                        .icon(Ico::FolderTree.icon())
                        .on_click(act(Box::new(move |this, window, cx| {
                            this.show_explorer(id.clone(), window, cx)
                        })))
                })
                // A project with a folder opens in Zed (a server's over SSH).
                .when_some(zed.clone(), |menu, (folder, host)| {
                    menu.item(
                        PopupMenuItem::new("Open in Zed")
                            .icon(Ico::Zed.icon())
                            .on_click(act(Box::new(move |this, _, cx| {
                                this.open_in_zed(&folder, None, host.as_deref(), cx)
                            }))),
                    )
                })
                .separator()
            }
            Item::Thread(_) => menu,
        };
        // This Mac's Host is always first: no pin. An agent's Host is never removed (a server's
        // agent leaves Bench in the gear).
        let key_of = |i: &Item| match i {
            Item::Project(id) => Some(crate::app::model::ProjectKey::parse(id)),
            Item::Thread(_) => None,
        };
        let host = key_of(&item).is_some_and(|k| k.is_host());
        let pinnable = !key_of(&item).is_some_and(|k| k.is_host() && k.agent.is_empty());
        menu.when(pinnable, |menu| {
            menu.item(
                PopupMenuItem::new(if pinned { "Unpin" } else { "Pin" })
                    .icon(Ico::Pin.icon())
                    .on_click(act(Box::new(move |this, _, cx| {
                        this.toggle_pin(pin_key.clone(), cx)
                    }))),
            )
        })
        // A thread is renamed here; a project in its environment (its Name).
        .when(matches!(rename, Item::Thread(_)), |menu| {
            menu.item(
                PopupMenuItem::new("Rename")
                    .icon(Ico::Pencil.icon())
                    .on_click(act(Box::new(move |this, window, cx| {
                        this.start_rename(rename.clone(), window, cx)
                    }))),
            )
        })
        .separator()
        // Reads the color live, so a click shows at once in the open menu.
        .item(PopupMenuItem::element(move |_, cx| {
            let current = swatch_view.read(cx).mark_of(&swatch_key).color;
            color_row(&swatch_view, &swatch_key, current, cx)
        }))
        .when(!host, |menu| {
            menu.separator().item(
                PopupMenuItem::new(match del {
                    Item::Thread(_) => "Delete…",
                    Item::Project(_) => "Remove project…",
                })
                .icon(Ico::Trash.icon())
                .on_click(act(Box::new(move |this, window, cx| match &del {
                    Item::Thread(id) => this.arm_delete(id.clone(), cx),
                    Item::Project(id) => this.delete_project(id.clone(), window, cx),
                }))),
            )
        })
    }
}
