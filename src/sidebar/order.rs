//! The order of the sidebar's projects: the person makes it by dragging a project's group where
//! they want it — working in two projects at once keeps them under each other, near the top.
//! Kept on this Mac (`project-order.json`) as ids in order; a project not in it yet — a new one,
//! or one whose agent has not answered — keeps the place its source gave and sits after the
//! arranged ones. Pin is the same order at its front: pinning puts a project first, dragging one
//! says its place outright and lets its pin go.

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
    /// it says the edge (`bench_ui::drop_edge`), and letting go takes that place. The list itself
    /// is the place for the end of it (`sidebar/mod.rs`); the Host is no place at all, so it is
    /// never wrapped in this.
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

    /// The project `from` takes the place it was dropped in, and every project on screen then
    /// keeps the place the person has given it: from that drop on, the list is theirs and not
    /// what the sources happened to return.
    pub fn move_project(&mut self, from: &str, at: ProjectPlace, cx: &mut Context<Self>) {
        let (_, groups) = self.sidebar_groups();
        let mut ids: Vec<String> = groups.into_iter().map(|(p, _)| p.id).collect();
        let Some(moving) = ids.iter().position(|i| i == from) else {
            // Not on the screen any more — a project removed, an agent gone: nothing to move.
            return;
        };
        ids.remove(moving);
        let to = match &at {
            ProjectPlace::Front => 0,
            ProjectPlace::Last => ids.len(),
            ProjectPlace::On(onto, edge) => {
                let k = ids.iter().position(|i| i == onto).unwrap_or(ids.len());
                if *edge == bench_ui::DropEdge::Below {
                    k + 1
                } else {
                    k
                }
            }
        };
        ids.insert(to.min(ids.len()), from.to_string());
        self.sidebar.order = ids;
        self.store_order();
        cx.notify();
    }

    /// A project taken from the air and let go: it takes the place, and its pin goes with it — the
    /// pin is the same wish, only ever at the front, and the list now says the place outright, so a
    /// badge left on would promise what the list no longer does.
    pub fn drop_project(&mut self, from: &str, at: ProjectPlace, cx: &mut Context<Self>) {
        let key = format!("p:{from}");
        if self.mark_of(&key).pinned {
            self.toggle_pin(key, cx);
        }
        self.move_project(from, at, cx);
    }

    /// The saved order holds only the projects still listed; one that comes back later takes its
    /// source's place after the arranged ones.
    fn store_order(&self) {
        let live: Vec<&String> = self.places.projects.iter().map(|p| &p.id).collect();
        let order: Vec<&String> = self
            .sidebar
            .order
            .iter()
            .filter(|id| live.contains(id))
            .collect();
        let _ = std::fs::create_dir_all(connection::state_dir());
        let _ = std::fs::write(path(), serde_json::to_vec(&order).unwrap_or_default());
    }
}
