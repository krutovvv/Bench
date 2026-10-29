//! What hangs around the conversation: the pages a piece of work touched. Its questions and
//! plan are `asks.rs`, its background processes `processes.rs`, the files it changed
//! `changed_files.rs`.

use ::thread::{Row, Step};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::thread::ThreadView;
use crate::thread::rows::tool_labels::{host_of, tool_urls};
use bench_ui::icons::Ico;

impl ThreadView {
    /// The pages the work right above answer `k` touched: one chip per site.
    pub(crate) fn render_sources(&self, k: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        let Some(Row::Work { key, steps, .. }) = k.checked_sub(1).map(|j| &self.thread.tv.rows[j])
        else {
            return None;
        };
        let mut urls = vec![];
        for s in steps {
            if let Step::Tool { name, args, .. } = s {
                tool_urls(name, args, &mut urls);
            }
        }
        if urls.is_empty() {
            return None;
        }
        let theme = cx.theme().clone();
        let mut hosts: Vec<(String, Vec<String>)> = vec![];
        for u in urls {
            let h = host_of(&u);
            match hosts.iter_mut().find(|(x, _)| *x == h) {
                Some((_, pages)) => pages.push(u),
                None => hosts.push((h, vec![u])),
            }
        }
        hosts.sort_by_key(|(_, p)| std::cmp::Reverse(p.len()));
        let open_host = self
            .thread
            .open_source
            .as_ref()
            .filter(|(t, _)| t == key)
            .map(|(_, h)| h.clone());
        let pages = open_host
            .as_ref()
            .and_then(|h| hosts.iter().find(|(x, _)| x == h))
            .map(|(_, p)| p.clone())
            .unwrap_or_default();
        let chips: Vec<AnyElement> = hosts
            .into_iter()
            .take(12)
            .enumerate()
            .map(|(n, (host, pages))| {
                let active = open_host.as_deref() == Some(&host);
                let single = (pages.len() == 1).then(|| pages[0].clone());
                let (h, key) = (host.clone(), key.clone());
                h_flex()
                    .id(SharedString::from(format!("src-{key}-{n}")))
                    .gap_1()
                    .px_1p5()
                    .py_0p5()
                    .border_1()
                    .rounded_md()
                    .text_size(px(bench_ui::THREAD_META))
                    .border_color(if active { theme.primary } else { theme.border })
                    .text_color(if active {
                        theme.foreground
                    } else {
                        theme.muted_foreground
                    })
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                    .child(Ico::Globe.icon().xsmall())
                    .child(host)
                    .when(pages.len() > 1, |d| {
                        d.child(div().opacity(0.6).child(pages.len().to_string()))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| match &single {
                        Some(url) => cx.open_url(url),
                        None => {
                            this.claim_click(cx);
                            this.thread.open_source = match &this.thread.open_source {
                                Some((t, x)) if *t == key && *x == h => None,
                                _ => Some((key.clone(), h.clone())),
                            };
                            cx.notify();
                        }
                    }))
                    .into_any_element()
            })
            .collect();
        let list = (!pages.is_empty()).then(|| {
            v_flex()
                .mt_1()
                .pl_2()
                .border_l_1()
                .border_color(theme.border)
                .children(pages.into_iter().enumerate().map(|(n, url)| {
                    let open = url.clone();
                    div()
                        .id(SharedString::from(format!("page-{key}-{n}")))
                        .text_color(theme.link)
                        .truncate()
                        .cursor_pointer()
                        .hover(|s| s.underline())
                        .child(
                            url.trim_start_matches("https://")
                                .trim_start_matches("http://")
                                .to_string(),
                        )
                        .on_click(move |_, _, cx| cx.open_url(&open))
                }))
        });
        Some(
            v_flex()
                .child(h_flex().flex_wrap().gap_1().children(chips))
                .children(list)
                .into_any_element(),
        )
    }
}
