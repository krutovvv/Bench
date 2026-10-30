//! Tools mode's views: the list of tools in the sidebar (groups like the servers'), the open
//! tool's header (name, a hint, its modes) and its body — input | output side by side, a
//! Markdown preview, a generator, the regex tester or the Base64 image.

use gpui_kit::component::input::{Editor, Input};
use gpui_kit::component::text::TextView;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use bench_ui::Gap;

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
        let source = self.inserts.tools.html_source;
        let toggle = bench_ui::segmented_cells(
            "tool-html-view",
            ["Rendered", "HTML"],
            source as usize,
            cx,
            bench_ui::on_pick(cx, |this: &mut Self, k, _, cx| {
                this.inserts.tools.html_source = k == 1;
                cx.notify();
            }),
        )
        .into_any_element();
        let body = if source {
            div()
                .flex_1()
                .min_h_0()
                .child(Editor::new(&self.inserts.tools.input).appearance(false).h_full())
                .into_any_element()
        } else if self.inserts.tools.preview.trim().is_empty() {
            div()
                .flex_1()
                .px(Gap::Xl)
                .py(Gap::L)
                .text_size(px(bench_ui::THREAD_META))
                .whitespace_normal()
                .text_color(theme.muted_foreground)
                .child("Copy part of a web page, a Word document or a note, then press ⌘V")
                .into_any_element()
        } else {
            crate::inserts::scroll_pane(
                "tool-html",
                &self.inserts.tools.html_scroll,
                div().px(Gap::Xl).py(Gap::L).text_size(px(bench_ui::THREAD_META)).child(
                    TextView::html("tool-html-view-body", self.inserts.tools.preview.clone())
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
        let tool = self.inserts.tools.tool;
        let input_bar = |extra: Vec<AnyElement>, cx: &mut Context<Self>| {
            let mut buttons = vec![
                small_button("tool-paste", "Paste", cx, |this, window, cx| {
                    this.paste_tool_input(window, cx)
                }),
                small_button("tool-sample", "Sample", cx, |this, window, cx| {
                    let s = this.inserts.tools.tool.sample().to_string();
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
                .p(Gap::Xs)
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
        let error = self.inserts.tools.error.clone();
        let input_pane = |extra: Vec<AnyElement>, this: &mut Self, cx: &mut Context<Self>| {
            v_flex()
                .flex_1()
                .min_w_0()
                .child(input_bar(extra, cx))
                .child(editor(&this.inserts.tools.input))
        };
        // Panes fill the height (h_flex centers its children otherwise).
        let split = |left: AnyElement, right: AnyElement| {
            h_flex()
                .size_full()
                .items_stretch()
                .child(left)
                .child(div().w(px(bench_ui::HAIRLINE)).h_full().bg(theme.border))
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
                    .child(editor(&self.inserts.tools.output))
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
                        &self.inserts.tools.preview_scroll,
                        div()
                            .px(Gap::Xl)
                            .py(Gap::L)
                            .text_size(px(bench_ui::THREAD_META))
                            .child(crate::media::text::markdown(
                                "tool-md",
                                self.inserts.tools.preview.clone(),
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
                let current = self.inserts.tools.count(tool);
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
                        bench_ui::segmented_cells(
                            "tool-count",
                            labels,
                            chosen,
                            cx,
                            bench_ui::on_pick(cx, move |this: &mut Self, k, window, cx| {
                                this.inserts.tools.counts.insert(tool, counts_vec[k]);
                                this.run_tool(window, cx)
                            }),
                        )
                        .into_any_element(),
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
                    .child(editor(&self.inserts.tools.output))
                    .into_any_element()
            }
            Kind::Regex => {
                let flags = self.inserts.tools.flags;
                let flag = |k: usize,
                            label: &'static str,
                            on: bool,
                            tip: &'static str,
                            cx: &mut Context<Self>| {
                    bench_ui::chip_tight(("regex-flag", k), cx)
                        .text_sm()
                        .font_family(bench_ui::theme::MONO)
                        .when(on, |d| {
                            d.bg(theme.primary).text_color(theme.primary_foreground)
                        })
                        .child(label)
                        .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.toggle_regex_flag(k, window, cx)
                        }))
                        .into_any_element()
                };
                let pattern_row = h_flex()
                    .flex_none()
                    .px(Gap::L)
                    .py(Gap::M)
                    .gap(Gap::M)
                    .items_center()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(div().flex_1().child(Input::new(&self.inserts.tools.pattern)))
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
                let count = self.inserts.tools.matches;
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
                    .child(editor(&self.inserts.tools.output))
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
                let image = self.inserts.tools.image.clone();
                let title = match &self.inserts.tools.image_info {
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
                            .p(Gap::Xl)
                            .flex()
                            .items_center()
                            .justify_center()
                            .map(|d| match image {
                                Some(path) => d.child(
                                    // Fit to the pane, small images scaled up too.
                                    img(path).size_full().object_fit(ObjectFit::Contain),
                                ),
                                None => d
                                    .text_size(px(bench_ui::THREAD_META))
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
    bench_ui::chip(id, cx)
        .flex_none()
        .text_sm()
        .child(label)
        .on_click(cx.listener(move |this, _, window, cx| on(this, window, cx)))
        .into_any_element()
}
