//! A spec as a tab (`DocKey::Spec`): its Markdown read, the frontmatter left out, and over it the
//! graph around it — the spec it is under, the ones under it, the ones it refers to and that refer
//! to it — each a link to that spec's tab, as a wiki reads. "Edit" opens the file itself.

use bench_ui::icons::Ico;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use bench_ui::Gap;

use crate::app::BenchApp;
use crate::client::rest;
use crate::explorer::pane::item::Item;
use crate::explorer::pane::view::note;
use crate::explorer::{DocKey, FileRef};

pub struct SpecItem(pub FileRef);

impl Item for SpecItem {
    fn tab(&self, app: &BenchApp, cx: &App) -> (String, (Ico, Hsla)) {
        let name = app
            .spec_at(&self.0)
            .map(|s| short_title(&s.title))
            .unwrap_or_else(|| self.0.name().to_string());
        (name, (Ico::BookCheck, cx.theme().primary))
    }

    fn open(&self, app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) -> bool {
        let f = self.0.clone();
        if app.explorer.spec_texts.contains_key(&f.path) {
            return true;
        }
        app.explorer.spec_texts.insert(f.path.clone(), None);
        let Some((conn, _)) = app.env_target(&f.project) else {
            return true;
        };
        let (machine, path) = (f.machine.clone(), f.path.clone());
        app.background(
            window,
            cx,
            move || rest::read_text(&conn, machine.as_deref(), &path),
            move |this, r, _, cx| {
                let text = r.map(|t| t.text).unwrap_or_else(|e| format!("Could not read it: {e:#}"));
                this.explorer.spec_texts.insert(f.path.clone(), Some(text));
                cx.notify();
            },
        );
        true
    }

    fn closed(&self, app: &mut BenchApp, _cx: &mut Context<BenchApp>) {
        // Read again the next time: the agent may have changed it meanwhile.
        app.explorer.spec_texts.remove(&self.0.path);
    }

    fn render(&self, app: &mut BenchApp, _window: &mut Window, cx: &mut Context<BenchApp>) -> AnyElement {
        app.spec_pane(&self.0, cx)
    }
}

/// "src/thread — a thread on screen" → "src/thread": a tab's name is short.
pub(crate) fn short_title(t: &str) -> String {
    t.split(" — ").next().unwrap_or(t).to_string()
}

/// The Markdown after the frontmatter.
pub fn body(text: &str) -> &str {
    if let Some(rest) = text.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            return rest[end + 5..].trim_start();
        }
    }
    text
}

impl BenchApp {
    /// The spec `f` is, among the shown project's.
    pub(crate) fn spec_at(&self, f: &FileRef) -> Option<&crate::explorer::specs::Spec> {
        self.explorer
            .specs
            .get(&f.project)?
            .as_ref()?
            .specs
            .iter()
            .find(|s| s.path == f.path)
    }

    fn spec_pane(&mut self, f: &FileRef, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let specs = self.explorer.specs.get(&f.project).cloned().flatten().unwrap_or_default();
        let me = specs.specs.iter().find(|s| s.path == f.path).cloned();
        let edit = f.clone();
        let actions = vec![
            bench_ui::form_action("spec-edit", "Edit", false, cx)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_doc(DocKey::File(edit.clone()), window, cx)
                }))
                .into_any_element(),
        ];
        let title = me.as_ref().map_or_else(|| f.name().to_string(), |s| s.title.clone());
        let rel = f.path.strip_prefix(&format!("{}/", specs.root)).unwrap_or(&f.path).to_string();
        let head = bench_ui::ide_head(title, Some(rel.into()), actions, cx);
        // The graph around it, a line of links per kind.
        let mut links: Vec<AnyElement> = vec![];
        if let Some(me) = &me {
            let up: Vec<_> = me.parent.iter().filter_map(|p| specs.get(p)).collect();
            let down = specs.children(Some(&me.id));
            let to: Vec<_> = me.references.iter().filter_map(|r| specs.get(r)).collect();
            let from = specs.referring(&me.id);
            for (label, list) in [("Under", up), ("Over", down), ("Refers to", to), ("Referred to by", from)] {
                if list.is_empty() {
                    continue;
                }
                let chips = list.into_iter().map(|s| {
                    let key = self.spec_key(s);
                    div()
                        .id(SharedString::from(format!("spec-link-{}-{}", label, s.id)))
                        .flex_none()
                        .text_color(theme.primary)
                        .cursor_pointer()
                        .hover(|d| d.underline())
                        .child(short_title(&s.title))
                        .on_click(cx.listener(move |this, _, window, cx| this.open_doc(key.clone(), window, cx)))
                        .into_any_element()
                });
                links.push(
                    h_flex()
                        .gap(Gap::L)
                        .flex_wrap()
                        .text_size(px(bench_ui::BAR_TEXT))
                        .child(div().flex_none().w(px(bench_ui::SPEC_LINK_LABEL_W)).text_color(bench_ui::faint(&theme)).child(label))
                        .children(chips)
                        .into_any_element(),
                );
            }
        }
        let body = match self.explorer.spec_texts.get(&f.path).cloned().flatten() {
            None => note("Reading…", cx),
            Some(text) => crate::media::text::markdown(
                SharedString::from(format!("spec-{}", f.path)),
                body(&text).to_string(),
                cx,
            )
            .into_any_element(),
        };
        v_flex()
            .size_full()
            .child(head)
            .child(
                div().id("spec-scroll").flex_1().min_h_0().overflow_y_scroll().child(
                    v_flex()
                        .w_full()
                        .max_w(px(bench_ui::SPEC_COLUMN_W))
                        .mx_auto()
                        .px(Gap::Xxl)
                        .py(Gap::Xl)
                        .gap(Gap::Xl)
                        .when(!links.is_empty(), |d| {
                            d.child(
                                v_flex()
                                    .gap(Gap::Xs)
                                    .pb(Gap::L)
                                    .border_b_1()
                                    .border_color(theme.border.opacity(0.6))
                                    .children(links),
                            )
                        })
                        .child(body),
                ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_body_is_what_follows_the_frontmatter() {
        assert_eq!(super::body("---\nid: a\n---\n\n# A\n"), "# A\n");
        assert_eq!(super::body("# No frontmatter\n"), "# No frontmatter\n");
        assert_eq!(super::short_title("src/thread — a thread on screen"), "src/thread");
    }
}
