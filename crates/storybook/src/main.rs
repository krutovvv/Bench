//! The storybook: every `bench_ui` element in every state, in one window.
//!
//! `cargo run -p storybook` (light) or `cargo run -p storybook -- --dark`.
//! `crates/storybook/shot.sh` takes both pictures into `crates/storybook/reference/`.

use std::rc::Rc;

use bench_ui::theme::{self, Appearance, Mode};
use bench_ui::*;
use gpui_kit::component::input::InputState;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::*;

struct Story {
    name: Entity<InputState>,
    password: Entity<InputState>,
    search: Entity<InputState>,
    empty: Entity<InputState>,
    picked: usize,
    tab_open: usize,
    armed: bool,
}

impl Story {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = |text: &str, hint: &str, window: &mut Window, cx: &mut Context<Self>| {
            let (text, hint) = (text.to_string(), hint.to_string());
            cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(hint)
                    .default_value(text)
            })
        };
        Self {
            name: input("Dev server", "Name", window, cx),
            password: cx.new(|cx| secret_state(window, cx).default_value("secret")),
            search: input("", "Search sessions", window, cx),
            empty: input("", "Empty field", window, cx),
            picked: 1,
            tab_open: 0,
            armed: false,
        }
    }
}

/// A section of the page: its name, then what it shows.
fn section(name: &'static str, body: impl IntoElement, cx: &App) -> Div {
    stack(Gap::M)
        .child(title(cx).text_lg().child(name))
        .child(body)
}

impl Render for Story {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let view = cx.entity();
        let tab_view = view.clone();
        let tab_pick: OnPick = Rc::new(move |k, _, cx| {
            let view = tab_view.clone();
            view.update(cx, |this, cx| {
                this.tab_open = k;
                cx.notify();
            })
        });
        let pick: OnPick = Rc::new(move |k, _, cx| {
            view.update(cx, |this, cx| {
                this.picked = k;
                cx.notify();
            })
        });

        let text_section = stack(Gap::S)
            .child(body(cx).child("Body — the thread's text: messages, answers, the composer."))
            .child(title(cx).child("Title — a block's or a form's name"))
            .child(muted(cx).child("Muted — meta, hints, empty states"))
            .child(small(cx).child("Small — chips, captions, pane strips"))
            .child(mono(cx).child("mono — src/ui/chat.rs:42"))
            .child(error(cx).child("Error — said in place"));

        let buttons = stack(Gap::M)
            .child(
                row(Gap::M)
                    .child(primary_button("p", "Save"))
                    .child(secondary_button("s", "Check"))
                    .child(ghost_button("g", "Cancel"))
                    .child(danger_button("d", "Delete"))
                    .child(field(&self.empty).w(px(160.))),
            )
            .child(
                row(Gap::M)
                    .child(form_action("fa1", "Save", true, cx))
                    .child(form_action("fa2", "Revert", false, cx))
                    .child(compact_button("c", "Copy"))
                    .child(icon_button("i1", Ico::Settings, "Settings", false, cx))
                    .child(icon_button("i2", Ico::PanelLeft, "Sidebar (on)", true, cx))
                    .child(arm_delete_button("a1", false, cx))
                    .child(arm_delete_button("a2", true, cx)),
            );

        let fields = stack(Gap::S)
            .child(field_row("Name", field(&self.name), cx))
            .child(field_row("Password", secret_field(&self.password), cx))
            .child(field_row("Search", search_field(&self.search, cx), cx))
            .child(field_row("Empty", field(&self.empty), cx));

        let gaps = stack(Gap::S).children([Gap::Xs, Gap::S, Gap::M, Gap::L, Gap::Xl].map(|g| {
            row(g)
                .child(small(cx).w(px(40.)).child(format!("{g:?}")))
                .children((0..4).map(|_| {
                    div()
                        .size(px(16.))
                        .rounded_sm()
                        .bg(theme.primary.opacity(0.5))
                }))
        }));

        let blocks = stack(Gap::M)
            .child(block(
                "/terminal · local",
                vec![icon_button("b1", Ico::RefreshCw, "Restart", false, cx).into_any_element()],
                Some(
                    block_body()
                        .child(mono(cx).child("$ cargo build\n   Finished dev"))
                        .into_any_element(),
                ),
                cx,
            ))
            .child(block("A block with a head only", vec![], None, cx))
            .child(
                row(Gap::M)
                    .child(avatar(false, &theme))
                    .child(avatar(true, &theme))
                    .child(empty_state("Nothing here yet.", cx)),
            );

        let forms = stack(Gap::L)
            .child(inline_form(
                "New server",
                vec![
                    ("Name", field(&self.name).into_any_element()),
                    ("Password", secret_field(&self.password).into_any_element()),
                ],
                Some("Kept in this project only.".into()),
                vec![
                    form_action("f-save", "Add", true, cx).into_any_element(),
                    form_action("f-cancel", "Cancel", false, cx).into_any_element(),
                ],
                cx,
            ))
            .child(settings_row(
                "Appearance",
                "Light, dark or as the system",
                segmented("seg", &["Light", "Dark", "System"], self.picked, cx, pick)
                    .into_any_element(),
                cx,
            ))
            .child(
                settings_tabs(
                    "stab",
                    &[(Ico::Bot, "Agents"), (Ico::Layers, "Presets"), (Ico::Moon, "Appearance")],
                    self.tab_open,
                    cx,
                    tab_pick,
                )
                .w(px(560.)),
            );

        let lists_view = cx.entity();
        let lists = v_flex()
            .w(px(280.))
            .bg(theme.sidebar)
            .rounded_md()
            .py_2()
            .child(list_row(
                "r1",
                "Отладка вставки при выборе утилиты",
                None,
                true,
                Some(false),
                None,
                cx,
            ))
            .child(list_row(
                "r2",
                "Increase scroll gap",
                Some("2 h ago".into()),
                false,
                Some(false),
                None,
                cx,
            ))
            .child(
                list_row(
                    "r3",
                    "A row with an action on hover",
                    None,
                    false,
                    Some(true),
                    Some(arm_delete_button("r3-del", self.armed, cx).into_any_element()),
                    cx,
                )
                .on_click(move |_, _, cx| {
                    lists_view.update(cx, |this, cx| {
                        this.armed = !this.armed;
                        cx.notify();
                    })
                }),
            )
            .child(pick_row("pk", Ico::Server, "dev", "root@10.0.0.2", cx))
            .child(
                choice_row("ch1", cx).child("A choice of the agent's question — as wide as the card, its text wrapped."),
            );

        let files = row(Gap::L).children(
            [
                "main.rs",
                "app.tsx",
                "Cargo.toml",
                "notes.md",
                "shot.png",
                "src",
            ]
            .map(|n| {
                let (icon, color) = file_icon(n, n == "src", &theme);
                row(Gap::S)
                    .child(icon.icon().text_color(color))
                    .child(small(cx).child(n))
            }),
        );

        let bars = stack(Gap::M)
            .child(header_bar(
                "Отладка вставки при выборе утилиты",
                icon_button("hb", Ico::PanelLeft, "Sidebar", false, cx).into_any_element(),
                cx,
            ))
            .child(pane_bar(
                "Output",
                None,
                vec![compact_button("pb", "Copy").into_any_element()],
                cx,
            ))
            .child(pane_bar(
                "Output",
                Some("Invalid JSON at line 3".into()),
                vec![],
                cx,
            ))
            .child(
                row(Gap::Xs)
                    .h(px(STATUS_H))
                    .bg(theme.status_bar)
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(status_text("63k / 131k · 49%"))
                    .child(spacer())
                    .child(status_button("sb", Ico::Settings, "Settings", false, cx)),
            )
            .child(
                div()
                    .w(px(280.))
                    .bg(theme.sidebar)
                    .child(search_band(&self.search, cx)),
            );

        let icons = row(Gap::M).flex_wrap().children(
            Ico::ALL
                .iter()
                .map(|i| i.icon().text_color(theme.muted_foreground)),
        );

        div()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                v_flex()
                    .id("page")
                    .size_full()
                    .overflow_y_scrollbar()
                    .child(
                        row(Gap::Xl)
                            .items_start()
                            .p(px(32.))
                            .gap(px(48.))
                            .child(
                                stack(Gap::Xl)
                                    .flex_1()
                                    .child(section("Text", text_section, cx))
                                    .child(section(
                                        "Buttons (a form's are as tall as its field)",
                                        buttons,
                                        cx,
                                    ))
                                    .child(section("Fields", fields, cx))
                                    .child(section("Gaps", gaps, cx))
                                    .child(section("Blocks", blocks, cx)),
                            )
                            .child(
                                stack(Gap::Xl)
                                    .flex_1()
                                    .child(section("Forms", forms, cx))
                                    .child(section("Lists", lists, cx))
                                    .child(section("File icons", files, cx))
                                    .child(section("Bars", bars, cx))
                                    .child(section("Icons", icons, cx)),
                            ),
                    ),
            )
    }
}

fn main() {
    let dark = std::env::args().any(|a| a == "--dark") || std::env::var("STORYBOOK_DARK").is_ok();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            let look = Appearance {
                mode: if dark { Mode::Dark } else { Mode::Light },
                ..Default::default()
            };
            theme::apply(look, None, cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1560.), px(1000.)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Storybook".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            if let Err(e) = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| Story::new(window, cx))
            }) {
                eprintln!("open window: {e:#}");
            }
            cx.activate(true);
        });
}
