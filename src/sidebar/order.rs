//! The order of the sidebar's projects: the person makes it by dragging a project's group where
//! they want it — working in two projects at once keeps them under each other, near the top.
//! Kept on this Mac (`project-order.json`) as ids in order; a project not in it yet — a new one —
//! keeps the place its source gave and sits after the arranged ones; one whose agent has not
//! answered keeps its place for when it does.

use gpui_kit::component::{ActiveTheme as _};
use gpui_kit::*;
use bench_ui::Gap;

use crate::app::BenchApp;
use crate::client::connection;

/// Where a dragged project goes among the ones on screen.
#[derive(Clone, Debug)]
pub enum ProjectPlace {
    /// The front of the projects (under the Host, which is always first).
    Front,
    /// Above or below another project's group, as the pointer was over it.
    On(String, bench_ui::DropEdge),
    /// Past the last group: the end of the list.
    Last,
}

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
        let theme = cx.theme();
        bench_ui::row(Gap::S)
            .pl(px(bench_ui::SIDEBAR_TEXT_X))
            .pr(px(bench_ui::SIDEBAR_ROW_INSET))
            .py(px(bench_ui::GROUP_HEAD_PB))
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
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
    /// it says the edge (`bench_ui::drop_edge`), and letting go takes that place.
    pub fn drop_zone(
        &mut self,
        zone: Stateful<Div>,
        onto: String,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let over = onto.clone();
        zone.on_drag_move::<DraggedProject>(cx.listener(
            move |this, ev: &DragMoveEvent<DraggedProject>, _, cx| {
                if !ev.bounds.contains(&ev.event.position) {
                    return;
                }
                let edge = bench_ui::drop_edge(ev.event.position.y, &ev.bounds);
                this.drag_over_project(&over, edge, cx);
            },
        ))
        .on_drop(cx.listener(move |this, d: &DraggedProject, _, cx| {
            let edge = this
                .drop_edge_over(&onto)
                .unwrap_or(bench_ui::DropEdge::Below);
            this.drop_project(&d.id, ProjectPlace::On(onto.clone(), edge), cx);
        }))
    }

    /// The Host's group: always first, so the project in the air lands under it, the first place.
    pub fn first_place_zone(
        &mut self,
        zone: Stateful<Div>,
        host: String,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        zone.on_drag_move::<DraggedProject>(cx.listener(
            move |this, ev: &DragMoveEvent<DraggedProject>, _, cx| {
                if ev.bounds.contains(&ev.event.position) {
                    this.drag_over_project(&host, bench_ui::DropEdge::Below, cx);
                }
            },
        ))
        .on_drop(cx.listener(|this, d: &DraggedProject, _, cx| {
            this.drop_project(&d.id, ProjectPlace::Front, cx);
        }))
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
            .on_drop(cx.listener(|this, d: &DraggedProject, _, cx| {
                this.drop_project(&d.id, ProjectPlace::Last, cx);
            }))
            .into_any_element()
    }

    /// The project `from` takes the place it was dropped in, and every project on screen then
    /// keeps the place the person has given it: from that drop on, the list is theirs and not
    /// what the sources happened to return. Those not on screen (an agent still connecting) keep
    /// theirs, after these.
    pub fn drop_project(&mut self, from: &str, at: ProjectPlace, cx: &mut Context<Self>) {
        self.sidebar.drop = None;
        let (_, groups) = self.sidebar_groups();
        let ids: Vec<String> = groups.into_iter().map(|(p, _)| p.id).collect();
        if let Some(order) = placed(&ids, from, &at) {
            let away = self.sidebar.order.iter().filter(|id| !ids.contains(id)).cloned();
            self.sidebar.order = order.into_iter().chain(away.collect::<Vec<_>>()).collect();
            let _ = std::fs::create_dir_all(connection::state_dir());
            let _ = std::fs::write(path(), serde_json::to_vec(&self.sidebar.order).unwrap_or_default());
        }
        cx.notify();
    }
}

/// The projects `ids` with `from` at the place `at`; None: nothing moves, as it is not there (a
/// project removed meanwhile) or it was let go over itself.
fn placed(ids: &[String], from: &str, at: &ProjectPlace) -> Option<Vec<String>> {
    if matches!(at, ProjectPlace::On(onto, _) if onto == from) {
        return None;
    }
    let mut ids = ids.to_vec();
    let moving = ids.iter().position(|i| i == from)?;
    ids.remove(moving);
    let to = match at {
        ProjectPlace::Front => 0,
        ProjectPlace::Last => ids.len(),
        ProjectPlace::On(onto, edge) => {
            let k = ids.iter().position(|i| i == onto).unwrap_or(ids.len());
            if *edge == bench_ui::DropEdge::Below { k + 1 } else { k }
        }
    };
    ids.insert(to.min(ids.len()), from.to_string());
    Some(ids)
}

#[cfg(test)]
mod tests {
    use super::{ProjectPlace, placed};
    use bench_ui::DropEdge::{Above, Below};

    fn ids() -> Vec<String> {
        ["a", "b", "c"].map(String::from).to_vec()
    }

    fn on(id: &str, edge: bench_ui::DropEdge) -> ProjectPlace {
        ProjectPlace::On(id.into(), edge)
    }

    #[test]
    fn a_project_lands_where_it_was_let_go() {
        assert_eq!(placed(&ids(), "c", &ProjectPlace::Front).unwrap(), ["c", "a", "b"]);
        assert_eq!(placed(&ids(), "a", &ProjectPlace::Last).unwrap(), ["b", "c", "a"]);
        assert_eq!(placed(&ids(), "a", &on("b", Below)).unwrap(), ["b", "a", "c"]);
        assert_eq!(placed(&ids(), "c", &on("b", Above)).unwrap(), ["a", "c", "b"]);
    }

    #[test]
    fn let_go_over_itself_or_gone_it_moves_nothing() {
        assert_eq!(placed(&ids(), "b", &on("b", Above)), None);
        assert_eq!(placed(&ids(), "b", &on("b", Below)), None);
        assert_eq!(placed(&ids(), "x", &ProjectPlace::Front), None);
    }
}
