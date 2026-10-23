//! An answer's text, delivered files and diagrams, in order. Images and diagrams show inline,
//! videos play in place (ffmpeg), documents show their first page, audio plays here, Markdown
//! files read in place, formulas read as Unicode; any other file opens in its own app on a click.

use std::path::PathBuf;

use gpui_kit::component::text::{
    MarkdownNode, MarkdownParseContext, MarkdownPlugin, TextView, markdown_ast,
};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::HermesApp;
use crate::audio::{self, Playback};
use crate::icons::Ico;
use crate::media::{self, Fetched, Kind, Loaded, Segment, file_name, latex_text};

/// The largest box an image or preview takes, in points.
const MAX_W: f32 = 560.;
const MAX_H: f32 = 420.;
/// A Markdown file shows this much until unfolded.
const PREVIEW_H: f32 = 280.;

fn fit_points((w, h): (f32, f32), max_w: f32, max_h: f32) -> (f32, f32) {
    let k = (max_w / w).min(max_h / h).min(1.);
    ((w * k).round(), (h * k).round())
}

fn fit(size: (f32, f32), max_w: f32, max_h: f32) -> (Pixels, Pixels) {
    let (w, h) = fit_points(size, max_w, max_h);
    (px(w), px(h))
}

/// `$…$`: a formula as Unicode text in italics. Money ("$5 and $10") is not one: TeX math
/// never starts or ends with a space.
struct InlineMath;

impl MarkdownPlugin for InlineMath {
    fn name(&self) -> &str {
        "math"
    }

    fn parse(
        &self,
        node: &markdown_ast::Node,
        _: &MarkdownParseContext<'_>,
    ) -> Option<MarkdownNode> {
        let markdown_ast::Node::InlineMath(m) = node else {
            return None;
        };
        let math = m.value.trim() == m.value && !m.value.is_empty();
        let text = if math {
            latex_text(&m.value)
        } else {
            format!("${}$", m.value)
        };
        Some(MarkdownNode::new("math", math).text(text))
    }

    fn render(&self, node: &MarkdownNode, _: &mut Window, _: &mut App) -> impl IntoElement {
        let math = node.data::<bool>().copied().unwrap_or(true);
        div()
            .when(math, |d| d.italic())
            .child(node.as_text().to_string())
    }
}

/// `$$…$$`: a formula on its own line, centered.
struct BlockMath;

impl MarkdownPlugin for BlockMath {
    fn is_block(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "math-block"
    }

    fn parse(
        &self,
        node: &markdown_ast::Node,
        _: &MarkdownParseContext<'_>,
    ) -> Option<MarkdownNode> {
        let markdown_ast::Node::Math(m) = node else {
            return None;
        };
        Some(MarkdownNode::new("math-block", ()).text(latex_text(&m.value)))
    }

    fn render(&self, node: &MarkdownNode, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .w_full()
            .py_1()
            .flex()
            .justify_center()
            .italic()
            .text_base()
            .child(node.as_text().to_string())
    }
}

/// Text exactly as written (the user's own message), selectable and copyable like an answer:
/// Markdown's characters are escaped, line breaks kept, leading spaces made non-breaking.
pub(super) fn plain_text(id: impl Into<SharedString>, text: &str) -> TextView {
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            let body = line.trim_start_matches([' ', '\t']);
            let indent: String = line[..line.len() - body.len()]
                .chars()
                .map(|c| {
                    if c == '\t' {
                        "\u{a0}".repeat(4)
                    } else {
                        "\u{a0}".into()
                    }
                })
                .collect();
            let mut out = indent;
            for c in body.chars() {
                if c.is_ascii_punctuation() {
                    out.push('\\');
                }
                out.push(c);
            }
            out
        })
        .collect();
    // Two spaces before a newline: a line break inside the paragraph (blank lines stay paragraphs).
    TextView::markdown(ElementId::Name(id.into()), lines.join("  \n"))
}

/// Markdown as answers show it: with formulas.
pub(super) fn markdown(id: impl Into<SharedString>, text: String) -> TextView {
    TextView::markdown(ElementId::Name(id.into()), text)
        .plugin(InlineMath)
        .plugin(BlockMath)
}

impl HermesApp {
    pub(super) fn render_answer(&mut self, i: usize, cx: &mut Context<Self>) -> Vec<AnyElement> {
        media::segments(&self.turns[i].answer)
            .into_iter()
            .enumerate()
            .map(|(k, s)| match s {
                Segment::Text(text) => markdown(format!("answer-{i}-{k}"), text).into_any_element(),
                Segment::Media(path) => self.render_media(i, k, path, cx),
                Segment::Mermaid(code) => self.render_diagram(i, k, code, cx),
            })
            .collect()
    }

    fn fetch_media(&mut self, key: String, path: String, cx: &mut Context<Self>) {
        let Some(conn) = self.conn.clone() else {
            return; // still connecting: the next frame asks again
        };
        self.media.insert(key.clone(), Loaded::Pending);
        let (target, profile) = (self.target.clone(), self.current_profile());
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_spawn(async move { media::fetch(&conn, &target, &profile, &path) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.media.insert(key, r.into());
                cx.notify();
            });
        })
        .detach();
    }

    fn toggle_media_audio(&mut self, key: String, local: PathBuf, cx: &mut Context<Self>) {
        if self.media_playing.as_ref().is_some_and(|(k, _)| *k == key) {
            self.media_playing = None;
            cx.notify();
            return;
        }
        self.media_playing = None;
        self.playing = None;
        cx.spawn(async move |this, cx| {
            let file = local.clone();
            let bytes = cx
                .background_spawn(async move {
                    std::fs::read(&file).ok().filter(|b| audio::decodable(b))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                match bytes {
                    Some(bytes) => {
                        this.stop_video(None, cx);
                        this.media_since = std::time::Instant::now();
                        this.media_playing = Some((key, Playback::start(bytes)));
                    }
                    // A format playback cannot decode: the system player can.
                    None => cx.open_with_system(&local),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Play a video in place: frames at its picture's size (twice, for Retina), with sound.
    fn start_video(
        &mut self,
        key: String,
        file: PathBuf,
        (w, h): (f32, f32),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stop_video(Some(window), cx);
        self.media_playing = None;
        self.playing = None;
        self.video_loading = Some(key.clone());
        let scale = window.scale_factor().clamp(1., 2.);
        let (pw, ph) = ((w * scale) as u32, (h * scale) as u32);
        self.video_task = Some(cx.spawn(async move |this, cx| {
            let (k, fallback) = (key.clone(), file.clone());
            let player = cx
                .background_spawn(async move { crate::video::Player::start(k, &file, pw, ph) })
                .await;
            let ok = this.update(cx, |this, cx| {
                this.video_loading = None;
                match player {
                    Ok(p) => this.video = Some(p),
                    // Could not decode it here: the system player can.
                    Err(_) => cx.open_with_system(&fallback),
                }
                cx.notify();
            });
            if ok.is_err() {
                return;
            }
            // Frames: each tick shows the one due and frees the one it replaced.
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(15))
                    .await;
                let go = this.update(cx, |this, cx| {
                    let Some(p) = this.video.as_mut() else {
                        return false;
                    };
                    let (changed, old) = p.tick();
                    if let Some(old) = old {
                        cx.drop_image(old, None);
                    }
                    if p.finished() {
                        this.stop_video(None, cx);
                        return false;
                    }
                    if changed {
                        cx.notify();
                    }
                    true
                });
                if !matches!(go, Ok(true)) {
                    break;
                }
            }
        }));
        cx.notify();
    }

    /// `window`: the one being updated, if any (its frame is freed there too).
    pub(crate) fn stop_video(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) {
        if let Some(mut p) = self.video.take() {
            if let Some(frame) = p.frame.take() {
                cx.drop_image(frame, window);
            }
        }
        self.video_loading = None;
        cx.notify();
    }

    fn render_media(
        &mut self,
        i: usize,
        k: usize,
        path: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = format!("{}\0{path}", self.target.label());
        if !self.media.contains_key(&key) {
            match media::fetch_now(&self.target, &path) {
                Some(f) => {
                    self.media.insert(key.clone(), Loaded::Ready(f));
                }
                None => self.fetch_media(key.clone(), path.clone(), cx),
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
                .border_color(theme.border)
                .text_sm()
                .child(icon.icon().xsmall().text_color(theme.muted_foreground))
                .child(div().flex_none().child(name.clone()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_xs()
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

        let fetched: Fetched = match self.media.get(&key).cloned().unwrap_or(Loaded::Pending) {
            // A video keeps a 16:9 frame while its first picture is made, so the chat barely moves.
            Loaded::Pending if kind == Kind::Video => {
                return div()
                    .w(px(MAX_W))
                    .h(px(MAX_W * 9. / 16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_1()
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
                        this.media.remove(&retry);
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
            .filter(|_| matches!(self.target, crate::model::Target::Local))
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
                .w(w)
                .h(h)
                .border_1()
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
                let player = self.video.as_ref().filter(|v| v.key == key);
                let elapsed = player.map(|v| super::clock(v.elapsed()));
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
                let starting = self.video_loading.as_deref() == Some(&key);
                let badge = |icon: AnyElement| {
                    div()
                        .p_3()
                        .rounded_full()
                        .bg(gpui_kit::black().opacity(0.55))
                        .text_color(gpui_kit::white())
                        .child(icon)
                        .into_any_element()
                };
                let here = crate::video::ffmpeg().is_some();
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
                        } else if this.video.as_ref().is_some_and(|v| v.key == k2) {
                            this.stop_video(Some(window), cx);
                        } else {
                            this.start_video(k2.clone(), l2.clone(), box_size, window, cx);
                        }
                    })))
                    .child(line)
                    .into_any_element()
            }
            Kind::Audio => {
                let playing = self.media_playing.as_ref().is_some_and(|(k, _)| *k == key);
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
                                    let t = self.media_since.elapsed().as_secs();
                                    (format!("playing · {}", super::clock(t)), theme.primary)
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
                let open = self.media_open.contains(&key);
                let k2 = key.clone();
                v_flex()
                    .max_w(px(MAX_W + 160.))
                    .border_1()
                    .border_color(theme.border)
                    .child(
                        h_flex()
                            .gap_2()
                            .px_2()
                            .py_1()
                            .items_center()
                            .text_sm()
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
                                    .text_xs()
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
                            .child(markdown(id("md"), text)),
                    )
                    .when(long, |d| {
                        d.child(
                            div()
                                .id(id("more"))
                                .px_3()
                                .py_1()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .border_t_1()
                                .border_color(theme.border)
                                .cursor_pointer()
                                .hover(|s| s.text_color(theme.foreground))
                                .child(if open { "Show less" } else { "Show all" })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if !this.media_open.remove(&k2) {
                                        this.media_open.insert(k2.clone());
                                    }
                                    cx.notify();
                                })),
                        )
                    })
                    .into_any_element()
            }
            // Anything else (3D models, archives, …) opens in its own app on a click.
            Kind::File => row(
                Some(open_button(local.clone(), "Open").into_any_element()),
                Some((where_, theme.muted_foreground)),
            )
            .id(id("file"))
            .cursor_pointer()
            .hover(|s| s.bg(theme.list_hover))
            .tooltip(|window, cx| Tooltip::new("Open in its app").build(window, cx))
            .on_click(opener(local))
            .into_any_element(),
        }
    }

    fn render_diagram(
        &mut self,
        i: usize,
        k: usize,
        code: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let key = (code.clone(), theme.is_dark());
        if !self.diagrams.contains_key(&key) {
            // Drawn right here (milliseconds, then cached): the chat lays out at its final
            // height in the first frame.
            let r = media::mermaid_svg(&code, theme.is_dark());
            self.diagrams.insert(key, r.into());
        }
        let id = SharedString::from(format!("diagram-{i}-{k}"));
        match self.diagrams.get(&(code.clone(), theme.is_dark())) {
            Some(Loaded::Ready((svg, size))) => {
                let (w, h) = fit(*size, MAX_W + 160., MAX_H + 200.);
                let open = svg.clone();
                div()
                    .id(id)
                    .w(w)
                    .h(h)
                    .cursor_pointer()
                    .child(img(svg.clone()).size_full())
                    .tooltip(|window, cx| Tooltip::new("Diagram — click to open").build(window, cx))
                    .on_click(move |_, _, cx| cx.open_with_system(&open))
                    .into_any_element()
            }
            // Not drawable here (a diagram type it lacks, or a syntax slip): its source, as code.
            Some(Loaded::Failed(_)) => {
                markdown(id, format!("```mermaid\n{code}```")).into_any_element()
            }
            _ => div().into_any_element(),
        }
    }

    /// A file going with a message: a small picture for an image, else its icon, and the name.
    /// `remove`: its index in the composer (shows ×); otherwise a click opens it.
    pub(super) fn attachment_chip(
        &self,
        id: SharedString,
        path: &str,
        remove: Option<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let local = std::path::PathBuf::from(path);
        let here = local.is_file();
        let icon = match media::kind(path) {
            Kind::Image => Ico::Image,
            Kind::Audio => Ico::Music,
            Kind::Video => Ico::Film,
            Kind::Markdown => Ico::FileText,
            Kind::Doc | Kind::File => Ico::File,
        };
        let open = local.clone();
        h_flex()
            .id(id.clone())
            .max_w(px(260.))
            .h(px(32.))
            .pl_1()
            .pr_2()
            .gap_1p5()
            .items_center()
            .border_1()
            .border_color(theme.border)
            .bg(theme.background)
            .text_xs()
            .map(|d| {
                if here && media::kind(path) == Kind::Image {
                    d.child(
                        img(local.clone())
                            .size(px(24.))
                            .flex_none()
                            .object_fit(ObjectFit::Cover),
                    )
                } else {
                    d.child(
                        div()
                            .size(px(24.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(theme.muted_foreground)
                            .child(icon.icon().xsmall()),
                    )
                }
            })
            .child(div().min_w_0().truncate().child(
                // A server file's local copy names the server file it stands for.
                match crate::remote::origin(path) {
                    Some((alias, remote)) => format!("{alias}: {remote}"),
                    None => file_name(path),
                },
            ))
            .map(|d| match remove {
                Some(k) => d.child(
                    div()
                        .id(SharedString::from(format!("{id}-x")))
                        .flex_none()
                        .p_0p5()
                        .cursor_pointer()
                        .text_color(theme.muted_foreground)
                        .hover(|s| s.text_color(theme.foreground))
                        .child(Ico::X.icon().xsmall())
                        .on_click(cx.listener(move |this, _, _, cx| this.remove_attachment(k, cx))),
                ),
                None if here => d
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.list_hover))
                    .on_click(move |_, _, cx| cx.open_with_system(&open)),
                None => d,
            })
            .into_any_element()
    }
}
