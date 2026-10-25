//! The line under the mini IDE's editor, as in Zed: where the cursor is and what is selected,
//! the language. Its own small view, observing the editor, so moving the cursor repaints only it.
//! A click on "Ln, Col" asks to go to a line (⌃G).

use gpui_kit::component::input::EditorState;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;

pub struct EditorStatus {
    editor: Entity<EditorState>,
    app: WeakEntity<BenchApp>,
    _observe: Subscription,
}

impl EditorStatus {
    pub fn new(
        editor: Entity<EditorState>,
        app: WeakEntity<BenchApp>,
        cx: &mut Context<Self>,
    ) -> Self {
        let observe = cx.observe(&editor, |_, _, cx| cx.notify());
        Self {
            editor,
            app,
            _observe: observe,
        }
    }
}

impl Render for EditorStatus {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let s = self.editor.read(cx);
        let at = s.cursor_position();
        let range = s.selected_range();
        let selected = if range.is_empty() {
            None
        } else {
            let text = s.value();
            let picked = text.get(range.clone()).unwrap_or_default();
            let lines = picked.matches('\n').count();
            Some(if lines > 0 {
                format!(
                    "{} chars, {} lines selected",
                    picked.chars().count(),
                    lines + 1
                )
            } else {
                format!("{} selected", picked.chars().count())
            })
        };
        let language = s.language_name().to_string();
        let app = self.app.clone();
        h_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .gap_3()
            .items_center()
            .child(
                div()
                    .id("status-goto")
                    .cursor_pointer()
                    .hover(|s| s.text_color(theme.foreground))
                    .child(format!("Ln {}, Col {}", at.line + 1, at.character + 1))
                    .tooltip(|window, cx| Tooltip::new("Go to line  ⌃G").build(window, cx))
                    .on_click(move |_, window, cx| {
                        let _ = app.update(cx, |this, cx| this.open_go_line(window, cx));
                    }),
            )
            .children(selected)
            .child(div().flex_1())
            .when(!language.is_empty() && language != "text", |d| {
                d.child(language)
            })
    }
}
