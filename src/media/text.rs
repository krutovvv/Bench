//! An answer's text: Markdown kept parsed while the thread is open (`Texts`), formulas as Unicode,
//! code blocks with their head.

use crate::media::{self, Segment, latex_text};
use bench_ui::icons::Ico;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::text::{
    MarkdownNode, MarkdownParseContext, MarkdownPlugin, TextView, TextViewState, markdown_ast,
};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// `$…$`: a formula as Unicode text in italics. Money ("$5 and $10") is not one: TeX math
/// never starts or ends with a space.
struct InlineMath;

impl MarkdownPlugin for InlineMath {
    fn name(&self) -> &str {
        "math"
    }

    fn parse(
        &self,
        node: &markdown_ast::Node,
        _: &MarkdownParseContext<'_>,
    ) -> Option<MarkdownNode> {
        let markdown_ast::Node::InlineMath(m) = node else {
            return None;
        };
        let math = m.value.trim() == m.value && !m.value.is_empty();
        let text = if math {
            latex_text(&m.value)
        } else {
            format!("${}$", m.value)
        };
        Some(MarkdownNode::new("math", math).text(text))
    }

    fn render(&self, node: &MarkdownNode, _: &mut Window, _: &mut App) -> impl IntoElement {
        let math = node.data::<bool>().copied().unwrap_or(true);
        div()
            .when(math, |d| d.italic())
            .child(node.as_text().to_string())
    }
}

/// `$$…$$`: a formula on its own line, centered.
struct BlockMath;

impl MarkdownPlugin for BlockMath {
    fn is_block(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "math-block"
    }

    fn parse(
        &self,
        node: &markdown_ast::Node,
        _: &MarkdownParseContext<'_>,
    ) -> Option<MarkdownNode> {
        let markdown_ast::Node::Math(m) = node else {
            return None;
        };
        Some(MarkdownNode::new("math-block", ()).text(latex_text(&m.value)))
    }

    fn render(&self, node: &MarkdownNode, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .w_full()
            .py_1()
            .flex()
            .justify_center()
            .italic()
            .text_base()
            .child(node.as_text().to_string())
    }
}

/// The thread's texts, parsed once and kept while the thread is open: id → (its state, the text
/// it holds). A text view keeps its parse only while it is on screen; a long one (over 4 KB)
/// coming back into view was parsed again in the background, drawn short for a frame and then
/// grew — which moved the thread under the wheel. Kept here, a row has its height at once.
#[derive(Default)]
pub struct Texts(std::collections::HashMap<String, (Entity<TextViewState>, String)>);

/// An answer as its parts, in order, without empty prose between them. The ids of its parts are
/// `answer-<row key>-<k>` and `code-<row key>-<k>`, k its place here.
pub(crate) fn answer_segments(text: &str) -> Vec<Segment> {
    media::segments(text.trim())
        .into_iter()
        .filter_map(|s| match s {
            Segment::Text(t) => {
                let t = t.trim().to_string();
                (!t.is_empty()).then_some(Segment::Text(t))
            }
            other => Some(other),
        })
        .collect()
}

impl Texts {
    /// The states under ids starting `from` move to the same ids starting `to`: the answer that
    /// was being written keeps its parsed text when it becomes the journal's entry, instead of
    /// being parsed again (a long one in the background, blank meanwhile — the thread flickers).
    pub fn rename(&mut self, from: &str, to: &str) {
        let ids: Vec<String> = self
            .0
            .keys()
            .filter(|id| id.starts_with(from))
            .cloned()
            .collect();
        for id in ids {
            if let Some(v) = self.0.remove(&id) {
                self.0.insert(format!("{to}{}", &id[from.len()..]), v);
            }
        }
    }

    /// The state for `id` holding `markdown`: made (its parse starts) or brought up to date.
    pub fn state(&mut self, id: &str, markdown: &str, cx: &mut App) -> Entity<TextViewState> {
        if let Some((state, had)) = self.0.get_mut(id) {
            if had != markdown {
                *had = markdown.to_string();
                state.update(cx, |s, cx| s.set_text(markdown, cx));
            }
            return state.clone();
        }
        let state = cx.new(|cx| TextViewState::markdown(markdown, cx));
        self.0
            .insert(id.to_string(), (state.clone(), markdown.to_string()));
        state
    }
}

impl Texts {
    /// An answer's prose (Markdown, with formulas), kept under `id`.
    pub(crate) fn markdown(&mut self, id: &str, text: &str, cx: &mut App) -> TextView {
        let state = self.state(id, text, cx);
        answer_style(TextView::new(&state), cx)
    }

    /// The person's own words exactly as written, kept under `id`.
    pub(crate) fn plain(&mut self, id: &str, text: &str, cx: &mut App) -> TextView {
        let state = self.state(id, &escape_plain(text), cx);
        TextView::new(&state)
    }
}

/// Text exactly as written, as Markdown: its characters escaped, line breaks kept, leading spaces
/// made non-breaking.
pub(crate) fn escape_plain(text: &str) -> String {
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            let body = line.trim_start_matches([' ', '\t']);
            let indent: String = line[..line.len() - body.len()]
                .chars()
                .map(|c| {
                    if c == '\t' {
                        "\u{a0}".repeat(4)
                    } else {
                        "\u{a0}".into()
                    }
                })
                .collect();
            let mut out = indent;
            for c in body.chars() {
                if c.is_ascii_punctuation() {
                    out.push('\\');
                }
                out.push(c);
            }
            out
        })
        .collect();
    // Two spaces before a newline: a line break inside the paragraph (blank lines stay paragraphs).
    lines.join("  \n")
}

/// Text exactly as written (a short one, outside the thread): selectable and copyable.
pub(crate) fn plain_text(id: impl Into<SharedString>, text: &str) -> TextView {
    TextView::markdown(ElementId::Name(id.into()), escape_plain(text))
}

/// Markdown as answers show it: with formulas. Code blocks keep the monospace font (trees and
/// columns stay aligned) at the chat text's size, not the theme's larger code size.
pub(crate) fn markdown(id: impl Into<SharedString>, text: String, cx: &App) -> TextView {
    answer_style(TextView::markdown(ElementId::Name(id.into()), text), cx)
}

/// Inline code as the theme paints it: a step of the page's own surface under the text's ink.
/// The kit's default tints the span with the selection colour, and code is not selected.
fn code_span(t: &gpui_kit::component::Theme) -> HighlightStyle {
    HighlightStyle {
        color: Some(t.foreground),
        background_color: Some(t.secondary),
        ..Default::default()
    }
}

fn answer_style(view: TextView, cx: &App) -> TextView {
    let code = code_span(&cx.theme());
    view.plugin(InlineMath).plugin(BlockMath).style(
        gpui_kit::component::text::TextViewStyle::default()
            .inline_code(code)
            .code_block(
                StyleRefinement::default()
                    .text_size(rems(1.0))
                    .line_height(relative(bench_ui::LEAD)),
            ),
    )
}

/// A code block taller than this shows its first lines and a “Show all” line: a log dump must
/// not take the window when nobody is going to read it.
pub(super) const FOLD_AFTER_LINES: usize = 10;
pub(super) const FOLD_PREVIEW_H: f32 = 140.;

/// A code block in an answer, as chat UIs show one: a header with the language on the left and
/// a copy button on the right, the highlighted code under it (one frame around both). A long one
/// comes folded (`folded`) with the caller's `more` line below.
pub(super) fn render_code(
    body: TextView,
    i: &str,
    k: usize,
    lang: String,
    code: String,
    folded: bool,
    more: Option<AnyElement>,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let body = body
        // Any action, even an empty one, keeps the language out of the code's own corner.
        .code_block_actions(|_, _, _| div())
        .style(
            gpui_kit::component::text::TextViewStyle::default()
                .inline_code(code_span(&theme))
                .code_block(
                    StyleRefinement::default()
                        .text_size(rems(1.0))
                        .line_height(relative(bench_ui::LEAD))
                        .border_0()
                        // No fill, like the audio, video and changed-files cards: the frame alone.
                        .bg(gpui_kit::transparent_black()),
                ),
        );
    let copy = code.trim_end_matches('\n').to_string();
    v_flex()
        .w_full()
        .border_1()
        .rounded_md()
        .overflow_hidden()
        .border_color(theme.border)
        .child(
            // As tall as the changed-files card's line: small text, 4 above and below.
            bench_ui::block_head(&theme, true)
                .justify_between()
                .child(if lang.is_empty() {
                    "code".to_string()
                } else {
                    lang
                })
                .child(
                    h_flex()
                        .id(SharedString::from(format!("code-copy-{i}-{k}")))
                        .p_1()
                        .rounded_sm()
                        .items_center()
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                        .tooltip(|window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new("Copy").build(window, cx)
                        })
                        .child(Ico::Copy.icon().small())
                        .on_click(move |_, window, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
                            window.push_notification(
                                (NotificationType::Success, SharedString::from("Copied")),
                                cx,
                            );
                        }),
                ),
        )
        .child(
            div()
                .when(folded, |d| d.max_h(px(FOLD_PREVIEW_H)).overflow_hidden())
                .child(body),
        )
        .when_some(more, |d, m| d.child(m))
        .into_any_element()
}
