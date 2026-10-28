//! What the gear keeps while it is open: the agent it shows, its tab, that agent's providers, the
//! presets and the one being edited, the form adding a server's agent.

use super::agent_form::AgentForm;
use super::presets::{PresetEditor, Presets};
use super::providers::Providers;

#[derive(Default)]
pub struct Gear {
    /// The agent whose models and voice it shows ("" = this Mac's).
    pub agent: String,
    /// Its tab: agents, presets, appearance.
    pub tab: usize,
    /// That agent's model providers, keys and messaging (`providers.rs`).
    pub providers: Providers,
    /// The person's presets (this Mac's agent's) and the kind open to edit.
    pub presets: Presets,
    pub preset_editor: Option<PresetEditor>,
    /// "+ Agent on a server", while open (`agent_form.rs`).
    pub agent_form: Option<AgentForm>,
}
