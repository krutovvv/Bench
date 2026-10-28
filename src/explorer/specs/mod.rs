//! A project's specs in the Explorer: `SPEC.md` beside each module and the documents adopted as
//! nodes, linked by their frontmatter's `parent` and `references` (`agent/src/specs.ts` finds
//! them on the project's machine). The tree's "Specs" group shows them by parent, not by folder
//! (`tree.rs`); one opens as its own tab, the Markdown read with its links (`doc.rs`).

pub mod doc;
pub mod tree;

use gpui_kit::*;
use serde::Deserialize;

use crate::app::BenchApp;
use crate::client::rest;

/// One spec, as the agent lists it.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Spec {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub references: Vec<String>,
    /// Absolute, on the project's machine.
    pub path: String,
}

/// A repository's specs: its root and every spec in it.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Specs {
    pub root: String,
    pub specs: Vec<Spec>,
}

impl Specs {
    pub fn get(&self, id: &str) -> Option<&Spec> {
        self.specs.iter().find(|s| s.id == id)
    }

    /// The specs under `parent` (None: the roots — no parent, or one that is not here), by title.
    pub fn children(&self, parent: Option<&str>) -> Vec<&Spec> {
        let mut out: Vec<&Spec> = self
            .specs
            .iter()
            .filter(|s| match parent {
                Some(p) => s.parent.as_deref() == Some(p),
                None => s.parent.as_deref().is_none_or(|p| self.get(p).is_none()),
            })
            .collect();
        out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        out
    }

    /// The specs that name `id` among their references.
    pub fn referring(&self, id: &str) -> Vec<&Spec> {
        self.specs.iter().filter(|s| s.references.iter().any(|r| r == id)).collect()
    }
}

impl BenchApp {
    /// The shown project's specs, read once per showing of the Explorer (and again on its ↻).
    pub(crate) fn ensure_specs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let project = self.explorer.project.clone();
        if self.explorer.specs.contains_key(&project) {
            return;
        }
        self.explorer.specs.insert(project.clone(), None);
        if self.demo {
            self.explorer.specs.insert(project, Some(crate::demo::specs()));
            return;
        }
        let (Some(root), Some((conn, _))) = (self.explorer_root(), self.env_target(&project)) else {
            return;
        };
        self.background(
            window,
            cx,
            move || rest::specs(&conn, root.machine.as_deref(), &root.path),
            move |this, r, _, cx| {
                let specs = r.ok().and_then(|v| serde_json::from_value::<Specs>(v).ok());
                this.explorer.specs.insert(project.clone(), Some(specs.unwrap_or_default()));
                cx.notify();
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{Spec, Specs};

    fn spec(id: &str, title: &str, parent: Option<&str>, refs: &[&str]) -> Spec {
        Spec {
            id: id.into(),
            title: title.into(),
            status: None,
            parent: parent.map(Into::into),
            references: refs.iter().map(|r| r.to_string()).collect(),
            path: format!("/r/{id}/SPEC.md"),
        }
    }

    #[test]
    fn the_tree_follows_parents_and_an_orphan_is_a_root() {
        let s = Specs {
            root: "/r".into(),
            specs: vec![
                spec("app", "Bench", None, &[]),
                spec("thread", "src/thread", Some("app"), &["ui"]),
                spec("ui", "crates/ui", Some("app"), &[]),
                spec("lost", "Lost", Some("gone"), &[]),
            ],
        };
        let roots: Vec<_> = s.children(None).iter().map(|s| s.id.clone()).collect();
        assert_eq!(roots, ["app", "lost"]);
        let under: Vec<_> = s.children(Some("app")).iter().map(|s| s.id.clone()).collect();
        assert_eq!(under, ["ui", "thread"], "by title: crates/ui before src/thread");
        assert_eq!(s.referring("ui")[0].id, "thread");
    }
}
