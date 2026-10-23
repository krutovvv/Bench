//! Tools mode's views: the list of tools in the sidebar (groups like the servers'), the open
//! tool's header (name, a hint, its modes) and its body — input | output side by side, a
//! Markdown preview, a generator, the regex tester or the Base64 image.

use gpui_kit::component::input::{Editor, Input};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::HermesApp;
use crate::devtools::{GROUPS, Kind, Tool};
use crate::icons::Ico;

fn group_icon(name: &str) -> Ico {
    match name {
        "Markdown" => Ico::FileText,
        "Generate" => Ico::Pencil,
        "Encode" => Ico::FileCode,
        "Convert" => Ico::RefreshCw,
        _ => Ico::Search,
    }
}

impl HermesApp {
    pub(super) fn render_tools_sidebar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let compact = self.appearance.compact_sidebar;
        let q = self.tools.query.clone();
        let mut list: Vec<AnyElement> = vec![];
        for (g, group) in GROUPS.iter().enumerate() {
            let tools: Vec<Tool> = group
                .tools
                .iter()
                .copied()
                .filter(|t| q.is_empty() || t.name().to_lowercase().contains(&q))
                .collect();
            if tools.is_empty() {
                continue;
            }
            list.push(
                h_flex()
                    .px_4()
                    .pt(rems(if list.is_empty() { 0.5 } else { 0.75 }))
                    .pb_1()
                    .gap_1p5()
                    .items_center()
                    .text_sm()
                    .text_color(muted)
                    .child(group_icon(group.name).icon().small())
                    .child(div().font_weight(FontWeight::MEDIUM).child(group.name))
                    .into_any_element(),
            );
            let n = tools.len();
            for (k, tool) in tools.into_iter().enumerate() {
                let selected = self.tools.tool == tool;
                list.push(
                    h_flex()
                        .id(("tool-row", g * 100 + k))
                        .relative()
                        .pl(super::nest_pl(cx))
                        .child(super::nest_guide(cx, k + 1 == n))
                        .pr_3()
                        .map(|d| if compact { d.py_1() } else { d.py_2() })
                        .border_l_2()
                        .border_color(if selected {
                            theme.primary
                        } else {
                            theme.transparent
                        })
                        .when(selected, |d| d.bg(theme.sidebar_accent))
                        .hover(|s| s.bg(theme.sidebar_accent))
                        .cursor_pointer()
                        .child(div().text_sm().truncate().child(tool.name()))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.select_tool(tool, window, cx)
                        }))
                        .into_any_element(),
                );
            }
        }
        v_flex()
            .size_full()
            .bg(theme.sidebar)
            .child(
                v_flex().flex_none().pt(px(38.)).child(
                    h_flex()
                        .px_4()
                        .pb_2()
                        .items_center()
                        .border_b_1()
                        .border_color(theme.sidebar_border)
                        .child(
                            Input::new(&self.tools.search)
                                .cleanable(true)
                                .prefix(Ico::Search.icon().small().text_color(muted)),
                        ),
                ),
            )
            .child(
                v_flex()
                    .id("tools-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .py_1()
                    .children(list),
            )
            .into_any_element()
    }

    pub(super) fn render_tools_header(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let tool = self.tools.tool;
        let mode = self.tools.mode(tool);
        let modes = tool.modes();
        h_flex()
            .id("header")
            .h(px(36.))
            .flex_none()
            .bg(theme.title_bar)
            .border_b_1()
            .border_color(theme.border)
            .pl_4()
            .pr_2()
            .gap_3()
            .items_center()
            .child(
                div()
                    .flex_none()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(tool.name()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(tool.hint()),
            )
            .when(!modes.is_empty(), |d| {
                d.child(segmented(
                    "tool-mode",
                    modes,
                    mode,
                    cx,
                    |this, k, window, cx| this.set_tool_mode(k, window, cx),
                ))
            })
            .child(self.render_header_tail(cx))
            .into_any_element()
    }

    pub(super) fn render_tools_body(&mut self, cx: &mut Context<Self>) -> AnyElement {
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
        let copy = |cx: &mut Context<Self>| {
            small_button("tool-copy", "Copy", cx, |this, _, cx| {
                this.copy_tool_output(cx)
            })
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
                let left = input_pane(vec![], self, cx).into_any_element();
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
                    .child(
                        div()
                            .id("tool-preview")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .px_4()
                            .py_3()
                            .text_sm()
                            .child(super::media::markdown(
                                "tool-md",
                                self.tools.preview.clone(),
                            )),
                    )
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
                            .text_xs()
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
                        .text_xs()
                        .font_family("Menlo")
                        .border_1()
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
                                    .text_sm()
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
fn pane_bar(
    title: &str,
    error: Option<String>,
    buttons: Vec<AnyElement>,
    cx: &mut Context<HermesApp>,
) -> AnyElement {
    let theme = cx.theme();
    h_flex()
        .flex_none()
        .h(px(36.))
        .px_3()
        .gap_2()
        .items_center()
        .border_b_1()
        .border_color(theme.border)
        .text_xs()
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
fn small_button(
    id: &'static str,
    label: &'static str,
    cx: &mut Context<HermesApp>,
    on: impl Fn(&mut HermesApp, &mut Window, &mut Context<HermesApp>) + 'static,
) -> AnyElement {
    let theme = cx.theme().clone();
    div()
        .id(id)
        .flex_none()
        .px_2()
        .py_0p5()
        .text_xs()
        .border_1()
        .border_color(theme.border)
        .cursor_pointer()
        .hover(|s| s.bg(theme.list_hover))
        .child(label)
        .on_click(cx.listener(move |this, _, window, cx| on(this, window, cx)))
        .into_any_element()
}

/// Pick one: square cells sharing borders, the chosen one in the accent.
fn segmented(
    id: &'static str,
    labels: &[&'static str],
    chosen: usize,
    cx: &mut Context<HermesApp>,
    on: impl Fn(&mut HermesApp, usize, &mut Window, &mut Context<HermesApp>) + Clone + 'static,
) -> AnyElement {
    segmented_owned(
        id,
        labels.iter().map(|s| s.to_string()).collect(),
        chosen,
        cx,
        on,
    )
}

fn segmented_owned(
    id: &'static str,
    labels: Vec<String>,
    chosen: usize,
    cx: &mut Context<HermesApp>,
    on: impl Fn(&mut HermesApp, usize, &mut Window, &mut Context<HermesApp>) + Clone + 'static,
) -> AnyElement {
    let theme = cx.theme().clone();
    h_flex()
        .flex_none()
        .border_1()
        .border_color(theme.border)
        .children(labels.into_iter().enumerate().map(|(k, label)| {
            let on = on.clone();
            div()
                .id((id, k))
                .px_2()
                .py_0p5()
                .text_xs()
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
