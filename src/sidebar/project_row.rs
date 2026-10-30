//! A project in the sidebar: its head (the name, its Telegram and schedule marks, "⋯" on hover —
//! its Settings among its items —, the fold, then "+" for a new thread in it, always shown and
//! rightmost, as Claude's), then
//! its threads on the head's guide. Its files and environment are the Code view's (the switch
//! over the window).

use gpui_kit::component::input::Input;
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use bench_ui::Gap;

use super::menu::item_menu;
use super::order::DraggedProject;
use crate::app::BenchApp;
use crate::sidebar::items::Item;
use bench_ui::icons::Ico;

/// What a group's head shows: Host's and every project's alike, drawn by one function, so
/// nothing is on one head and missing from another.
pub(super) struct GroupHead {
    /// Its place in the sidebar (0 is Host): the ids of its controls.
    pub g: usize,
    pub name: SharedString,
    /// The project's id: its menu, its mark, its rename and remove.
    pub id: String,
    /// The project a "+" makes a new thread in ("" is Host's).
    pub new_in: String,
    pub telegram: bool,
    /// Its schedule has a job that is on.
    pub cron: bool,
    /// The key `toggle_group` flips, and whether the group is folded now.
    pub fold_key: String,
    pub collapsed: bool,
}

impl BenchApp {
    /// Host: the machine's own agent and environment, for looking after the machine itself and
    /// the threads of no project; first in the sidebar, folded until opened (the key marks it
    /// open). A group as every project's.
    pub(super) fn render_host(&mut self, rows: Vec<usize>, cx: &mut Context<Self>) -> AnyElement {
        let fold_key = "open:loose".to_string();
        let head = GroupHead {
            g: 0,
            name: crate::app::LOCAL_HOST.into(),
            id: crate::app::model::ProjectKey::HOST.into(),
            new_in: String::new(),
            telegram: self.places.host_bots.contains(""),
            cron: self.places.host_crons.contains(""),
            collapsed: !self.sidebar.collapsed.contains(&fold_key),
            fold_key,
        };
        self.render_group(head, None, rows, cx)
    }

    /// A project: its head and its threads on the head's guide (the controls of the head are in
    /// `menu.rs`'s notes: hover "⋯", "+", and the fold always shown).
    pub(super) fn render_project(
        &mut self,
        g: usize,
        p: &crate::app::model::ProjectEntry,
        rows: Vec<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let fold_key = format!("p:{}", p.id);
        // A remote project's agent: connecting, or why it is not there.
        let note = Some(&p.agent)
            .filter(|h| !h.is_empty())
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
        let head = GroupHead {
            g,
            name: p.name.clone().into(),
            id: p.id.clone(),
            new_in: p.id.clone(),
            telegram: p.telegram,
            cron: p.cron,
            collapsed: self.sidebar.collapsed.contains(&fold_key),
            fold_key,
        };
        self.render_group(head, note, rows, cx)
    }

    /// A group: its head, then (unless folded) a note on its agent and its threads. The whole
    /// group is the place a dragged project lands against — above it or below it, never between
    /// a head and its threads, which are one project — so the line says so over the group, and
    /// the group is `relative` to hold that line where it belongs.
    fn render_group(
        &mut self,
        head: GroupHead,
        note: Option<String>,
        rows: Vec<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let collapsed = head.collapsed;
        let g = head.g;
        let id = head.id.clone();
        // The line over a group says where the project in the air will land, and it shows only
        // while GPUI says a drag is live: a target left from a drag let go elsewhere cannot keep a
        // line on screen.
        let edge = cx
            .has_active_drag()
            .then(|| self.drop_edge_over(&id))
            .flatten();
        let mut parts: Vec<AnyElement> = vec![self.render_group_head(head, cx)];
        if !collapsed {
            if let Some(note) = note {
                parts.push(
                    div()
                        .pl(px(bench_ui::SIDEBAR_TEXT_X))
                        .pr(Gap::Xl)
                        .py(Gap::Xs)
                        // The note belongs to the head: the heading's own size.
                        .text_xs()
                        .text_color(muted)
                        .truncate()
                        .child(note)
                        .into_any_element(),
                );
            }
            for i in rows {
                parts.push(self.render_thread_row(i, cx).into_any_element());
            }
        }
        let onto = id;
        let zone = v_flex()
            .id(("pgroup", g))
            .relative()
            .children(parts)
            .when_some(edge, |d, edge| {
                d.child(
                    bench_ui::drop_line(edge, cx)
                        .left(px(bench_ui::SIDEBAR_ROW_INSET))
                        .right(px(bench_ui::SIDEBAR_ROW_INSET)),
                )
            });
        // Every group takes the project in the air (`order.rs`).
        self.drop_zone(zone, onto, cx).into_any_element()
    }

    /// A group's head: the name, its tag dot and Telegram mark, then on hover "⋯" (its menu,
    /// Settings among its items) and "+", and the fold.
    fn render_group_head(&mut self, head: GroupHead, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let GroupHead { g, name, id, new_in, telegram, cron, fold_key, collapsed } = head;
        let group: SharedString = format!("phead{g}").into();
        let mark = self.mark_of(&format!("p:{id}"));
        // A chosen color is a tag's dot after the name (as Finder's); the name stays the text's.
        let color: Option<Hsla> = mark.color.and_then(bench_ui::mark_color);
        let armed = self.armed.is(&format!("p:{id}"));
        let renaming = self.renaming_field(&Item::Project(id.clone()));
        let tag = color.filter(|_| renaming.is_none());
        // A project's head goes in the air: its group comes down where the person leaves it, so
        // the two they work in together stand under each other (`order.rs`). The Host is always
        // first, so it never lifts; a name being typed selects, not drags.
        let drag = (g != 0 && renaming.is_none()).then(|| DraggedProject {
            id: id.clone(),
            name: name.clone(),
            color,
        });
        h_flex()
            .id(("phead", g))
            .group(group.clone())
            .cursor_pointer()
            .when_some(drag, |d, drag| {
                d.cursor(CursorStyle::OpenHand)
                    .on_drag(drag, |d, _, _, cx| cx.new(|_| d.clone()))
            })
            .pl(px(bench_ui::SIDEBAR_TEXT_X))
            .pr(px(bench_ui::SIDEBAR_ROW_INSET))
            .pt(if g == 0 {
                AbsoluteLength::from(rems(0.5))
            } else {
                AbsoluteLength::from(px(bench_ui::GROUP_GAP))
            })
            // One pixel more breath under a heading than a plain pb_1: projects are
            // sections, and the eye wants the groups further apart than the threads inside.
            .pb(px(bench_ui::GROUP_HEAD_PB))
            .gap(Gap::S)
            .items_center()
            // A section's header, as a Mac's sidebar has them: small, semibold, secondary.
            .text_size(px(bench_ui::SIDEBAR_HEAD_TEXT))
            .child(match renaming {
                Some(field) => div()
                    .flex_1()
                    .min_w_0()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(Input::new(&field).xsmall())
                    .into_any_element(),
                // The name in the secondary colour; a chosen color is its tag dot.
                None => div()
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(muted)
                    .child(name)
                    .into_any_element(),
            })
            .children(tag.map(bench_ui::tag_dot))
            // A bot of its own: its threads are written to from Telegram too.
            .when(telegram, |d| d.child(telegram_mark(("ptg", g), muted)))
            // A schedule: threads start in it by themselves.
            .when(cron, |d| d.child(cron_mark(("pcron", g), muted)))
            .child(div().flex_1())
            // Removing asks first: its environment goes with it.
            .when(armed, |d| {
                let yes = id.clone();
                d.child(bench_ui::confirm_buttons(
                    format!("premove-{g}"),
                    "Remove",
                    Box::new(cx.listener(move |this, _, window, cx| {
                        this.delete_project(yes.clone(), window, cx)
                    })),
                    Box::new(cx.listener(|this, _, _, cx| this.cancel_delete(cx))),
                    cx,
                ))
            })
            // "⋯" (on hover): the same menu as a right click.
            .child(
                bench_ui::on_group_hover(group.clone()).child(
                    bench_ui::row_icon_button(("pmore", g), Ico::Ellipsis).dropdown_menu(
                        item_menu(cx.entity(), Item::Project(id.clone()), false),
                    ),
                ),
            )
            // The fold, before "+": shown when folded, on hover when open (Finder's); "+" is the
            // rightmost, so it stands at the same x on every head.
            .child(bench_ui::group_chevron(collapsed, group.clone(), cx))
            .child(
                bench_ui::head_plus_slot()
                    .id(("pnew", g))
                    .rounded_sm()
                    .cursor_pointer()
                    .text_color(muted)
                    // Always shown, as Claude's: every project says where its new thread goes
                    // without a hover to find it.
                    .hover(|s| s.bg(theme.sidebar_accent).text_color(theme.foreground))
                    .child(Ico::Plus.icon().small())
                    .tooltip(|window, cx| {
                        Tooltip::new("New thread in this project  ⌘N").build(window, cx)
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.new_thread_in(new_in.clone(), cx)
                    })),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_group(fold_key.clone(), cx)))
            .context_menu(item_menu(cx.entity(), Item::Project(id), false))
            .into_any_element()
    }
}

/// A group head's sign that its project's schedule has a job on: threads start in it by
/// themselves. Beside the Telegram mark, the same quiet glyph.
pub(super) fn cron_mark(id: impl Into<ElementId>, muted: Hsla) -> impl IntoElement {
    div()
        .id(id)
        .flex_none()
        .text_color(muted.opacity(0.7))
        .child(Ico::Clock.icon().with_size(px(bench_ui::HEAD_MARK_ICON)))
        .tooltip(|window, cx| Tooltip::new("It runs on a schedule").build(window, cx))
}

/// A group head's sign that its project has a Telegram bot: its threads are written to from
/// Telegram too.
pub(super) fn telegram_mark(id: impl Into<ElementId>, muted: Hsla) -> impl IntoElement {
    div()
        .id(id)
        .flex_none()
        .text_color(muted.opacity(0.7))
        .child(Ico::Send.icon().with_size(px(bench_ui::HEAD_MARK_ICON)))
        .tooltip(|window, cx| Tooltip::new("Its Telegram bot is on").build(window, cx))
}
