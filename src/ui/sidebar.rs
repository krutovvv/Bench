//! Left column: where, which profile, navigation, sessions (grouped by source), status bar.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::plural;
use crate::app::{HermesApp, Panel};
use crate::icons::Ico;
use crate::model::source_label;

impl HermesApp {
    pub(super) fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;

        // Group visible sessions: by git project when they work in one, else by where they live
        // (Hermes UI/CLI, Telegram, cron…). Plain Hermes chats first, then projects, then the rest.
        let mut groups: Vec<(String, Vec<usize>)> = vec![];
        let mut rank: Vec<(String, u8)> = vec![];
        // A project/folder group's place on disk and whether it is a git repo (for its "+").
        let mut homes: Vec<(String, String, bool)> = vec![];
        for i in self.visible_sessions() {
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
                    let r = if l == "Hermes" { 1 } else { 3 };
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
                list.push(
                    h_flex()
                        .group(group.clone())
                        .px_4()
                        .pt_4()
                        .pb_1()
                        .items_center()
                        .text_xs()
                        .text_color(muted)
                        .child(div().flex_1().child(format!(
                            "{} · {}",
                            label.to_uppercase(),
                            rows.len()
                        )))
                        // A project's "+": a new chat already in it (in a branch of its own).
                        .children(home.map(|(_, path, repo)| {
                            div()
                                .id(("group-new", g))
                                .p_0p5()
                                .cursor_pointer()
                                .opacity(0.)
                                .group_hover(group.clone(), |s| s.opacity(1.))
                                .hover(|s| s.bg(theme.sidebar_accent).text_color(theme.foreground))
                                .child(Ico::Plus.icon().xsmall())
                                .tooltip(move |window, cx| {
                                    gpui_kit::component::tooltip::Tooltip::new(if repo {
                                        "New chat in this project"
                                    } else {
                                        "New chat in this folder"
                                    })
                                    .build(window, cx)
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.new_chat_in(Some(path.clone()), repo, cx);
                                    this.new_project_repo = repo;
                                }))
                        }))
                        .into_any_element(),
                );
            }
            for i in rows {
                list.push(self.render_session_row(i, cx).into_any_element());
            }
        }

        let new_session = h_flex()
            .id("new-session")
            .h(rems(super::BAR_CONTROL_REMS))
            .px_3()
            .gap_2()
            .items_center()
            .justify_center()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .bg(theme.primary)
            .text_color(theme.primary_foreground)
            .cursor_pointer()
            .hover(|s| s.bg(theme.primary_hover))
            .child(Ico::Plus.icon().small())
            .child("New session")
            .on_click(cx.listener(|this, _, _, cx| this.new_session(cx)));

        v_flex()
            .size_full()
            .bg(theme.sidebar)
            .child(
                // Right under the window buttons (no padding of its own above), 8px below like the
                // footer's, then the rule.
                div().pt(px(38.)).flex_none().child(
                    h_flex()
                        .px_4()
                        .pb_2()
                        .items_center()
                        .border_b_1()
                        .border_color(theme.sidebar_border)
                        .child(
                            // Medium input = 2 rem, the same as the button below.
                            Input::new(&self.search)
                                .cleanable(true)
                                .prefix(Ico::Search.icon().small().text_color(muted)),
                        ),
                ),
            )
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
            .child(
                // Same height as the composer, so the two top rules line up across the window.
                h_flex()
                    .flex_none()
                    .h(super::bar_h(cx))
                    .px_4()
                    .items_center()
                    .border_t_1()
                    .border_color(theme.sidebar_border)
                    .child(new_session.flex_1()),
            )
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
            .child(div().text_sm().truncate().child(title))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .line_clamp(2)
                    .child(snippet.replace(">>>", "").replace("<<<", "")),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.open_session(open_id.clone(), cx)))
    }

    fn render_session_row(&self, i: usize, cx: &mut Context<Self>) -> impl IntoElement {
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
            // "⋯" on hover: pin, color, a window of its own, delete.
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
                            let (pin_id, win_id, del) = (id.clone(), id.clone(), del_id.clone());
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
                                PopupMenuItem::new("Open in new window")
                                    .icon(Ico::AppWindow.icon())
                                    .on_click(act(Box::new(move |this, _, cx| {
                                        this.open_window(Some(win_id.clone()), cx)
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
        let stripe = self
            .mark_of(&s.id)
            .color
            .and_then(|c| crate::app::marks::SESSION_COLORS.get(c))
            .map(|c| -> Hsla { rgb(*c).into() });

        h_flex()
            .id(("session", i))
            .group(group)
            .gap_1()
            .pl(px(14.))
            .pr_3()
            .py_2()
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
            .when(selected && stripe.is_none(), |d| {
                d.bg(theme.sidebar_accent)
                    .text_color(theme.sidebar_accent_foreground)
            })
            .hover(|s| s.bg(theme.sidebar_accent))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_sm().truncate().child(s.title.clone()))
                    .child(div().text_xs().text_color(muted).child(meta)),
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
