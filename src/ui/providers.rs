//! Settings → Providers (models, fallback, voice, service keys) and Messages (Telegram).

use gpui_kit::component::button::Button;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;

type Act = Box<dyn Fn(&mut BenchApp, &mut Window, &mut Context<BenchApp>)>;

impl BenchApp {
    pub(crate) fn pv_heading(&self, text: impl Into<SharedString>, cx: &App) -> Div {
        div()
            .pt_6()
            .pb_1()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(text.into())
    }

    /// Title and hint on the left, the control on the right, a rule below.
    pub(crate) fn pv_row(
        &self,
        title: impl Into<SharedString>,
        hint: impl Into<SharedString>,
        control: AnyElement,
        cx: &App,
    ) -> Div {
        let theme = cx.theme();
        h_flex()
            .gap_4()
            .py_2p5()
            .items_center()
            .border_b_1()
            .border_color(theme.border)
            .child(
                v_flex()
                    .w(px(220.))
                    .flex_none()
                    .gap_0p5()
                    .child(div().text_base().child(title.into()))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(hint.into()),
                    ),
            )
            .child(div().flex_1().min_w_0().child(control))
    }

    pub(crate) fn pv_input(&self, input: &Entity<InputState>) -> AnyElement {
        Input::new(input).into_any_element()
    }

    /// A bordered text button; `primary` fills it with the accent.
    pub(crate) fn pv_button(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        primary: bool,
        cx: &mut Context<Self>,
        on: Act,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        div()
            .id(id)
            .flex_none()
            .h(rems(super::BAR_CONTROL_REMS))
            .px_3()
            .flex()
            .items_center()
            .text_base()
            .rounded_md()
            .cursor_pointer()
            .when(primary, |d| {
                d.bg(theme.primary)
                    .text_color(theme.primary_foreground)
                    .hover(|s| s.bg(theme.primary_hover))
            })
            .when(!primary, |d| {
                d.border_1()
                    .border_color(theme.border)
                    .hover(|s| s.bg(theme.list_hover))
            })
            .child(label.into())
            .on_click(cx.listener(move |this, _, window, cx| on(this, window, cx)))
            .into_any_element()
    }

    pub(crate) fn render_providers(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let mut page: Vec<AnyElement> = vec![];

        // ---- tabs: the agents (the main thing), the presets, the look ----
        let tabs = crate::inserts::tools::view::segmented_tall(
            "gear-tabs",
            &["Agents", "Presets", "Appearance"],
            self.gear_tab,
            cx,
            |this, k, _, cx| {
                this.gear_tab = k;
                cx.notify();
            },
        );
        page.push(h_flex().pt_4().child(tabs).into_any_element());
        match self.gear_tab {
            1 => {
                page.extend(self.render_presets(cx));
                return Self::providers_page(page);
            }
            2 => {
                page.push(self.pv_heading("APPEARANCE", cx).into_any_element());
                page.push(self.render_appearance(cx).into_any_element());
                return Self::providers_page(page);
            }
            _ => {}
        }

        // ---- agents: each has its own models and voice (its own network: a model on its
        // own machine is localhost there) ----
        page.push(self.pv_heading("AGENTS", cx).into_any_element());
        for host in self.agent_hosts() {
            let a = &self.agents[&host];
            let current = crate::pi_agent::build();
            let state = match (&a.error, &a.conn) {
                (Some(e), _) => e.clone(),
                (None, None) => "connecting…".into(),
                (None, Some(_)) => match (&a.build, &current) {
                    (Some(b), Some(c)) if b == c => "connected · up to date".into(),
                    (Some(_), Some(_)) => "connected · an older version: updated when idle".into(),
                    _ => "connected".into(),
                },
            };
            let shown = host == self.gear_agent;
            let pick = host.clone();
            // A server's agent can be forgotten here (asked once); this Mac's is always there.
            let armed = self.confirm_delete.as_deref() == Some(format!("agent:{host}").as_str());
            let remove = (!host.is_empty()).then(|| {
                let (gone, label) = (host.clone(), if armed { "Remove from Bench" } else { "Remove" });
                self.pv_button(
                    SharedString::from(format!("gear-agent-rm-{host}")),
                    label,
                    false,
                    cx,
                    Box::new(move |this, w, cx| this.remove_agent(gone.clone(), w, cx)),
                )
            });
            page.push(
                self.pv_row(
                    Self::agent_name(&host),
                    if armed {
                        "Bench forgets it; the agent on the server runs on, its Telegram and schedule too".to_string()
                    } else {
                        state
                    },
                    h_flex()
                        .justify_end()
                        .gap_2()
                        .children(remove)
                        .child(self.pv_button(
                            SharedString::from(format!("gear-agent-{host}")),
                            if shown { "Shown below" } else { "Show" },
                            shown,
                            cx,
                            Box::new(move |this, w, cx| this.show_gear_agent(pick.clone(), w, cx)),
                        ))
                        .into_any_element(),
                    cx,
                )
                .into_any_element(),
            );
        }
        page.extend(self.render_agent_form(cx));

        // The shown agent's settings, once read.
        if !self.pv.loaded {
            page.push(
                div()
                    .py_2p5()
                    .text_base()
                    .text_color(muted)
                    .child(if self.conn_of(&self.gear_agent).is_some() {
                        "Loading…"
                    } else {
                        "Not connected: its settings show once it answers."
                    })
                    .into_any_element(),
            );
            return Self::providers_page(page);
        }

        // ---- custom endpoints ----
        page.push(
            self.pv_heading(
                // One model for every thread of this agent's projects: the main one and the
                // fallback below.
                format!("MODELS · {} · its projects", Self::agent_name(&self.gear_agent)),
                cx,
            )
            .into_any_element(),
        );
        for i in 0..self.pv.endpoints.len() {
            let e = &self.pv.endpoints[i];
            let (name, url, model, context, parallel, key) = (
                e.name.clone(),
                e.url.clone(),
                e.model.clone(),
                e.context.clone(),
                e.parallel.clone(),
                e.key.clone(),
            );
            let current = e.current;
            let saved = !e.id.is_empty();
            let check = e.check.clone();
            let title = if saved {
                e.id.clone()
            } else {
                "New provider".into()
            };
            let key_hint = match &e.key_env {
                Some(name) => format!("In .env as {name}"),
                None => "If the server asks for one".into(),
            };
            let armed = saved && self.pv.armed.as_deref() == Some(e.id.as_str());
            let mut actions = vec![self.pv_button(
                ("ep-delete", i),
                if armed { "Confirm delete" } else { "Delete" },
                false,
                cx,
                Box::new(move |this, w, cx| this.delete_endpoint(i, w, cx)),
            )];
            actions.push(self.pv_button(
                ("ep-check", i),
                "Check",
                false,
                cx,
                Box::new(move |this, w, cx| this.check_endpoint(i, w, cx)),
            ));
            if saved && !current {
                actions.push(self.pv_button(
                    ("ep-default", i),
                    "Make main",
                    false,
                    cx,
                    Box::new(move |this, w, cx| this.make_endpoint_default(i, w, cx)),
                ));
            }
            actions.push(self.pv_button(
                ("ep-save", i),
                "Save",
                true,
                cx,
                Box::new(move |this, w, cx| this.save_endpoint(i, w, cx)),
            ));
            page.push(
                v_flex()
                    .pt_3()
                    .child(
                        h_flex()
                            .gap_2()
                            .pb_1()
                            .items_center()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(title),
                            )
                            .when(current, |d| {
                                d.child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.primary)
                                        .child("main model"),
                                )
                            }),
                    )
                    .child(self.pv_row("Name", "How it is listed", self.pv_input(&name), cx))
                    .child(self.pv_row(
                        "Address",
                        "OpenAI-compatible, ends in /v1",
                        self.pv_input(&url),
                        cx,
                    ))
                    .child(self.pv_row(
                        "Default model",
                        "The model it serves first",
                        self.pv_input(&model),
                        cx,
                    ))
                    .child(self.pv_row("Context length", "Tokens", self.pv_input(&context), cx))
                    .child(self.pv_row(
                        "Parallel requests",
                        "How many its server answers at once; subagents keep to it",
                        self.pv_input(&parallel),
                        cx,
                    ))
                    .child(self.pv_row("API key", key_hint, self.pv_input(&key), cx))
                    .child(
                        h_flex()
                            .py_2()
                            .gap_2()
                            .items_center()
                            .child(div().flex_1().min_w_0().truncate().text_sm().map(|d| {
                                match check {
                                    Some((ok, msg)) => d
                                        .text_color(if ok { muted } else { theme.danger })
                                        .child(msg),
                                    None => d,
                                }
                            }))
                            .children(actions),
                    )
                    .into_any_element(),
            );
        }
        page.push(
            div()
                .id("ep-add")
                .py_2()
                .text_base()
                .text_color(muted)
                .cursor_pointer()
                .hover(|s| s.text_color(theme.foreground))
                .child("+ Add a provider")
                .on_click(cx.listener(|this, _, window, cx| this.add_endpoint(window, cx)))
                .into_any_element(),
        );

        // ---- fallback ----
        page.push(self.pv_heading("FALLBACK", cx).into_any_element());
        let current = self.pv.fallback.clone();
        let label = current
            .as_ref()
            .map(|(p, m)| {
                let group = self
                    .profile_models
                    .iter()
                    .find(|c| c.provider == *p)
                    .map_or(p.clone(), |c| c.group.clone());
                format!("{m}  ·  {group}")
            })
            .unwrap_or_else(|| "None".into());
        let mut choices: Vec<(Option<String>, String, Option<(String, String)>)> =
            vec![(None, "None".into(), None)];
        let mut last_group = String::new();
        for c in self.profile_models.iter().filter(|c| c.tag != "main") {
            let header = (c.group != last_group).then(|| c.group.clone());
            last_group = c.group.clone();
            choices.push((
                header,
                c.model.clone(),
                Some((c.provider.clone(), c.model.clone())),
            ));
        }
        let view = cx.entity();
        let picker = Button::new("fallback")
            .outline()
            .small()
            .label(label)
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu;
                for (header, text, pick) in &choices {
                    if let Some(h) = header {
                        menu = menu.separator().label(h.clone());
                    }
                    let (view, pick2) = (view.clone(), pick.clone());
                    let checked = *pick == current;
                    menu = menu.item(PopupMenuItem::new(text.clone()).checked(checked).on_click(
                        move |_, window, cx| {
                            let pick = pick2.clone();
                            view.update(cx, |this, cx| this.set_fallback(pick, window, cx))
                        },
                    ));
                }
                menu
            });
        page.push(
            self.pv_row(
                "Fallback model",
                "Answers when the main model fails or goes silent",
                h_flex().child(picker).into_any_element(),
                cx,
            )
            .into_any_element(),
        );

        // ---- voice ----
        if let Some(v) = &self.pv.voice {
            let v = (
                v.stt_url.clone(),
                v.stt_model.clone(),
                v.stt_lang.clone(),
                v.stt_key.clone(),
                v.tts_url.clone(),
                v.tts_model.clone(),
                v.tts_voice.clone(),
            );
            page.push(
                self.pv_heading("VOICE · OPENAI-COMPATIBLE SERVERS", cx)
                    .into_any_element(),
            );
            page.push(
                self.pv_row(
                    "Speech to text",
                    "Address of the server",
                    self.pv_input(&v.0),
                    cx,
                )
                .into_any_element(),
            );
            page.push(
                self.pv_row("Model", "", self.pv_input(&v.1), cx)
                    .into_any_element(),
            );
            page.push(
                self.pv_row(
                    "Language",
                    "e.g. en, ru; empty = auto",
                    self.pv_input(&v.2),
                    cx,
                )
                .into_any_element(),
            );
            page.push(
                self.pv_row(
                    "API key",
                    "If the server asks for one",
                    self.pv_input(&v.3),
                    cx,
                )
                .into_any_element(),
            );
            page.push(
                self.pv_row(
                    "Text to speech",
                    "Address of the server",
                    self.pv_input(&v.4),
                    cx,
                )
                .into_any_element(),
            );
            page.push(
                self.pv_row("Model", "", self.pv_input(&v.5), cx)
                    .into_any_element(),
            );
            page.push(
                self.pv_row("Voice", "", self.pv_input(&v.6), cx)
                    .into_any_element(),
            );
            page.push(
                h_flex()
                    .py_2()
                    .child(div().flex_1())
                    .child(self.pv_button(
                        "voice-save",
                        "Save",
                        true,
                        cx,
                        Box::new(|this, w, cx| this.save_voice(w, cx)),
                    ))
                    .into_any_element(),
            );
        }

        // No list of keys here: each key is set on the card or MCP server that reads it.
        Self::providers_page(page)
    }

    fn providers_page(page: Vec<AnyElement>) -> AnyElement {
        v_flex()
            .id("providers")
            .size_full()
            .overflow_y_scroll()
            .px_4()
            .pb_6()
            .child(v_flex().max_w(px(820.)).children(page))
            .into_any_element()
    }
}

impl BenchApp {
    /// "+ Agent on a server", or its form: a host of ~/.ssh/config (or a new one), its name.
    fn render_agent_form(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let muted = cx.theme().muted_foreground;
        let Some(form) = self.agent_form.as_ref() else {
            return vec![h_flex()
                .py_2()
                .child(self.pv_button(
                    "gear-agent-add",
                    "+ Agent on a server",
                    false,
                    cx,
                    Box::new(|this, w, cx| this.open_agent_form(w, cx)),
                ))
                .into_any_element()];
        };
        let (host, name, address, user, port, key) = (
            form.host.clone(),
            form.name.clone(),
            form.address.clone(),
            form.user.clone(),
            form.port.clone(),
            form.key.clone(),
        );
        let mut labels = self.hosts_without_agent();
        let at = host
            .as_ref()
            .and_then(|h| labels.iter().position(|l| l == h))
            .unwrap_or(labels.len());
        labels.push("New server…".into());
        let names = labels.clone();
        let pick = crate::inserts::tools::view::segmented_tall_owned(
            "agent-server",
            labels,
            at,
            cx,
            move |this, k, _, cx| {
                if let Some(f) = this.agent_form.as_mut() {
                    f.host = (k + 1 < names.len()).then(|| names[k].clone());
                }
                cx.notify();
            },
        );
        let mut out = vec![
            self.pv_row(
                "Server",
                "A host from ~/.ssh/config, or a new one",
                h_flex().flex_wrap().child(pick).into_any_element(),
                cx,
            )
            .into_any_element(),
        ];
        if host.is_none() {
            for (title, hint, input) in [
                ("Address", "Its IP or domain", &address),
                ("Login", "The user on it", &user),
                ("Port", "Empty: 22", &port),
                ("Key file", "Empty: ssh-agent", &key),
            ] {
                out.push(self.pv_row(title, hint, self.pv_input(input), cx).into_any_element());
            }
        }
        out.push(self.pv_row("Name", "Its project in the sidebar", self.pv_input(&name), cx).into_any_element());
        out.push(
            div()
                .pt_2()
                .text_sm()
                .text_color(muted)
                .child("Bench installs its agent there over SSH (it needs Node.js 22.18+) and starts it. It keeps its own threads, models, skills and Telegram there and works without this Mac; its models are set here once it answers.")
                .into_any_element(),
        );
        out.push(
            h_flex()
                .pt_3()
                .pb_2()
                .gap_2()
                .child(self.pv_button(
                    "gear-agent-save",
                    "Add agent",
                    true,
                    cx,
                    Box::new(|this, w, cx| this.add_agent(w, cx)),
                ))
                .child(self.pv_button(
                    "gear-agent-cancel",
                    "Cancel",
                    false,
                    cx,
                    Box::new(|this, _, cx| {
                        this.agent_form = None;
                        cx.notify();
                    }),
                ))
                .into_any_element(),
        );
        out
    }
}

impl BenchApp {
    /// The Presets tab: each kind as a few lines, Edit opens its JSON (Save, Built-in, Cancel).
    fn render_presets(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let muted = cx.theme().muted_foreground;
        let p = self.presets.clone();
        let mut out = vec![div()
            .pt_4()
            .text_sm()
            .text_color(muted)
            .child("What a project takes with one click (+ Add in its environment), and the choices of its Model section. Kept by This Mac's agent; a server's projects take them too. Keys can be written right into a preset.")
            .into_any_element()];
        for (kind, title) in crate::app::presets::KINDS {
            let lines: Vec<(String, String)> = match kind {
                "mcp" => p.mcp.iter().map(|(n, t, _)| (n.clone(), t.clone())).collect(),
                "lsp" => p.lsp.iter().map(|(n, t, _)| (n.clone(), t.clone())).collect(),
                "compaction" => p
                    .compaction
                    .iter()
                    .map(|(at, n)| (format!("{}%", (at * 100.).round()), n.clone()))
                    .collect(),
                _ => vec![(p.reasoning.join(", "), "offered in a project's Model section and the composer".into())],
            };
            out.push(self.pv_heading(title.to_uppercase(), cx).into_any_element());
            for (name, note) in lines {
                out.push(
                    h_flex()
                        .gap_2()
                        .py_1p5()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(div().flex_none().child(name))
                        .child(div().flex_1().min_w_0().truncate().text_color(muted).child(note))
                        .into_any_element(),
                );
            }
            let open = self.preset_editor.as_ref().filter(|e| e.kind == kind).map(|e| e.text.clone());
            match open {
                Some(text) => {
                    out.push(div().pt_2().child(crate::env::section::code(&text, cx)).into_any_element());
                    out.push(
                        h_flex()
                            .py_2()
                            .gap_2()
                            .child(self.pv_button(
                                SharedString::from(format!("preset-save-{kind}")),
                                "Save",
                                true,
                                cx,
                                Box::new(|this, w, cx| this.save_presets(false, w, cx)),
                            ))
                            .child(self.pv_button(
                                SharedString::from(format!("preset-reset-{kind}")),
                                "Built-in",
                                false,
                                cx,
                                Box::new(|this, w, cx| this.save_presets(true, w, cx)),
                            ))
                            .child(self.pv_button(
                                SharedString::from(format!("preset-cancel-{kind}")),
                                "Cancel",
                                false,
                                cx,
                                Box::new(|this, _, cx| {
                                    this.preset_editor = None;
                                    cx.notify();
                                }),
                            ))
                            .into_any_element(),
                    );
                }
                None => out.push(
                    h_flex()
                        .py_2()
                        .child(self.pv_button(
                            SharedString::from(format!("preset-edit-{kind}")),
                            "Edit",
                            false,
                            cx,
                            Box::new(move |this, w, cx| this.edit_presets(kind, w, cx)),
                        ))
                        .into_any_element(),
                ),
            }
        }
        out
    }
}
