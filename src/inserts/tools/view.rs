//! Tools mode's views: the list of tools in the sidebar (groups like the servers'), the open
//! tool's header (name, a hint, its modes) and its body — input | output side by side, a
//! Markdown preview, a generator, the regex tester or the Base64 image.

use gpui_kit::component::input::{Editor, Input};
use gpui_kit::component::text::TextView;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use devtools::{Kind, Tool};

impl BenchApp {
    /// Rich Text to Markdown's input: the paste rendered as it looked on the page (tables,
    /// headings, links, images), or its HTML code to edit.
    fn render_rich_input(
        &mut self,
        input_bar: impl Fn(Vec<AnyElement>, &mut Context<Self>) -> AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let source = self.tools.html_source;
        let toggle = segmented(
            "tool-html-view",
            &["Rendered", "HTML"],
            source as usize,
            cx,
            |this, k, _, cx| {
                this.tools.html_source = k == 1;
                cx.notify();
            },
        );
        let body = if source {
            div()
                .flex_1()
                .min_h_0()
                .child(Editor::new(&self.tools.input).appearance(false).h_full())
                .into_any_element()
        } else if self.tools.preview.trim().is_empty() {
            div()
                .flex_1()
                .px_4()
                .py_3()
                .text_base()
                .whitespace_normal()
                .text_color(theme.muted_foreground)
                .child("Copy part of a web page, a Word document or a note, then press ⌘V")
                .into_any_element()
        } else {
            crate::inserts::scroll_pane(
                "tool-html",
                &self.tools.html_scroll,
                div().px_4().py_3().text_base().child(
                    TextView::html("tool-html-view-body", self.tools.preview.clone())
                        .selectable(true),
                ),
            )
        };
        v_flex()
            .flex_1()
            .min_w_0()
            .child(input_bar(vec![toggle], cx))
            .child(body)
            .into_any_element()
    }

    pub(crate) fn render_tools_body(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let tool = self.tools.tool;
        let input_bar = |extra: Vec<AnyElement>, cx: &mut Context<Self>| {
            let mut buttons = vec![
                small_button("tool-paste", "Paste", cx, |this, window, cx| {
                    this.paste_tool_input(window, cx)
                }),
                small_button("tool-sample", "Sample", cx, |this, window, cx| {
                    let s = this.tools.tool.sample().to_string();
                    this.tool_input(s, window, cx)
                }),
                small_button("tool-clear", "Clear", cx, |this, window, cx| {
                    this.tool_input(String::new(), window, cx)
                }),
            ];
            buttons.extend(extra);
            pane_bar("Input", None, buttons, cx)
        };
        // Copying is an icon, as on every block.
        let copy = |cx: &mut Context<Self>| {
            let theme = cx.theme().clone();
            div()
                .id("tool-copy")
                .flex_none()
                .p_1()
                .rounded_sm()
                .cursor_pointer()
                .text_color(theme.muted_foreground)
                .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                .tooltip(|window, cx| Tooltip::new("Copy").build(window, cx))
                .child(bench_ui::icons::Ico::Copy.icon().small())
                .on_click(cx.listener(|this, _, _, cx| this.copy_tool_output(cx)))
                .into_any_element()
        };
        let editor = |state: &Entity<gpui_kit::component::input::EditorState>| {
            div()
                .flex_1()
                .min_h_0()
                .child(Editor::new(state).appearance(false).h_full())
        };
        let error = self.tools.error.clone();
        let input_pane = |extra: Vec<AnyElement>, this: &mut Self, cx: &mut Context<Self>| {
            v_flex()
                .flex_1()
                .min_w_0()
                .child(input_bar(extra, cx))
                .child(editor(&this.tools.input))
        };
        // Panes fill the height (h_flex centers its children otherwise).
        let split = |left: AnyElement, right: AnyElement| {
            h_flex()
                .size_full()
                .items_stretch()
                .child(left)
                .child(div().w(px(1.)).h_full().bg(theme.border))
                .child(right)
                .into_any_element()
        };

        match tool.kind() {
            Kind::Transform => {
                let left = if tool == Tool::HtmlToMarkdown {
                    self.render_rich_input(input_bar, cx)
                } else {
                    input_pane(vec![], self, cx).into_any_element()
                };
                let right = v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(pane_bar("Output", error, vec![copy(cx)], cx))
                    .child(editor(&self.tools.output))
                    .into_any_element();
                split(left, right)
            }
            Kind::Preview => {
                let left = input_pane(vec![], self, cx).into_any_element();
                let right = v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(pane_bar("Preview", None, vec![], cx))
                    .child(crate::inserts::scroll_pane(
                        "tool-preview",
                        &self.tools.preview_scroll,
                        div()
                            .px_4()
                            .py_3()
                            .text_base()
                            .child(crate::media::text::markdown(
                                "tool-md",
                                self.tools.preview.clone(),
                                cx,
                            )),
                    ))
                    .into_any_element();
                split(left, right)
            }
            Kind::Generator => {
                let counts: &[usize] = match tool {
                    Tool::Lorem => &[1, 3, 5, 10],
                    _ => &[1, 5, 10, 25],
                };
                let current = self.tools.count(tool);
                let labels: Vec<String> = counts.iter().map(|n| n.to_string()).collect();
                let chosen = counts.iter().position(|n| *n == current).unwrap_or(0);
                let counts_vec = counts.to_vec();
                let bar = pane_bar(
                    "Output",
                    None,
                    vec![
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(if tool == Tool::Lorem {
                                "How many"
                            } else {
                                "Count"
                            })
                            .into_any_element(),
                        segmented_owned(
                            "tool-count",
                            labels,
                            chosen,
                            cx,
                            move |this, k, window, cx| {
                                this.tools.counts.insert(tool, counts_vec[k]);
                                this.run_tool(window, cx)
                            },
                        ),
                        small_button("tool-gen", "Generate", cx, |this, window, cx| {
                            this.generate_tool(window, cx)
                        }),
                        copy(cx),
                    ],
                    cx,
                );
                v_flex()
                    .size_full()
                    .child(bar)
                    .child(editor(&self.tools.output))
                    .into_any_element()
            }
            Kind::Regex => {
                let flags = self.tools.flags;
                let flag = |k: usize,
                            label: &'static str,
                            on: bool,
                            tip: &'static str,
                            cx: &mut Context<Self>| {
                    div()
                        .id(("regex-flag", k))
                        .px_1p5()
                        .py_0p5()
                        .text_sm()
                        .font_family(bench_ui::theme::MONO)
                        .border_1()
                        .rounded_md()
                        .border_color(theme.border)
                        .cursor_pointer()
                        .when(on, |d| {
                            d.bg(theme.primary).text_color(theme.primary_foreground)
                        })
                        .when(!on, |d| d.hover(|s| s.bg(theme.list_hover)))
                        .child(label)
                        .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.toggle_regex_flag(k, window, cx)
                        }))
                        .into_any_element()
                };
                let pattern_row = h_flex()
                    .flex_none()
                    .px_3()
                    .py_2()
                    .gap_2()
                    .items_center()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(div().flex_1().child(Input::new(&self.tools.pattern)))
                    .child(flag(0, "i", flags.case_insensitive, "Ignore case", cx))
                    .child(flag(
                        1,
                        "m",
                        flags.multi_line,
                        "^ and $ match at line ends",
                        cx,
                    ))
                    .child(flag(2, "s", flags.dot_all, ". matches newlines", cx));
                let left = input_pane(vec![], self, cx).into_any_element();
                let count = self.tools.matches;
                let right = v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(pane_bar(
                        &match count {
                            1 => "1 match".to_string(),
                            n => format!("{n} matches"),
                        },
                        error,
                        vec![copy(cx)],
                        cx,
                    ))
                    .child(editor(&self.tools.output))
                    .into_any_element();
                v_flex()
                    .size_full()
                    .child(pattern_row)
                    .child(div().flex_1().min_h_0().child(split(left, right)))
                    .into_any_element()
            }
            Kind::Image => {
                let left = input_pane(
                    vec![small_button(
                        "tool-load",
                        "Load image…",
                        cx,
                        |this, window, cx| this.load_tool_image(window, cx),
                    )],
                    self,
                    cx,
                )
                .into_any_element();
                let image = self.tools.image.clone();
                let title = match &self.tools.image_info {
                    Some(info) => format!("Image · {info}"),
                    None => "Image".into(),
                };
                let right = v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(pane_bar(
                        &title,
                        error,
                        if image.is_some() {
                            vec![small_button(
                                "tool-save",
                                "Save…",
                                cx,
                                |this, window, cx| this.save_tool_image(window, cx),
                            )]
                        } else {
                            vec![]
                        },
                        cx,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .p_4()
                            .flex()
                            .items_center()
                            .justify_center()
                            .map(|d| match image {
                                Some(path) => d.child(
                                    // Fit to the pane, small images scaled up too.
                                    img(path).size_full().object_fit(ObjectFit::Contain),
                                ),
                                None => d
                                    .text_base()
                                    .text_color(theme.muted_foreground)
                                    .child("Paste Base64 or load an image"),
                            }),
                    )
                    .into_any_element();
                split(left, right)
            }
        }
    }
}

/// The strip above a pane: its name (or an error in red), then its buttons.
pub(crate) fn pane_bar(
    title: &str,
    error: Option<String>,
    buttons: Vec<AnyElement>,
    cx: &mut Context<BenchApp>,
) -> AnyElement {
    let theme = cx.theme();
    bench_ui::block_head(theme, true)
        .child(div().flex_1().min_w_0().truncate().map(|d| {
            match &error {
                Some(e) => d.text_color(theme.danger).child(e.clone()),
                None => d
                    .text_color(theme.muted_foreground)
                    .child(title.to_string()),
            }
        }))
        .children(buttons)
        .into_any_element()
}

/// A small square text button in a pane's strip.
pub(crate) fn small_button(
    id: &'static str,
    label: &'static str,
    cx: &mut Context<BenchApp>,
    on: impl Fn(&mut BenchApp, &mut Window, &mut Context<BenchApp>) + 'static,
) -> AnyElement {
    let theme = cx.theme().clone();
    div()
        .id(id)
        .flex_none()
        .px_2()
        .py_0p5()
        .text_sm()
        .border_1()
        .rounded_md()
        .border_color(theme.border)
        .cursor_pointer()
        .hover(|s| s.bg(theme.list_hover))
        .child(label)
        .on_click(cx.listener(move |this, _, window, cx| on(this, window, cx)))
        .into_any_element()
}

/// Pick one: square cells sharing borders, the chosen one in the accent.
pub(crate) fn segmented(
    id: &'static str,
    labels: &[&'static str],
    chosen: usize,
    cx: &mut Context<BenchApp>,
    on: impl Fn(&mut BenchApp, usize, &mut Window, &mut Context<BenchApp>) + Clone + 'static,
) -> AnyElement {
    segmented_owned(
        id,
        labels.iter().map(|s| s.to_string()).collect(),
        chosen,
        cx,
        on,
    )
}

pub(crate) fn segmented_owned(
    id: &'static str,
    labels: Vec<String>,
    chosen: usize,
    cx: &mut Context<BenchApp>,
    on: impl Fn(&mut BenchApp, usize, &mut Window, &mut Context<BenchApp>) + Clone + 'static,
) -> AnyElement {
    let theme = cx.theme().clone();
    h_flex()
        .flex_none()
        .border_1()
        .rounded_md()
        .overflow_hidden()
        .border_color(theme.border)
        .children(labels.into_iter().enumerate().map(|(k, label)| {
            let on = on.clone();
            div()
                .id((id, k))
                .px_2()
                .py_0p5()
                .text_sm()
                .cursor_pointer()
                .when(k > 0, |d| d.border_l_1().border_color(theme.border))
                .when(k == chosen, |d| {
                    d.bg(theme.primary).text_color(theme.primary_foreground)
                })
                .when(k != chosen, |d| d.hover(|s| s.bg(theme.list_hover)))
                .child(label)
                .on_click(cx.listener(move |this, _, window, cx| on(this, k, window, cx)))
        }))
        .into_any_element()
}

/// `segmented` as tall as a text field, for a form's row beside fields; labels are made
/// at run time (a form's list of servers).
pub(crate) fn segmented_tall_owned(
    id: &'static str,
    labels: Vec<String>,
    chosen: usize,
    cx: &mut Context<BenchApp>,
    on: impl Fn(&mut BenchApp, usize, &mut Window, &mut Context<BenchApp>) + Clone + 'static,
) -> AnyElement {
    let theme = cx.theme().clone();
    h_flex()
        .flex_none()
        .h(px(bench_ui::FIELD_H))
        .border_1()
        .rounded_md()
        .overflow_hidden()
        .border_color(theme.border)
        .children(labels.into_iter().enumerate().map(|(k, label)| {
            let on = on.clone();
            div()
                .id((id, k))
                .h_full()
                .flex()
                .items_center()
                .px_3()
                // The fields beside it draw their text at `text_sm` (gpui-kit's default input):
                // the cells match, so nothing in the row changes size.
                .text_sm()
                .cursor_pointer()
                .when(k > 0, |d| d.border_l_1().border_color(theme.border))
                .when(k == chosen, |d| {
                    d.bg(theme.primary).text_color(theme.primary_foreground)
                })
                .when(k != chosen, |d| d.hover(|s| s.bg(theme.list_hover)))
                .child(label)
                .on_click(cx.listener(move |this, _, window, cx| on(this, k, window, cx)))
        }))
        .into_any_element()
}
