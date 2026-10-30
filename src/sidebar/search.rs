//! The search (⌘K, ⌘F, the sidebar's "Search"), as Claude's: a list over the window with its
//! field. Nothing typed, the recent threads and what can be made; typed, the threads and projects
//! whose names have it, then the threads whose messages do (the agent's search, `search_threads`).
//! ↑ ↓ choose, Enter opens, Esc or a click outside closes.

use gpui_kit::component::input::Input;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use bench_ui::Gap;

use crate::app::BenchApp;
use bench_ui::icons::Ico;
use protocol::ThreadId;

/// What a row of the search opens.
#[derive(Clone)]
enum Found {
    Thread(ThreadId),
    /// A thread whose messages have the words, with the matching words.
    Message(ThreadId, String),
    /// A project, opened in the Explorer.
    Project(String),
    NewThread,
    NewProject,
}

/// How many of each kind the list shows.
const PER_SECTION: usize = 8;

impl BenchApp {
    /// ⌘K: the search's list over the window, its field focused (the words typed last kept).
    pub(crate) fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar.finding = Some(0);
        let search = self.sidebar.search.clone();
        search.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    pub(crate) fn close_search(&mut self, cx: &mut Context<Self>) -> bool {
        let open = self.sidebar.finding.take().is_some();
        cx.notify();
        open
    }

    /// The rows for what is typed, in sections: (section's name, its rows).
    fn search_sections(&self) -> Vec<(&'static str, Vec<Found>)> {
        let q = self.sidebar.query.to_lowercase();
        let visible = self.visible_threads();
        if q.is_empty() {
            let recent = visible
                .iter()
                .take(PER_SECTION)
                .map(|&i| Found::Thread(self.threads.list[i].id.clone()))
                .collect();
            return vec![("Recent", recent), ("Actions", vec![Found::NewThread, Found::NewProject])];
        }
        let threads: Vec<Found> = visible
            .iter()
            .map(|&i| &self.threads.list[i])
            .filter(|t| t.title.to_lowercase().contains(&q))
            .take(PER_SECTION)
            .map(|t| Found::Thread(t.id.clone()))
            .collect();
        let projects: Vec<Found> = self
            .places
            .projects
            .iter()
            .filter(|p| p.name.to_lowercase().contains(&q))
            .take(PER_SECTION)
            .map(|p| Found::Project(p.id.clone()))
            .collect();
        let messages: Vec<Found> = self
            .sidebar
            .hits
            .iter()
            .flatten()
            .filter(|(id, _)| !threads.iter().any(|f| matches!(f, Found::Thread(t) if t.as_str() == id)))
            .take(PER_SECTION)
            .map(|(id, snippet)| {
                Found::Message(
                    id.clone().into(),
                    snippet.replace(">>>", "").replace("<<<", ""),
                )
            })
            .collect();
        vec![("Threads", threads), ("Projects", projects), ("In messages", messages)]
            .into_iter()
            .filter(|(_, rows)| !rows.is_empty())
            .collect()
    }

    fn search_rows(&self) -> Vec<Found> {
        self.search_sections().into_iter().flat_map(|(_, rows)| rows).collect()
    }

    fn search_move(&mut self, by: isize, cx: &mut Context<Self>) {
        let n = self.search_rows().len();
        if let Some(sel) = self.sidebar.finding.as_mut() {
            if n > 0 {
                *sel = (*sel as isize + by).rem_euclid(n as isize) as usize;
            }
        }
        cx.notify();
    }

    /// Enter, or a click: the chosen row opens and the list closes.
    pub(crate) fn search_choose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sel) = self.sidebar.finding else {
            return;
        };
        let Some(found) = self.search_rows().into_iter().nth(sel) else {
            return;
        };
        self.close_search(cx);
        match found {
            Found::Thread(id) | Found::Message(id, _) => {
                self.open_center(crate::workspace::Center::Chat, window, cx);
                self.open_listed(id, cx);
            }
            Found::Project(id) => self.show_explorer(id, window, cx),
            Found::NewThread => {
                self.new_thread_here(cx);
                self.focus_composer(window, cx);
            }
            Found::NewProject => self.new_project_form(window, cx),
        }
    }

    /// The list over the window while it is open: a dimmed ground that closes it on a click, the
    /// field, the sections.
    pub(crate) fn render_search(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let sel = self.sidebar.finding?;
        let theme = cx.theme().clone();
        let searching = !self.sidebar.query.is_empty() && self.sidebar.hits.is_none();
        let mut body: Vec<AnyElement> = vec![];
        let mut n = 0;
        for (name, rows) in self.search_sections() {
            body.push(
                div()
                    .px(Gap::L)
                    .pt(Gap::M)
                    .pb(Gap::Xs)
                    .text_size(px(bench_ui::SIDEBAR_HEAD_TEXT))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.muted_foreground)
                    .child(name)
                    .into_any_element(),
            );
            for found in rows {
                body.push(self.render_found(n, n == sel, &found, cx));
                n += 1;
            }
        }
        if n == 0 {
            body.push(
                div()
                    .p(Gap::L)
                    .text_color(theme.muted_foreground)
                    .child(if searching { "Searching…" } else { "Nothing found" })
                    .into_any_element(),
            );
        }
        Some(
            div()
                .id("search-ground")
                .absolute()
                .size_full()
                .bg(gpui_kit::black().opacity(0.15))
                .flex()
                .justify_center()
                .pt(px(bench_ui::SEARCH_TOP))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.close_search(cx);
                }))
                .child(
                    bench_ui::quick_frame("search", cx)
                        // A click inside is the list's, not the ground's.
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                            match ev.keystroke.key.as_str() {
                                "down" => this.search_move(1, cx),
                                "up" => this.search_move(-1, cx),
                                _ => return,
                            }
                            cx.stop_propagation();
                        }))
                        .child(div().p(Gap::M).child(Input::new(&self.sidebar.search)))
                        .child(
                            v_flex()
                                .id("search-list")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .pb(Gap::Xs)
                                .children(body),
                        ),
                )
                .into_any_element(),
        )
    }

    /// One row: its icon, its name, a quieter second word (the project, the matching words).
    fn render_found(
        &self,
        n: usize,
        chosen: bool,
        found: &Found,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let title_of = |id: &ThreadId| {
            self.threads
                .list
                .iter()
                .find(|t| &t.id == id)
                .map(|t| (t.title.clone(), t.project.clone()))
                .unwrap_or_else(|| ("Untitled".into(), String::new()))
        };
        let (icon, name, note): (Ico, String, String) = match found {
            Found::Thread(id) => {
                let (title, project) = title_of(id);
                (Ico::MessageSquare, title, self.project_name(&project))
            }
            Found::Message(id, snippet) => (Ico::Search, title_of(id).0, snippet.clone()),
            Found::Project(id) => (Ico::Folder, self.project_name(id), String::new()),
            Found::NewThread => (Ico::Plus, "New thread".into(), "⌘N".into()),
            Found::NewProject => (Ico::FolderPlus, "New project".into(), String::new()),
        };
        h_flex()
            .id(("found", n))
            .mx(Gap::Xs)
            .px(Gap::M)
            .py(Gap::Xs)
            .gap(Gap::M)
            .rounded_md()
            .items_center()
            .cursor_pointer()
            .when(chosen, |d| d.bg(theme.list_active))
            .when(!chosen, |d| d.hover(|s| s.bg(theme.list_hover)))
            .child(bench_ui::head_icon(icon, cx))
            .child(div().flex_none().max_w(px(bench_ui::SEARCH_NAME_W)).truncate().child(name))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(note),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.sidebar.finding = Some(n);
                this.search_choose(window, cx)
            }))
            .into_any_element()
    }
}
