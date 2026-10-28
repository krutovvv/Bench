//! An answer's text, delivered files and diagrams, in order. Images and diagrams show inline,
//! videos play in place (ffmpeg), documents show their first page, audio plays here, Markdown
//! files read in place, formulas read as Unicode; any other file opens in its own app on a click.

use std::path::PathBuf;

use gpui_kit::component::text::TextView;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::text::{FOLD_AFTER_LINES, Texts, answer_segments, markdown, render_code};
use crate::app::BenchApp;
use crate::media::{self, Fetched, Kind, Loaded, Segment, file_name};
use crate::thread::ThreadRef;
use bench_ui::icons::Ico;

/// The largest box an image or preview takes, in points.
pub(super) const MAX_W: f32 = 560.;
const MAX_H: f32 = 420.;
/// A Markdown file shows this much until unfolded.
const PREVIEW_H: f32 = 280.;

fn fit_points((w, h): (f32, f32), max_w: f32, max_h: f32) -> (f32, f32) {
    let k = (max_w / w).min(max_h / h).min(1.);
    ((w * k).round(), (h * k).round())
}

pub(super) fn fit(size: (f32, f32), max_w: f32, max_h: f32) -> (Pixels, Pixels) {
    let (w, h) = fit_points(size, max_w, max_h);
    (px(w), px(h))
}

impl BenchApp {
    /// An answer's text as it shows: prose, delivered files, diagrams, code. `key` names its
    /// row (ids of its parts are made from it); `at` is the thread it is in (its files are where
    /// its agent is), `texts` that thread's parsed texts.
    pub(crate) fn render_answer(
        &mut self,
        at: &ThreadRef,
        texts: &mut Texts,
        key: &str,
        text: &str,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        // Blank lines around a segment (models often start with "\n\n") would show as empty
        // paragraphs: an answer is as tall as its text.
        answer_segments(text)
            .into_iter()
            .enumerate()
            .map(|(k, s)| match s {
                Segment::Text(text) => texts
                    .markdown(&format!("answer-{key}-{k}"), &text, cx)
                    .into_any_element(),
                Segment::Media(path) => self.render_media(at, key, k, path, cx),
                Segment::Mermaid(code) => self.render_diagram(key, k, code, cx),
                Segment::Code { lang, code } => {
                    let fence = if code.contains("```") { "~~~~" } else { "```" };
                    // A patch without a language on the fence: its query colours deletions as
                    // keywords and additions as strings. Its head still says `diff` (`lang` below).
                    let fence_lang = if lang == "diff" || lang == "patch" {
                        ""
                    } else {
                        lang.as_str()
                    };
                    let md = format!("{fence}{fence_lang}\n{code}{fence}");
                    let state_key = format!("code-{key}-{k}");
                    let body = TextView::new(&texts.state(&state_key, &md, cx));
                    // A long block (a log dump) comes folded to its first lines; opening it is
                    // the person's, remembered while Bench runs, like an unfolded Markdown file.
                    let long = code.lines().count() > FOLD_AFTER_LINES;
                    let open = self.media.open.contains(&state_key);
                    let more = long.then(|| {
                        let k2 = state_key.clone();
                        let n = code.lines().count();
                        div()
                            .id(SharedString::from(format!("code-more-{key}-{k}")))
                            .px_3()
                            .py_1()
                            .text_base()
                            .text_color(cx.theme().muted_foreground)
                            .border_t_1()
                            .border_color(cx.theme().border)
                            .cursor_pointer()
                            .hover(|s| s.text_color(cx.theme().foreground))
                            .child(if open {
                                "Show less".to_string()
                            } else {
                                format!("Show all · {n} lines")
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !this.media.open.remove(&k2) {
                                    this.media.open.insert(k2.clone());
                                }
                                cx.notify();
                            }))
                            .into_any_element()
                    });
                    render_code(body, key, k, lang, code, long && !open, more, cx)
                }
            })
            .collect()
    }

    /// Bring a thread's file here. `force` downloads again even when a copy is cached: a
    /// remote file may have been rewritten where it lives, and its name alone doesn't say
    /// it's the same file.
    fn fetch_media(
        &mut self,
        at: &ThreadRef,
        key: String,
        path: String,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(conn) = self.thread_conn(at) else {
            return; // still connecting: the next frame asks again
        };
        if force && matches!(self.media.files.get(&key), Some(Loaded::Pending)) {
            return; // a fresh copy is already on its way
        }
        self.media.files.insert(key.clone(), Loaded::Pending);
        let target = self.thread_target(at);
        let machine = self.files_machine(at);
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_spawn(async move {
                    media::fetch(&conn, &target, machine.as_deref(), &path)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                // A click waited on this copy: open it in its app now that it is here.
                let open = match &r {
                    Ok(f) if this.media.open_after_fetch.as_deref() == Some(key.as_str()) => {
                        this.media.open_after_fetch = None;
                        Some(f.local.clone())
                    }
                    _ => None,
                };
                this.media.files.insert(key, r.into());
                if let Some(p) = open {
                    cx.open_with_system(&p);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn render_media(
        &mut self,
        at: &ThreadRef,
        i: &str,
        k: usize,
        path: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // A thread's files are where its agent is: this Mac's, or its server's.
        let target = self.thread_target(at);
        let key = format!("{}\0{path}", target.label());
        if !self.media.files.contains_key(&key) {
            match media::fetch_now(&target, &path) {
                Some(f) => {
                    self.media.files.insert(key.clone(), Loaded::Ready(f));
                }
                None => self.fetch_media(at, key.clone(), path.clone(), false, cx),
            }
        }
        let theme = cx.theme().clone();
        let kind = media::kind(&path);
        let name = file_name(&path);
        let id = |part: &str| SharedString::from(format!("media-{i}-{k}-{part}"));
        let icon = match kind {
            Kind::Image => Ico::Image,
            Kind::Audio => Ico::Music,
            Kind::Video => Ico::Film,
            Kind::Markdown => Ico::FileText,
            Kind::Doc | Kind::File => Ico::File,
        };
        // A file as one line: icon, name, where it is, then the actions.
        let row = |extra: Option<AnyElement>, note: Option<(String, Hsla)>| {
            h_flex()
                .max_w(px(MAX_W))
                .gap_2()
                .px_2()
                .py_1()
                .items_center()
                .border_1()
                .rounded_md()
                .border_color(theme.border)
                .text_base()
                .child(icon.icon().xsmall().text_color(theme.muted_foreground))
                .child(div().flex_none().child(name.clone()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_base()
                        .text_color(theme.muted_foreground)
                        .when_some(note, |d, (text, color)| d.text_color(color).child(text)),
                )
                .children(extra)
        };
        let open_button = |local: PathBuf, tip: &'static str| {
            div()
                .id(id("open"))
                .p_1()
                .cursor_pointer()
                .text_color(theme.muted_foreground)
                .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                .child(Ico::ExternalLink.icon().xsmall())
                .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
                .on_click(move |_, _, cx| cx.open_with_system(&local))
        };

        let fetched: Fetched = match self.media.files.get(&key).cloned().unwrap_or(Loaded::Pending) {
            // A video keeps a 16:9 frame while its first picture is made, so the chat barely moves.
            Loaded::Pending if kind == Kind::Video => {
                return div()
                    .w(px(MAX_W))
                    .h(px(MAX_W * 9. / 16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_1()
                    .rounded_md()
                    .border_color(theme.border)
                    .text_color(theme.muted_foreground)
                    .child(Ico::Loader.live(true))
                    .into_any_element();
            }
            Loaded::Pending => {
                return row(
                    Some(Ico::Loader.live(false)),
                    Some(("Loading…".into(), theme.muted_foreground)),
                )
                .into_any_element();
            }
            Loaded::Failed(e) => {
                let retry = key.clone();
                return row(None, Some((e, theme.danger)))
                    .id(id("failed"))
                    .cursor_pointer()
                    .tooltip(|window, cx| Tooltip::new("Click to try again").build(window, cx))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.media.files.remove(&retry);
                        cx.notify();
                    }))
                    .into_any_element();
            }
            Loaded::Ready(f) => f,
        };
        let local = fetched.local.clone();
        let where_ = local
            .parent()
            .map(|p| {
                p.to_string_lossy()
                    .replace(&std::env::var("HOME").unwrap_or_default(), "~")
            })
                        // A file fetched from another machine sits in Bench's cache: no folder to say.
            .filter(|_| !local.starts_with(crate::client::connection::state_dir()))
            .unwrap_or_default();

        // A picture that opens the file: an image itself, or a video's frame or a document's page.
        let picture = |src: ImageSource,
                       size: (f32, f32),
                       max_w: f32,
                       badge: Option<AnyElement>,
                       tip: SharedString| {
            let (w, h) = fit(size, max_w, MAX_H);
            div()
                .id(id("picture"))
                .relative()
                // Its own size at most, and narrower with a narrow thread (a dock): the shape kept.
                .w_full()
                .max_w(w)
                .aspect_ratio(f32::from(w) / f32::from(h).max(1.))
                .border_1()
                .rounded_md()
                .border_color(theme.border)
                .cursor_pointer()
                .child(img(src).size_full())
                .when_some(badge, |d, badge| {
                    d.child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(badge),
                    )
                })
                .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
        };
        let open_tip: SharedString = format!("{name} — click to open").into();
        let opener = |local: PathBuf| {
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| cx.open_with_system(&local)
        };

        match kind {
            Kind::Image => match fetched.size {
                Some(size) => picture(local.clone().into(), size, MAX_W, None, open_tip)
                    .on_click(opener(local.clone()))
                    .into_any_element(),
                None => row(Some(open_button(local, "Open").into_any_element()), None)
                    .into_any_element(),
            },
            Kind::Doc => {
                let line = row(
                    Some(open_button(local.clone(), "Open").into_any_element()),
                    Some((where_, theme.muted_foreground)),
                );
                match fetched.thumb {
                    Some((thumb, size)) => v_flex()
                        .gap_1()
                        .child(
                            picture(thumb.into(), size, 240., None, open_tip)
                                .on_click(opener(local.clone())),
                        )
                        .child(line)
                        .into_any_element(),
                    None => line.into_any_element(),
                }
            }
            Kind::Video => {
                let player = self.media.video.as_ref().filter(|v| v.key == key);
                let elapsed = player.map(|v| bench_ui::clock(v.elapsed()));
                let line = row(
                    Some(open_button(local.clone(), "Open in the player").into_any_element()),
                    Some(match &elapsed {
                        Some(t) => (format!("playing · {t}"), theme.primary),
                        None => (where_, theme.muted_foreground),
                    }),
                );
                let Some((thumb, size)) = fetched.thumb else {
                    return line.into_any_element();
                };
                // Playing: its current frame; else the first one with a play badge.
                let frame = player.and_then(|v| v.frame.clone());
                let starting = self.media.video_loading.as_deref() == Some(&key);
                let badge = |icon: AnyElement| {
                    div()
                        .p_3()
                        .rounded_full()
                        .bg(gpui_kit::black().opacity(0.55))
                        .text_color(gpui_kit::white())
                        .child(icon)
                        .into_any_element()
                };
                let here = crate::media::video::ffmpeg().is_some();
                let (k2, l2) = (key.clone(), local.clone());
                let shown = match (frame, player.is_some()) {
                    (Some(f), _) => picture(
                        ImageSource::Render(f),
                        size,
                        MAX_W,
                        None,
                        "Click to stop".into(),
                    ),
                    (None, playing) => picture(
                        thumb.into(),
                        size,
                        MAX_W,
                        Some(badge(if starting || playing {
                            Ico::Loader.live(true)
                        } else {
                            Ico::Play.icon().small().into_any_element()
                        })),
                        if here {
                            format!("{name} — click to play").into()
                        } else {
                            open_tip
                        },
                    ),
                };
                let box_size = fit_points(size, MAX_W, MAX_H);
                v_flex()
                    .gap_1()
                    .child(shown.on_click(cx.listener(move |this, _, window, cx| {
                        if !here {
                            cx.open_with_system(&l2);
                        } else if this.media.video.as_ref().is_some_and(|v| v.key == k2) {
                            this.stop_video(Some(window), cx);
                        } else {
                            this.start_video(k2.clone(), l2.clone(), box_size, window, cx);
                        }
                    })))
                    .child(line)
                    .into_any_element()
            }
            Kind::Audio => {
                let playing = self.media.audio.as_ref().is_some_and(|(k, _)| *k == key);
                let (k2, l2) = (key.clone(), local.clone());
                let play = div()
                    .id(id("play"))
                    .p_1()
                    .cursor_pointer()
                    .text_color(if playing {
                        theme.primary
                    } else {
                        theme.foreground
                    })
                    .hover(|s| s.bg(theme.list_hover))
                    .child(
                        if playing { Ico::Square } else { Ico::Play }
                            .icon()
                            .xsmall(),
                    )
                    .tooltip(move |window, cx| {
                        Tooltip::new(if playing { "Stop" } else { "Play" }).build(window, cx)
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_media_audio(k2.clone(), l2.clone(), cx)
                    }));
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(play)
                    .child(
                        row(
                            Some(open_button(local, "Open in the player").into_any_element()),
                            playing
                                .then(|| {
                                    let t = self.media.audio_since.elapsed().as_secs();
                                    (format!("playing · {}", bench_ui::clock(t)), theme.primary)
                                })
                                .or(Some((where_, theme.muted_foreground))),
                        )
                        .flex_1()
                        .min_w_0(),
                    )
                    .max_w(px(MAX_W))
                    .into_any_element()
            }
            Kind::Markdown => {
                let text = fetched.text.unwrap_or_default();
                let long = text.lines().count() > 14 || text.len() > 1400;
                let open = self.media.open.contains(&key);
                let k2 = key.clone();
                v_flex()
                    .max_w(px(MAX_W + 160.))
                    .border_1()
                    .rounded_md()
                    .border_color(theme.border)
                    .child(
                        h_flex()
                            .gap_2()
                            .px_2()
                            .py_1()
                            .items_center()
                            .text_base()
                            .bg(theme.secondary)
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                Ico::FileText
                                    .icon()
                                    .xsmall()
                                    .text_color(theme.muted_foreground),
                            )
                            .child(div().flex_none().child(name.clone()))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_base()
                                    .text_color(theme.muted_foreground)
                                    .child(where_),
                            )
                            .child(open_button(local, "Open")),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .when(long && !open, |d| d.max_h(px(PREVIEW_H)).overflow_hidden())
                            .child(markdown(id("md"), text, cx)),
                    )
                    .when(long, |d| {
                        d.child(
                            div()
                                .id(id("more"))
                                .px_3()
                                .py_1()
                                .text_base()
                                .text_color(theme.muted_foreground)
                                .border_t_1()
                                .border_color(theme.border)
                                .cursor_pointer()
                                .hover(|s| s.text_color(theme.foreground))
                                .child(if open { "Show less" } else { "Show all" })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if !this.media.open.remove(&k2) {
                                        this.media.open.insert(k2.clone());
                                    }
                                    cx.notify();
                                })),
                        )
                    })
                    .into_any_element()
            }
            // Anything else (3D models, archives, …) opens in its own app on a click — the
            // fresh copy: the agent may have rewritten it where it lives since it was last here.
            Kind::File => {
                let (k2, p2, at2) = (key.clone(), path.clone(), at.clone());
                row(
                    Some(open_button(local.clone(), "Open").into_any_element()),
                    Some((where_, theme.muted_foreground)),
                )
                .id(id("file"))
                .cursor_pointer()
                .hover(|s| s.bg(theme.list_hover))
                .tooltip(|window, cx| Tooltip::new("Open in its app").build(window, cx))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if matches!(this.thread_target(&at2), crate::app::model::Target::Local) {
                        cx.open_with_system(&local); // here already: this is the file itself
                        return;
                    }
                    this.media.open_after_fetch = Some(k2.clone());
                    this.fetch_media(&at2, k2.clone(), p2.clone(), true, cx);
                }))
                .into_any_element()
            }
        }
    }

    fn render_diagram(
        &mut self,
        i: &str,
        k: usize,
        code: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let key = (code.clone(), theme.is_dark());
        if !self.media.diagrams.contains_key(&key) {
            // Drawn right here (milliseconds, then cached): the chat lays out at its final
            // height in the first frame.
            let r = media::mermaid_svg(&code, theme.is_dark());
            self.media.diagrams.insert(key, r.into());
        }
        let id = SharedString::from(format!("diagram-{i}-{k}"));
        match self.media.diagrams.get(&(code.clone(), theme.is_dark())) {
            Some(Loaded::Ready((svg, size))) => {
                let (w, h) = fit(*size, MAX_W + 160., MAX_H + 200.);
                let open = svg.clone();
                div()
                    .id(id)
                    // As a picture: its size at most, narrower with the thread, its shape kept.
                    .w_full()
                    .max_w(w)
                    .aspect_ratio(f32::from(w) / f32::from(h).max(1.))
                    .cursor_pointer()
                    .child(img(svg.clone()).size_full())
                    .tooltip(|window, cx| Tooltip::new("Diagram — click to open").build(window, cx))
                    .on_click(move |_, _, cx| cx.open_with_system(&open))
                    .into_any_element()
            }
            // Not drawable here (a diagram type it lacks, or a syntax slip): its source, as code.
            Some(Loaded::Failed(_)) => {
                markdown(id, format!("```mermaid\n{code}```"), cx).into_any_element()
            }
            _ => div().into_any_element(),
        }
    }
}
