//! The tree's "Specs" group: the project's specs by parent — the product, its architecture, each
//! module under it — named by their titles, a draft quieter. A click opens the spec as a tab
//! (`doc.rs`) and unfolds what is under it.

use bench_ui::icons::Ico;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::explorer::specs::Specs;
use crate::explorer::{DocKey, FileRef};

impl BenchApp {
    /// The group's rows, under its heading while it is open.
    pub(crate) fn spec_rows(&self, specs: &Specs, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut out = vec![];
        for s in specs.children(None) {
            self.spec_row(specs, &s.id, 0, &mut out, cx);
        }
        out
    }

    fn spec_row(&self, specs: &Specs, id: &str, depth: usize, out: &mut Vec<AnyElement>, cx: &mut Context<Self>) {
        let Some(s) = specs.get(id) else { return };
        // A cycle in a hand-written graph must not hang the tree.
        if depth > 12 {
            return;
        }
        let theme = cx.theme().clone();
        let node = format!("spec:{id}");
        let kids = specs.children(Some(id));
        let open = self.explorer.is_open(&node);
        let key = self.spec_key(s);
        let chosen = self.explorer.active.as_ref() == Some(&key);
        let draft = s.status.as_deref() == Some("draft");
        // "content/posts — one release, one post": the module's name, what it is quieter.
        let (name, what) = match s.title.split_once(" — ") {
            Some((n, w)) => (n.to_string(), Some(SharedString::from(w.to_string()))),
            None => (s.title.clone(), None),
        };
        let fold = node.clone();
        let row = if kids.is_empty() {
            bench_ui::tree_row(
                format!("ex-spec-{id}"),
                depth,
                None,
                (Ico::BookCheck, theme.primary),
                name,
                what,
                draft,
                chosen,
                vec![],
                cx,
            )
        } else {
            // The chevron folds and unfolds; the row opens the spec (and unfolds it).
            bench_ui::tree_doc_row(
                format!("ex-spec-{id}"),
                depth,
                open,
                (Ico::BookCheck, theme.primary),
                name,
                what,
                draft,
                chosen,
                cx.listener(move |this, _, _, cx| this.toggle_node(&fold, cx)),
                cx,
            )
        };
        out.push(
            row.on_click(cx.listener(move |this, _, window, cx| {
                if !this.explorer.is_open(&node) {
                    this.toggle_node(&node, cx);
                }
                this.open_doc(key.clone(), window, cx)
            }))
            .into_any_element(),
        );
        if open {
            for k in kids {
                self.spec_row(specs, &k.id, depth + 1, out, cx);
            }
        }
    }

    /// A spec's tab: its file in the shown project, on its machine.
    pub(crate) fn spec_key(&self, s: &crate::explorer::specs::Spec) -> DocKey {
        let project = self.explorer.project.clone();
        let machine = self.location(&project).files_machine();
        DocKey::Spec(FileRef { project, machine, path: s.path.clone() })
    }
}
