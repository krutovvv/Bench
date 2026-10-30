//! The gear (Bench → Settings…, ⌘,): its tabs Agents (each agent's models, voice and
//! evaluator; servers' agents added and removed), Presets and Appearance.

pub(crate) mod state;
pub(crate) mod agent_form;
pub(crate) mod appearance;
pub(crate) mod evaluator;
pub(crate) mod new_project;
pub(crate) mod presets;
pub(crate) mod providers;
pub(crate) mod server_form;
pub(crate) mod voice;

use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
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

        page.extend(self.render_providers(cx));
        page.extend(self.render_voice(cx));
        page.extend(self.render_evaluator(cx));

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
