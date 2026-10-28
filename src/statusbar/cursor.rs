//! The status bar's corner for the open file (Code mode): where the caret is, how the file is
//! indented, its language — as an editor's status bar says them. A view of its own that watches
//! the file's editor, so a caret that moves redraws these words and not the window.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::h_flex;
use gpui_kit::component::input::EditorState;
use gpui_kit::*;

/// What the corner shows: the open file's editor and its path, or nothing.
#[derive(Default)]
pub struct CursorItem {
    file: Option<(Entity<EditorState>, String)>,
    _watch: Option<Subscription>,
}

impl CursorItem {
    /// Show `file` (its editor and path), or nothing; the same file again changes nothing.
    pub fn show(&mut self, file: Option<(Entity<EditorState>, String)>, cx: &mut Context<Self>) {
        if self.file.as_ref().map(|(e, _)| e.entity_id()) == file.as_ref().map(|(e, _)| e.entity_id()) {
            return;
        }
        self._watch = file
            .as_ref()
            .map(|(editor, _)| cx.observe(editor, |_, _, cx| cx.notify()));
        self.file = file;
        cx.notify();
    }
}

impl Render for CursorItem {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some((editor, path)) = &self.file else {
            return div();
        };
        let state = editor.read(cx);
        let at = state.cursor_position();
        let text = state.value();
        let words = [
            format!("Ln {}, Col {}", at.line + 1, at.character + 1),
            crate::explorer::code::lang::indent_of(&text),
            crate::explorer::code::lang::language_name(crate::explorer::code::lang::language_for_text(path, &text)),
        ];
        div().child(
            h_flex()
                .h_full()
                .items_center()
                .text_size(px(bench_ui::BAR_TEXT))
                .text_color(cx.theme().muted_foreground)
                .children(words.into_iter().map(|w| div().px_2().child(w))),
        )
    }
}
