//! The order of the sidebar's projects: the person makes it by dragging a project's group where
//! they want it — working in two projects at once keeps them under each other, near the top.
//! Kept on this Mac (`project-order.json`) as ids in order; a project not in it yet — a new one —
//! keeps the place its source gave and sits after the arranged ones; one whose agent has not
//! answered keeps its place for when it does.

use gpui_kit::component::{ActiveTheme as _};
use gpui_kit::*;
use bench_ui::Gap;

use crate::app::BenchApp;
use crate::app::model::ProjectKey;
use crate::client::connection;

/// A project held in the air: the row that follows the cursor while its group is dragged, the
/// head's own look in a card (as a dragged document tab is that tab in a card).
#[derive(Clone)]
pub struct DraggedProject {
    pub id: String,
    pub name: SharedString,
    /// The project's chosen colour: its tag dot, as on the head.
    pub color: Option<Hsla>,
}

impl Render for DraggedProject {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        bench_ui::drag_card(cx)
            .gap(Gap::S)
            .pl(px(bench_ui::SIDEBAR_TEXT_X))
            .pr(px(bench_ui::SIDEBAR_ROW_INSET))
            .py(px(bench_ui::GROUP_HEAD_PB))
            .text_size(px(bench_ui::SIDEBAR_HEAD_TEXT))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.muted_foreground)
            .child(self.name.clone())
            .children(self.color.map(bench_ui::tag_dot))
    }
}

fn path() -> std::path::PathBuf {
    connection::state_dir().join("project-order.json")
}

pub(crate) fn load() -> Vec<String> {
    std::fs::read(path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

impl BenchApp {
    /// Where a project sits in the person's order: those never arranged share the last place, and
    /// the sort that uses this is stable, so they keep the order their sources listed them in.
    pub fn project_rank(&self, id: &str) -> usize {
        self.sidebar
            .order
            .iter()
            .position(|o| o == id)
            .unwrap_or(usize::MAX)
    }

    /// The pointer is over the group `id`: the project in the air lands at the edge of it the
    /// pointer is nearest. Only said again when it changed — every move would redraw the list.
    pub fn drag_over_project(
        &mut self,
        id: &str,
        edge: bench_ui::DropEdge,
        cx: &mut Context<Self>,
    ) {
        let at = Some((id.to_string(), edge));
        if self.sidebar.drop != at {
            self.sidebar.drop = at;
            cx.notify();
        }
    }

    /// The edge of the group `id` the project in the air is over, if it is over it.
    pub fn drop_edge_over(&self, id: &str) -> Option<bench_ui::DropEdge> {
        self.sidebar
            .drop
            .as_ref()
            .filter(|(over, _)| over == id)
            .map(|(_, edge)| *edge)
    }

    /// A group of projects as a place the project in the air lands against: the pointer's half of
    /// it says the edge (`bench_ui::drop_edge`). The Host, always first, only takes it under itself.
    pub fn drop_zone(
        &mut self,
        zone: Stateful<Div>,
        onto: String,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        zone.on_drag_move::<DraggedProject>(cx.listener(
            move |this, ev: &DragMoveEvent<DraggedProject>, _, cx| {
                if ev.bounds.contains(&ev.event.position) {
                    let edge = if onto == ProjectKey::HOST {
                        bench_ui::DropEdge::Below
                    } else {
                        bench_ui::drop_edge(ev.event.position.y, &ev.bounds)
                    };
                    this.drag_over_project(&onto, edge, cx);
                }
            },
        ))
    }

    /// The room under the last group: the project in the air lands past it, and the line says so
    /// under that group. It fills what the list has left, and is never less than a group's gap.
    pub fn last_place_zone(&mut self, last: String, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("projects-end")
            .flex_1()
            .min_h(px(bench_ui::GROUP_GAP))
            .on_drag_move::<DraggedProject>(cx.listener(
                move |this, ev: &DragMoveEvent<DraggedProject>, _, cx| {
                    if ev.bounds.contains(&ev.event.position) {
                        this.drag_over_project(&last, bench_ui::DropEdge::Below, cx);
                    }
                },
            ))
            .into_any_element()
    }

    /// The list as what the project in the air moves over: out of it no line says it lands, near
    /// its top or bottom edge it scrolls, and let go anywhere in it, the project takes the place
    /// the line shows.
    pub fn drag_list(&mut self, list: Stateful<Div>, cx: &mut Context<Self>) -> Stateful<Div> {
        list.on_drag_move::<DraggedProject>(cx.listener(
            |this, ev: &DragMoveEvent<DraggedProject>, window, cx| {
                if !ev.bounds.contains(&ev.event.position) && this.sidebar.drop.take().is_some() {
                    cx.notify();
                }
                this.scroll_at_edge(ev.event.position.y, &ev.bounds, window, cx);
            },
        ))
        .on_drop(cx.listener(|this, d: &DraggedProject, _, cx| {
            this.sidebar.drag_scroll = None;
            if let Some((onto, edge)) = this.sidebar.drop.take() {
                this.drop_project(&d.id, &onto, edge, cx);
            }
        }))
    }

    /// While the pointer stays near an edge of the list, it scrolls that way. The list moves under
    /// a still pointer, so each step is told as a move: the line follows what is under it now.
    fn scroll_at_edge(&mut self, y: Pixels, list: &Bounds<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let edge = px(bench_ui::DRAG_SCROLL_EDGE);
        let way = if y < list.top() || y > list.bottom() {
            0.
        } else if y < list.top() + edge {
            1.
        } else if y > list.bottom() - edge {
            -1.
        } else {
            0.
        };
        if self.sidebar.drag_scroll.as_ref().map_or(0., |(w, _)| *w) == way {
            return;
        }
        let task = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(16)).await;
                let live = this.update(cx, |this, cx| {
                    let list = &this.sidebar.scroll;
                    let mut at = list.offset();
                    at.y = (at.y + px(way * bench_ui::DRAG_SCROLL_STEP)).clamp(-list.max_offset().y, px(0.));
                    list.set_offset(at);
                    cx.notify();
                    let live = cx.has_active_drag();
                    if !live {
                        this.sidebar.drag_scroll = None;
                    }
                    live
                });
                if !matches!(live, Ok(true)) {
                    return;
                }
                let _ = cx.update(|window, cx| {
                    let moved = MouseMoveEvent {
                        position: window.mouse_position(),
                        pressed_button: Some(MouseButton::Left),
                        modifiers: window.modifiers(),
                    };
                    window.dispatch_event(PlatformInput::MouseMove(moved), cx);
                });
            }
        });
        self.sidebar.drag_scroll = (way != 0.).then_some((way, task));
    }

    /// The project `from` takes the place it was dropped in, and every project on screen then
    /// keeps the place the person has given it: from that drop on, the list is theirs and not
    /// what the sources happened to return. Those not on screen (an agent still connecting) keep
    /// theirs, after these.
    fn drop_project(&mut self, from: &str, onto: &str, edge: bench_ui::DropEdge, cx: &mut Context<Self>) {
        let (_, groups) = self.sidebar_groups();
        let ids: Vec<String> = groups.into_iter().map(|(p, _)| p.id).collect();
        if let Some(order) = placed(&ids, from, onto, edge) {
            let away = self.sidebar.order.iter().filter(|id| !ids.contains(id)).cloned();
            self.sidebar.order = order.into_iter().chain(away.collect::<Vec<_>>()).collect();
            let _ = std::fs::create_dir_all(connection::state_dir());
            let _ = std::fs::write(path(), serde_json::to_vec(&self.sidebar.order).unwrap_or_default());
        }
        cx.notify();
    }
}

/// The projects `ids` with `from` at the given edge of `onto`, the Host's edge being the first
/// place; None: nothing moves, as it is not there (a project removed meanwhile) or it was let go
/// over itself.
fn placed(ids: &[String], from: &str, onto: &str, edge: bench_ui::DropEdge) -> Option<Vec<String>> {
    if onto == from {
        return None;
    }
    let mut ids = ids.to_vec();
    let moving = ids.iter().position(|i| i == from)?;
    ids.remove(moving);
    let to = match ids.iter().position(|i| i == onto) {
        Some(k) if edge == bench_ui::DropEdge::Below => k + 1,
        Some(k) => k,
        None => 0,
    };
    ids.insert(to, from.to_string());
    Some(ids)
}

#[cfg(test)]
mod tests {
    use super::placed;
    use bench_ui::DropEdge::{Above, Below};
    use crate::app::model::ProjectKey;

    fn ids() -> Vec<String> {
        ["a", "b", "c"].map(String::from).to_vec()
    }

    #[test]
    fn a_project_lands_where_it_was_let_go() {
        assert_eq!(placed(&ids(), "c", ProjectKey::HOST, Below).unwrap(), ["c", "a", "b"]);
        assert_eq!(placed(&ids(), "a", "c", Below).unwrap(), ["b", "c", "a"]);
        assert_eq!(placed(&ids(), "a", "b", Below).unwrap(), ["b", "a", "c"]);
        assert_eq!(placed(&ids(), "c", "b", Above).unwrap(), ["a", "c", "b"]);
    }

    #[test]
    fn let_go_over_itself_or_gone_it_moves_nothing() {
        assert_eq!(placed(&ids(), "b", "b", Above), None);
        assert_eq!(placed(&ids(), "b", "b", Below), None);
        assert_eq!(placed(&ids(), "x", ProjectKey::HOST, Below), None);
    }
}
