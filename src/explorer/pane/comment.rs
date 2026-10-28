//! A comment on lines of a file, as a reviewer leaves one: ⌘⇧L (or the head's button) opens a
//! field under the editor naming the selected lines; Enter puts the comment in the thread's
//! composer as a chip (`Reference::Comment`, `thread/composer/attachments.rs`), Esc drops it.
//! Comments gather there until the message goes: the agent addresses each and resolves it by id.
//! The caret stays in the code, so the next selection can be commented at once.

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::*;

use crate::app::BenchApp;
use crate::explorer::{DocKey, FileRef};
use crate::thread::composer::attachments::{CodeRef, CommentRef, Reference, lines_of};
use crate::workspace::{Center, Panel};

/// A comment being written: the file and lines it is about, the field.
pub struct Commenting {
    pub file: FileRef,
    pub code: CodeRef,
    pub input: Entity<InputState>,
}

impl BenchApp {
    /// ⌘⇧L: a comment on the selected lines of the file with the caret (its line, without a
    /// selection).
    pub fn start_comment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(DocKey::File(f)) = self.focused_doc() else {
            return;
        };
        let Some(d) = self.explorer.docs.get(&DocKey::File(f.clone())) else {
            return;
        };
        let s = d.editor.read(cx);
        let text = s.value().to_string();
        let range = s.selected_range();
        let (a, b) = lines_of(&text, range.clone());
        let lines: Vec<&str> = text.lines().collect();
        let picked = lines
            .get(a.saturating_sub(1)..b.min(lines.len()))
            .map(|l| l.join("\n"))
            .unwrap_or_default();
        let code = CodeRef {
            path: f.path.clone(),
            lines: Some((a, b)),
            text: Some(picked),
        };
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("What should change here"));
        cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.finish_comment(window, cx);
            }
        })
        .detach();
        input.update(cx, |s, cx| s.focus(window, cx));
        self.explorer.comment = Some(Commenting { file: f, code, input });
        cx.notify();
    }

    /// Enter: the comment to the thread's composer, the thread shown beside, the caret back in
    /// the code.
    pub fn finish_comment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = self.explorer.comment.as_ref() else {
            return;
        };
        let note = c.input.read(cx).value().trim().to_string();
        if note.is_empty() {
            return;
        }
        let Some(c) = self.explorer.comment.take() else {
            return;
        };
        let id = CommentRef::new_id(&c.code, &note);
        self.add_reference(Reference::Comment(CommentRef { id, code: c.code, note }), cx);
        if self.center != Center::Chat {
            self.right_dock.activate(Panel::Thread);
        }
        if let Some(d) = self.explorer.docs.get(&DocKey::File(c.file)) {
            d.editor.update(cx, |s, cx| s.focus(window, cx));
        }
        cx.notify();
    }

    /// The band under a file's editor while a comment on it is written: the lines, the field,
    /// Comment (Esc drops it, `app/keys.rs`).
    pub(crate) fn render_comment(&self, f: &FileRef, cx: &mut Context<Self>) -> Option<AnyElement> {
        let c = self.explorer.comment.as_ref().filter(|c| &c.file == f)?;
        let add = bench_ui::form_action("comment-add", "Comment", true, cx)
            .on_click(cx.listener(|this, _, window, cx| this.finish_comment(window, cx)));
        let at = match c.code.lines {
            Some((a, b)) if a == b => format!("Line {a}"),
            Some((a, b)) => format!("Lines {a}–{b}"),
            None => String::new(),
        };
        Some(
            bench_ui::ide_band(
                at,
                bench_ui::field(&c.input).w_full().into_any_element(),
                vec![add.into_any_element()],
                cx,
            )
            .into_any_element(),
        )
    }
}
