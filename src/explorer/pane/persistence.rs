//! What the Explorer was left with, kept on this Mac (`explorer.json` in the state folder), as
//! Zed's workspace persistence brings a workspace back: the open tabs and the one shown, each
//! project's open folders and groups. Read the first time the Explorer opens after a start (its
//! agents are reachable by then), written whenever one of them changes.

use std::collections::{HashMap, HashSet};

use gpui_kit::*;
use serde::{Deserialize, Serialize};

use crate::explorer::{DocKey, FileRef};
use crate::app::BenchApp;
use crate::client::connection;

/// A tab as kept: a `DocKey` with its section by name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Tab {
    File {
        project: String,
        machine: Option<String>,
        path: String,
    },
    Settings {
        project: String,
    },
    Part {
        project: String,
        section: String,
        name: String,
    },
    Problems {
        project: String,
    },
    Diff {
        project: String,
        machine: Option<String>,
        path: String,
    },
}

impl Tab {
    fn of(k: &DocKey) -> Tab {
        match k.clone() {
            DocKey::File(f) => Tab::File {
                project: f.project,
                machine: f.machine,
                path: f.path,
            },
            DocKey::Settings { project } => Tab::Settings { project },
            DocKey::Part {
                project,
                kind,
                name,
            } => Tab::Part {
                project,
                section: kind.to_string(),
                name,
            },
            DocKey::Problems { project } => Tab::Problems { project },
            DocKey::Diff(f) => Tab::Diff {
                project: f.project,
                machine: f.machine,
                path: f.path,
            },
        }
    }

    /// Its key again; None: a section Bench no longer has.
    fn key(self) -> Option<DocKey> {
        Some(match self {
            Tab::File {
                project,
                machine,
                path,
            } => DocKey::File(FileRef {
                project,
                machine,
                path,
            }),
            Tab::Settings { project } => DocKey::Settings { project },
            Tab::Part {
                project,
                section,
                name,
            } => DocKey::Part {
                project,
                kind: crate::env::part(&section)?.key(),
                name,
            },
            Tab::Problems { project } => DocKey::Problems { project },
            Tab::Diff {
                project,
                machine,
                path,
            } => DocKey::Diff(FileRef {
                project,
                machine,
                path,
            }),
        })
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Kept {
    #[serde(default)]
    tabs: Vec<Tab>,
    #[serde(default)]
    active: Option<Tab>,
    /// Each project's open nodes of the tree (`tree.rs` names them).
    #[serde(default)]
    open: HashMap<String, Vec<String>>,
}

fn path() -> std::path::PathBuf {
    connection::state_dir().join("explorer.json")
}

/// The nodes a project's tree opens with when nothing is kept for it.
pub fn first_open() -> HashSet<String> {
    ["files", "instructions"].map(String::from).into()
}

impl BenchApp {
    /// The first opening after a start: the kept tabs opened again, each project's folds back.
    pub(crate) fn restore_explorer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.explorer.restored {
            return;
        }
        self.explorer.restored = true;
        let kept: Kept = std::fs::read(path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        self.explorer.folds = kept
            .open
            .into_iter()
            .map(|(p, nodes)| (p, nodes.into_iter().collect()))
            .collect();
        for key in kept.tabs.into_iter().filter_map(Tab::key) {
            if !self.explorer.tabs.contains(&key) {
                self.open_doc(key, window, cx);
            }
        }
        if let Some(active) = kept.active.and_then(Tab::key) {
            if self.explorer.tabs.contains(&active) {
                self.explorer.active = Some(active);
            }
        }
    }

    /// The tabs and folds written down (not before they were read back).
    pub(crate) fn keep_explorer(&mut self) {
        if !self.explorer.restored {
            return;
        }
        let ex = &mut self.explorer;
        if !ex.project.is_empty() {
            ex.folds.insert(ex.project.clone(), ex.open.clone());
        }
        let kept = Kept {
            tabs: ex.tabs.iter().map(Tab::of).collect(),
            active: ex.active.as_ref().map(Tab::of),
            open: ex
                .folds
                .iter()
                .map(|(p, nodes)| {
                    let mut nodes: Vec<String> = nodes.iter().cloned().collect();
                    nodes.sort();
                    (p.clone(), nodes)
                })
                .collect(),
        };
        let _ = std::fs::create_dir_all(connection::state_dir());
        let _ = std::fs::write(path(), serde_json::to_vec(&kept).unwrap_or_default());
    }

    /// Every open folder of the project's tree listed (they were open when it was left).
    pub(crate) fn list_open_folders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let project = self.explorer.project.clone();
        let folders: Vec<FileRef> = self
            .explorer
            .open
            .iter()
            .filter_map(|n| n.strip_prefix("d:"))
            .filter(|k| !self.explorer.listings.contains_key(*k))
            .filter_map(|k| {
                let (machine, path) = k.split_once(':')?;
                Some(FileRef {
                    project: project.clone(),
                    machine: (!machine.is_empty()).then(|| machine.to_string()),
                    path: path.to_string(),
                })
            })
            .collect();
        for f in folders {
            self.list_folder(f, window, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::explorer::{DocKey, FileRef, Tab};

    #[test]
    fn a_tab_comes_back_as_it_was_kept() {
        let keys = [
            DocKey::File(FileRef {
                project: "dev:p1".into(),
                machine: Some("box".into()),
                path: "/srv/app/main.rs".into(),
            }),
            DocKey::Settings {
                project: "p1".into(),
            },
            DocKey::Part {
                project: "p1".into(),
                kind: "mcp",
                name: "ghost".into(),
            },
            DocKey::Problems {
                project: "p1".into(),
            },
        ];
        for k in keys {
            let json = serde_json::to_string(&Tab::of(&k)).unwrap();
            let back: Tab = serde_json::from_str(&json).unwrap();
            assert_eq!(back.key(), Some(k));
        }
        let gone = Tab::Part {
            project: "p1".into(),
            section: "no-such-section".into(),
            name: "x".into(),
        };
        assert_eq!(gone.key(), None);
    }
}
