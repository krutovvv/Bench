//! Documents drawn: the open one in the middle (a file in the code editor, or a part of the
//! environment as its `Section` draws it), the tabs in the header, the file kept beside on the
//! right, ⌘P's list over the middle. An editor fills its pane and scrolls itself, as in an IDE.

use gpui_kit::component::input::{Editor, Input};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{DocKey, FileRef};
use crate::app::BenchApp;
use crate::icons::Ico;

impl BenchApp {
    /// The middle in `Center::Doc`: the active document.
    pub(crate) fn render_doc(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let body = match self.explorer.active.clone() {
            Some(DocKey::File(f)) => self.file_pane(&f, false, cx),
            Some(DocKey::Part { kind, name }) => self.part_pane(kind, &name, cx),
            None => div().into_any_element(),
        };
        div()
            .relative()
            .size_full()
            .child(body)
            .children(self.render_quick(cx))
            .into_any_element()
    }

    /// A file: its head (where it is, Revert and Save while changed, beside / back, Close when
    /// beside) and its editor.
    fn file_pane(&mut self, f: &FileRef, beside: bool, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let key = DocKey::File(f.clone());
        let Some(d) = self.explorer.docs.get(&key) else {
            return div().into_any_element();
        };
        let (dirty, editor, binary, loading) = (d.dirty, d.editor.clone(), d.binary, d.loading);
        let tag = if beside { "beside" } else { "main" };
        let mut actions: Vec<AnyElement> = vec![];
        if dirty {
            let r = f.clone();
            actions.push(
                crate::ui::form_action(
                    SharedString::from(format!("doc-revert-{tag}")),
                    "Revert",
                    false,
                    cx,
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| this.revert_doc(r.clone(), window, cx)),
                )
                .into_any_element(),
            );
        }
        let s = key.clone();
        actions.push(
            save_button(SharedString::from(format!("doc-save-{tag}")), dirty, cx)
                .on_click(
                    cx.listener(move |this, _, window, cx| this.save_doc(s.clone(), window, cx)),
                )
                .into_any_element(),
        );
        // Where else it opens: Zed (on its machine), Finder on this Mac.
        let at = self.location(&f.project);
        let host = f
            .machine
            .clone()
            .or_else(|| (!at.agent.is_empty()).then(|| at.agent.clone()));
        let (zed, finder) = (f.path.clone(), f.path.clone());
        if host.is_none() {
            actions.push(
                crate::inserts::icon_button(
                    &format!("doc-finder-{tag}"),
                    Ico::FolderOpen,
                    "Show in Finder",
                    &theme,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.reveal_in_finder(&finder, window, cx)
                }))
                .into_any_element(),
            );
        }
        actions.push(
            crate::inserts::icon_button(&format!("doc-zed-{tag}"), Ico::Zed, "Open in Zed", &theme)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.open_in_zed(&zed, None, host.as_deref(), cx)
                }))
                .into_any_element(),
        );
        let other = f.clone();
        if beside {
            actions.push(
                crate::inserts::icon_button(
                    "doc-to-main",
                    Ico::PanelLeft,
                    "Open in the middle",
                    &theme,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.close_beside(cx);
                    this.open_doc(DocKey::File(other.clone()), window, cx)
                }))
                .into_any_element(),
            );
            actions.push(
                crate::inserts::icon_button("doc-beside-close", Ico::X, "Close", &theme)
                    .on_click(cx.listener(|this, _, _, cx| this.close_beside(cx)))
                    .into_any_element(),
            );
        } else {
            actions.push(
                crate::inserts::icon_button(
                    "doc-to-beside",
                    Ico::PanelRight,
                    "Open beside",
                    &theme,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_beside(other.clone(), window, cx)
                }))
                .into_any_element(),
            );
        }
        let where_ = match &f.machine {
            Some(m) => format!("{m}:{}", f.path),
            None => f.path.clone(),
        };
        let mut head = bench_ui::ide_head(f.name().to_string(), Some(where_.into()), actions, cx);
        // The right dock reaches the window's top: its head is the header's height and ground.
        if beside {
            head = head.h(bench_ui::bar_h(cx)).bg(theme.title_bar);
        }
        let body = if binary {
            note("Not text, or too large to edit here: Zed opens it", cx)
        } else if loading {
            note("Reading…", cx)
        } else {
            Editor::new(&editor)
                .appearance(false)
                .h_full()
                .into_any_element()
        };
        v_flex()
            .size_full()
            .child(head)
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }

    /// A part of the environment: its `Section`'s card, the head's actions its own.
    fn part_pane(&mut self, kind: &'static str, name: &str, cx: &mut Context<Self>) -> AnyElement {
        let card = crate::env::section(kind).and_then(|s| s.card(self, name, cx));
        let Some(card) = card else {
            return note("It is no longer there", cx);
        };
        v_flex()
            .size_full()
            .child(bench_ui::ide_head(card.title, None, card.actions, cx))
            .child(
                div()
                    .id("doc-part")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(card.body),
            )
            .into_any_element()
    }

    /// The pane on the right: the file kept beside.
    pub(crate) fn render_beside(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let f = self.explorer.beside.clone()?;
        Some(self.file_pane(&f, true, cx))
    }

    /// The header in `Center::Doc`: a tab per open document.
    pub(crate) fn render_doc_tabs(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let mut tabs = vec![];
        for (n, key) in self.explorer.tabs.clone().into_iter().enumerate() {
            let open = self.explorer.active.as_ref() == Some(&key);
            let (name, icon, dirty) = match &key {
                DocKey::File(f) => (
                    f.name().to_string(),
                    crate::ui::file_icon(f.name(), false, &theme),
                    self.explorer.docs.get(&key).is_some_and(|d| d.dirty),
                ),
                DocKey::Part { kind, name } => (
                    name.clone(),
                    (part_icon(kind), theme.muted_foreground),
                    false,
                ),
            };
            let armed = self.explorer.armed.as_deref() == Some(format!("close:{key:?}").as_str());
            let (k1, k2) = (key.clone(), key.clone());
            let close = crate::inserts::icon_button(
                &format!("doc-tab-x-{n}"),
                Ico::X,
                if armed {
                    "Close without saving"
                } else {
                    "Close  ⌘W"
                },
                &theme,
            )
            .when(armed, |d| d.text_color(theme.danger))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.close_doc(k1.clone(), cx)
            }));
            tabs.push(
                bench_ui::doc_tab(format!("doc-tab-{n}"), icon, name, open, dirty, close, cx)
                    .on_click(
                        cx.listener(move |this, _, window, cx| {
                            this.open_doc(k2.clone(), window, cx)
                        }),
                    )
                    .into_any_element(),
            );
        }
        h_flex()
            .id("doc-tabs")
            .size_full()
            .overflow_x_scroll()
            .children(tabs)
            .into_any_element()
    }

    /// ⌘P over the middle: the field and the files matching it.
    fn render_quick(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let q = self.explorer.quick.as_ref()?;
        let theme = cx.theme().clone();
        let input = q.input.clone();
        let (sel, listed) = (q.sel, q.files.is_some());
        let matches = self.quick_matches(cx);
        let rows = matches.into_iter().enumerate().map(|(n, rel)| {
            let name = rel.rsplit('/').next().unwrap_or(&rel).to_string();
            let (ico, color) = crate::ui::file_icon(&name, false, &theme);
            let pick = n;
            h_flex()
                .id(("quick", n))
                .px_3()
                .py_1()
                .gap_2()
                .items_center()
                .cursor_pointer()
                .when(n == sel, |d| d.bg(theme.list_active))
                .hover(|s| s.bg(theme.list_hover))
                .child(ico.icon().text_color(color))
                .child(div().flex_none().child(name))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(rel.clone()),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    if let Some(q) = this.explorer.quick.as_mut() {
                        q.sel = pick;
                    }
                    this.quick_open_chosen(window, cx)
                }))
                .into_any_element()
        });
        Some(
            div()
                .absolute()
                .top_2()
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(
                    v_flex()
                        .id("quick")
                        .w(px(bench_ui::QUICK_W))
                        .max_h(px(bench_ui::QUICK_MAX_H))
                        .rounded_md()
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.popover)
                        .shadow_lg()
                        .overflow_hidden()
                        .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                            match ev.keystroke.key.as_str() {
                                "down" => this.quick_move(1, cx),
                                "up" => this.quick_move(-1, cx),
                                _ => return,
                            }
                            cx.stop_propagation();
                        }))
                        .child(div().p_2().child(Input::new(&input)))
                        .child(
                            div()
                                .id("quick-list")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .pb_1()
                                .when(!listed, |d| d.child(note("Listing the folder…", cx)))
                                .children(rows),
                        ),
                )
                .into_any_element(),
        )
    }
}

/// A part's icon in its tab, by its section.
fn part_icon(kind: &str) -> Ico {
    match kind {
        "cron" => Ico::Clock,
        _ => Ico::Wrench,
    }
}

/// A document's Save (⌘S): filled once something changed.
fn save_button(id: SharedString, dirty: bool, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    div()
        .id(id)
        .flex_none()
        .px_2()
        .rounded_md()
        .cursor_pointer()
        .when(dirty, |d| {
            d.bg(theme.primary).text_color(theme.primary_foreground)
        })
        .when(!dirty, |d| d.text_color(theme.muted_foreground))
        .child("Save")
}

/// One muted line, padded.
fn note(text: &str, cx: &App) -> AnyElement {
    div()
        .p_3()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_string())
        .into_any_element()
}
