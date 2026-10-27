//! Settings → Providers (models, fallback, voice, service keys) and Messages (Telegram).

pub(crate) mod agent_form;
pub(crate) mod appearance;
mod controls;
pub(crate) mod new_project;
pub(crate) mod presets;
pub(crate) mod providers;

use gpui_kit::component::button::Button;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;

impl BenchApp {

    pub(crate) fn render_providers(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let mut page: Vec<AnyElement> = vec![];

        // ---- tabs: the agents (the main thing), the presets, the look ----
        let view = cx.entity();
        let pick: bench_ui::OnPick = std::rc::Rc::new(move |k, _, cx| {
            let view = view.clone();
            view.update(cx, |this, cx| {
                this.gear_tab = k;
                cx.notify();
            });
        });
        let tabs = bench_ui::settings_tabs(
            "gear-tabs",
            &[
                (bench_ui::Ico::Bot, "Agents"),
                (bench_ui::Ico::Layers, "Presets"),
                (bench_ui::Ico::Moon, "Appearance"),
            ],
            self.gear_tab,
            cx,
            pick,
        );
        page.push(
            h_flex()
                // the same air over the tabs as the page's inset everywhere
                .pt(px(bench_ui::CHAT_LEFT))
                .child(tabs)
                .into_any_element(),
        );
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
            let current = crate::client::launch::build();
            // A server's agent is linked once this Mac's lists it; until then (and in the demo,
            // which links none) it is not started.
            let (state, older) = match self.agents.get(&host) {
                None => ("not started".into(), false),
                Some(a) => agent_state(
                    a.error.as_deref(),
                    a.conn.is_some(),
                    a.build.as_deref(),
                    current.as_deref(),
                ),
            };
            let shown = host == self.gear_agent;
            let pick = host.clone();
            // A server's agent can be forgotten here (asked once); this Mac's is always there.
            let armed = self.confirm_delete.as_deref() == Some(format!("agent:{host}").as_str());
            // Restarts it from this Bench's sources; an interrupted run resumes (runs are durable).
            let update = older.then(|| {
                let h = host.clone();
                self.pv_button(
                    SharedString::from(format!("gear-agent-up-{host}")),
                    "Update",
                    false,
                    cx,
                    Box::new(move |this, w, cx| this.update_agent(h.clone(), w, cx)),
                )
            });
            let remove = (!host.is_empty()).then(|| {
                let (gone, label) = (
                    host.clone(),
                    if armed { "Remove from Bench" } else { "Remove" },
                );
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
                        .children(update)
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
                format!(
                    "MODELS · {} · its projects",
                    Self::agent_name(&self.gear_agent)
                ),
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
}

/// What the gear says of an agent, and whether it runs an older build than this Bench carries.
/// A build is named by the first 8 of its sources' hash (`pi_agent::source_hash`).
fn agent_state(
    error: Option<&str>,
    connected: bool,
    build: Option<&str>,
    current: Option<&str>,
) -> (String, bool) {
    let short = |b: &str| b.chars().take(8).collect::<String>();
    match (error, connected, build, current) {
        (Some(e), ..) => (e.to_string(), false),
        (None, false, ..) => ("connecting…".into(), false),
        (None, true, Some(b), Some(c)) if b == c => (
            format!("connected · build {} · up to date", short(b)),
            false,
        ),
        // Reattached while it was busy (`Connection::ssh`): nothing updates it until it is
        // restarted — Update here, or Bench's next start.
        (None, true, Some(b), Some(c)) => (
            format!(
                "connected · build {}, older than this Bench's {}",
                short(b),
                short(c)
            ),
            true,
        ),
        (None, true, Some(b), None) => (format!("connected · build {}", short(b)), false),
        (None, true, None, _) => ("connected".into(), false),
    }
}

#[cfg(test)]
mod tests {
    use super::agent_state;

    #[test]
    fn the_gear_names_an_agents_build_and_says_when_it_is_older() {
        let now = "3542f52d86a3399e";
        assert_eq!(
            agent_state(None, true, Some(now), Some(now)),
            ("connected · build 3542f52d · up to date".into(), false)
        );
        assert_eq!(
            agent_state(None, true, Some("1a2b3c4d5e6f7a8b"), Some(now)),
            (
                "connected · build 1a2b3c4d, older than this Bench's 3542f52d".into(),
                true
            )
        );
        assert_eq!(
            agent_state(None, false, None, Some(now)),
            ("connecting…".into(), false)
        );
        assert_eq!(
            agent_state(
                Some("dev: SSH could not connect"),
                true,
                Some("x"),
                Some(now)
            )
            .1,
            false
        );
    }
}
