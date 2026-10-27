//! What a project's agent reads as text, as the environment lists it: its own prompts and the
//! skills its threads can use. The texts themselves — the soul (`SYSTEM.md`), each prompt, each
//! skill's files — are files, opened and saved in the Explorer (`src/explorer/`).

use serde_json::Value;

/// A skill a project's threads can use (its folder named after it): its front matter's
/// description, where it lives
/// ("project": its own, "repo": its folder's .agents/skills or .claude/skills, "general": the
/// Host's, seen in Host alone), and its folder.
#[derive(Clone, Debug, Default)]
pub struct SkillInfo {
    pub description: String,
    pub scope: String,
    pub dir: String,
}

#[derive(Default)]
pub struct InstructionsEnv {
    /// Its own prompts' names (`<dir>/prompts/<name>.md`).
    pub prompts: Vec<String>,
    /// Every skill its threads can use.
    pub skills: Vec<SkillInfo>,
}

impl InstructionsEnv {
    /// Its part of the agent's answer (`/api/projects/<id>/env`).
    pub fn apply(&mut self, v: &Value) {
        self.prompts = v["prompts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p.as_str().map(str::to_string))
            .collect();
        self.skills = v["skills"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|k| {
                let s = |f: &str| k[f].as_str().unwrap_or_default().to_string();
                SkillInfo {
                    description: s("description"),
                    scope: s("scope"),
                    dir: s("dir"),
                }
            })
            .collect();
    }
}
