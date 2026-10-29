//! Settings → Providers (models, fallback, voice, service keys) and Messages (Telegram).

pub(crate) mod state;
pub(crate) mod agent_form;
pub(crate) mod appearance;
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
    pub(crate) fn render_settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let mut page: Vec<AnyElement> = vec![];

        // ---- tabs: the agents (the main thing), the presets, the look ----
        let view = cx.entity();
        let pick: bench_ui::OnPick = std::rc::Rc::new(move |k, _, cx| {
            let view = view.clone();
            view.update(cx, |this, cx| {
                this.gear.tab = k;
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
            self.gear.tab,
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
        match self.gear.tab {
            1 => {
                page.extend(self.render_presets(cx));
                return bench_ui::settings_page("settings", page);
            }
            2 => {
                page.push(bench_ui::settings_heading("Appearance", vec![], cx).into_any_element());
                page.push(self.render_appearance(cx).into_any_element());
                return bench_ui::settings_page("settings", page);
            }
            _ => {}
        }

        // ---- agents: each has its own models and voice (its own network: a model on its
        // own machine is localhost there) ----
        page.push(bench_ui::settings_heading("Agents", vec![], cx).into_any_element());
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
            let shown = host == self.gear.agent;
            let pick = host.clone();
            // A server's agent can be forgotten here (asked once); this Mac's is always there.
            let armed = self.armed.is(&format!("agent:{host}"));
            // Restarts it from this Bench's sources; an interrupted run resumes (runs are durable).
            let update = older.then(|| {
                let h = host.clone();
                bench_ui::secondary_button(
                    SharedString::from(format!("gear-agent-up-{host}")),
                    "Update",
                )
                .small()
                .on_click(cx.listener(move |this, _, w, cx| this.update_agent(h.clone(), w, cx)))
                .into_any_element()
            });
            let remove = (!host.is_empty()).then(|| {
                let (gone, label) = (
                    host.clone(),
                    if armed { "Remove from Bench" } else { "Remove" },
                );
                bench_ui::secondary_button(
                    SharedString::from(format!("gear-agent-rm-{host}")),
                    label,
                )
                .small()
                .on_click(cx.listener(move |this, _, w, cx| this.remove_agent(gone.clone(), w, cx)))
                .into_any_element()
            });
            page.push(
                bench_ui::settings_field(
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
                        .child((if shown { bench_ui::primary_button } else { bench_ui::secondary_button })(SharedString::from(format!("gear-agent-{host}")), if shown { "Shown below" } else { "Show" })
.small()
.on_click(cx.listener(move |this, _, w, cx| this.show_gear_agent(pick.clone(), w, cx)))
.into_any_element())
                        .into_any_element(),
                    cx,
                )
                .into_any_element(),
            );
        }
        page.extend(self.render_agent_form(cx));

        // The shown agent's settings, once read.
        if !self.gear.providers.loaded {
            page.push(
                div()
                    .py_2p5()
                    .text_base()
                    .text_color(muted)
                    .child(if self.conn_of(&self.gear.agent).is_some() {
                        "Loading…"
                    } else {
                        "Not connected: its settings show once it answers."
                    })
                    .into_any_element(),
            );
            return bench_ui::settings_page("settings", page);
        }

        // ---- custom endpoints ----
        page.push(
            bench_ui::settings_heading(
                // One model for every thread of this agent's projects: the main one and the
                // fallback below.
                format!(
                    "Models · {} · its projects",
                    Self::agent_name(&self.gear.agent)
                ),
                vec![],
                cx,
            )
            .into_any_element(),
        );
        for i in 0..self.gear.providers.endpoints.len() {
            let e = &self.gear.providers.endpoints[i];
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
            let armed = saved && self.armed.is(&format!("endpoint:{}", e.id));
            let mut actions = vec![
                bench_ui::secondary_button(
                    ("ep-delete", i),
                    if armed { "Confirm delete" } else { "Delete" },
                )
                .small()
                .on_click(cx.listener(move |this, _, w, cx| this.delete_endpoint(i, w, cx)))
                .into_any_element(),
            ];
            actions.push(
                bench_ui::secondary_button(("ep-check", i), "Check")
                    .small()
                    .on_click(cx.listener(move |this, _, w, cx| this.check_endpoint(i, w, cx)))
                    .into_any_element(),
            );
            if saved && !current {
                actions.push(
                    bench_ui::secondary_button(("ep-default", i), "Make main")
                        .small()
                        .on_click(
                            cx.listener(move |this, _, w, cx| this.make_endpoint_default(i, w, cx)),
                        )
                        .into_any_element(),
                );
            }
            actions.push(
                bench_ui::primary_button(("ep-save", i), "Save")
                    .small()
                    .on_click(cx.listener(move |this, _, w, cx| this.save_endpoint(i, w, cx)))
                    .into_any_element(),
            );
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
                                    .font_weight(FontWeight::SEMIBOLD)
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
                    .child(bench_ui::settings_field(
                        "Name",
                        "How it is listed",
                        gpui_kit::component::input::Input::new(&name).into_any_element(),
                        cx,
                    ))
                    .child(bench_ui::settings_field(
                        "Address",
                        "OpenAI-compatible, ends in /v1",
                        gpui_kit::component::input::Input::new(&url).into_any_element(),
                        cx,
                    ))
                    .child(bench_ui::settings_field(
                        "Default model",
                        "The model it serves first",
                        gpui_kit::component::input::Input::new(&model).into_any_element(),
                        cx,
                    ))
                    .child(bench_ui::settings_field(
                        "Context length",
                        "Tokens",
                        gpui_kit::component::input::Input::new(&context).into_any_element(),
                        cx,
                    ))
                    .child(bench_ui::settings_field(
                        "Parallel requests",
                        "How many its server answers at once; subagents keep to it",
                        gpui_kit::component::input::Input::new(&parallel).into_any_element(),
                        cx,
                    ))
                    .child(bench_ui::settings_field(
                        "API key",
                        key_hint,
                        gpui_kit::component::input::Input::new(&key).into_any_element(),
                        cx,
                    ))
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
                .text_size(px(bench_ui::SIDEBAR_TEXT))
                .text_color(theme.primary)
                .cursor_pointer()
                .hover(|s| s.opacity(0.8))
                .child("Add a provider…")
                .on_click(cx.listener(|this, _, window, cx| this.add_endpoint(window, cx)))
                .into_any_element(),
        );

        // ---- fallback ----
        page.push(bench_ui::settings_heading("Fallback", vec![], cx).into_any_element());
        let current = self.gear.providers.fallback.clone();
        let label = current
            .as_ref()
            .map(|(p, m)| {
                let group = self
                    .defaults.models
                    .iter()
                    .find(|c| c.provider == *p)
                    .map_or(p.clone(), |c| c.group.clone());
                format!("{m}  ·  {group}")
            })
            .unwrap_or_else(|| "None".into());
        let mut choices: Vec<(Option<String>, String, Option<(String, String)>)> =
            vec![(None, "None".into(), None)];
        let mut last_group = String::new();
        for c in self.defaults.models.iter().filter(|c| c.tag != "main") {
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
            bench_ui::settings_field(
                "Fallback model",
                "Answers when the main model fails or goes silent",
                h_flex().child(picker).into_any_element(),
                cx,
            )
            .into_any_element(),
        );

        // ---- voice ----
        if let Some(v) = &self.gear.providers.voice {
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
                bench_ui::settings_heading("Voice · OpenAI-compatible servers", vec![], cx)
                    .into_any_element(),
            );
            page.push(
                bench_ui::settings_field(
                    "Speech to text",
                    "Address of the server",
                    gpui_kit::component::input::Input::new(&v.0).into_any_element(),
                    cx,
                )
                .into_any_element(),
            );
            page.push(
                bench_ui::settings_field(
                    "Model",
                    "",
                    gpui_kit::component::input::Input::new(&v.1).into_any_element(),
                    cx,
                )
                .into_any_element(),
            );
            page.push(
                bench_ui::settings_field(
                    "Language",
                    "e.g. en, ru; empty = auto",
                    gpui_kit::component::input::Input::new(&v.2).into_any_element(),
                    cx,
                )
                .into_any_element(),
            );
            page.push(
                bench_ui::settings_field(
                    "API key",
                    "If the server asks for one",
                    gpui_kit::component::input::Input::new(&v.3).into_any_element(),
                    cx,
                )
                .into_any_element(),
            );
            page.push(
                bench_ui::settings_field(
                    "Text to speech",
                    "Address of the server",
                    gpui_kit::component::input::Input::new(&v.4).into_any_element(),
                    cx,
                )
                .into_any_element(),
            );
            page.push(
                bench_ui::settings_field(
                    "Model",
                    "",
                    gpui_kit::component::input::Input::new(&v.5).into_any_element(),
                    cx,
                )
                .into_any_element(),
            );
            page.push(
                bench_ui::settings_field(
                    "Voice",
                    "",
                    gpui_kit::component::input::Input::new(&v.6).into_any_element(),
                    cx,
                )
                .into_any_element(),
            );
            // Bench's own choice, not the agent's: saved at once, as a switch is.
            page.push(
                bench_ui::settings_field(
                    "Text while speaking",
                    "Each pause in dictation sends what was said, so the words come in as you speak (this computer's Bench)",
                    div()
                        .flex()
                        .justify_end()
                        .child(
                            gpui_kit::component::switch::Switch::new("voice-live")
                                .checked(self.media.live_pref)
                                .on_click(cx.listener(|this, on: &bool, _, cx| {
                                    this.media.live_pref = *on;
                                    crate::media::state::DictationPrefs { live: *on }.save();
                                    cx.notify();
                                })),
                        )
                        .into_any_element(),
                    cx,
                )
                .into_any_element(),
            );
            page.push(
                h_flex()
                    .py_2()
                    .child(div().flex_1())
                    .child(
                        bench_ui::primary_button("voice-save", "Save")
                            .small()
                            .on_click(cx.listener(|this, _, w, cx| this.save_voice(w, cx)))
                            .into_any_element(),
                    )
                    .into_any_element(),
            );
        }

        // ---- the evaluator ----
        if let Some(j) = &self.gear.providers.judge {
            let (on, url, model, key) = (j.enabled, j.url.clone(), j.model.clone(), j.key.clone());
            let check = j.check.clone();
            let muted = cx.theme().muted_foreground;
            let danger = cx.theme().danger;
            page.push(
                bench_ui::settings_heading("Evaluator · System One API", vec![], cx)
                    .into_any_element(),
            );
            page.push(
                bench_ui::settings_field(
                    "Connect the evaluator",
                    "Projects that turn it on in their Settings (Model → Evaluator) use it: a turn's skill and tools, what long results need, searches, the plan, the course, failures, repeats, risky commands, the answer. Off: none does",
                    div()
                        .flex()
                        .justify_end()
                        .child(
                            gpui_kit::component::switch::Switch::new("judge-on")
                                .checked(on)
                                .on_click(cx.listener(|this, on: &bool, w, cx| {
                                    this.set_judge(*on, w, cx)
                                })),
                        )
                        .into_any_element(),
                    cx,
                )
                .into_any_element(),
            );
            page.push(
                bench_ui::settings_field(
                    "Server",
                    "Its address; /v1/systemone is added",
                    gpui_kit::component::input::Input::new(&url).into_any_element(),
                    cx,
                )
                .into_any_element(),
            );
            page.push(
                bench_ui::settings_field(
                    "Model",
                    "",
                    gpui_kit::component::input::Input::new(&model).into_any_element(),
                    cx,
                )
                .into_any_element(),
            );
            page.push(
                bench_ui::settings_field(
                    "API key",
                    "If the server asks for one",
                    gpui_kit::component::input::Input::new(&key).into_any_element(),
                    cx,
                )
                .into_any_element(),
            );
            page.push(
                h_flex()
                    .py_2()
                    .gap_2()
                    .items_center()
                    .child(div().flex_1().min_w_0().truncate().text_sm().map(|d| match check {
                        Some((ok, msg)) => d.text_color(if ok { muted } else { danger }).child(msg),
                        None => d,
                    }))
                    .child(
                        bench_ui::secondary_button("judge-check", "Check")
                            .small()
                            .on_click(cx.listener(|this, _, w, cx| this.check_judge(w, cx)))
                            .into_any_element(),
                    )
                    .child(
                        bench_ui::primary_button("judge-save", "Save")
                            .small()
                            .on_click(cx.listener(|this, _, w, cx| this.save_judge(w, cx)))
                            .into_any_element(),
                    )
                    .into_any_element(),
            );
        }

        // No list of keys here: each key is set on the card or MCP server that reads it.
        bench_ui::settings_page("settings", page)
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
