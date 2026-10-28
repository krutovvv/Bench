//! Tools mode (⌘3, the wrench in the status bar): the utilities of `devtools` in the window,
//! listed in the sidebar like servers, each with its input and output on the right. Every tool
//! keeps its own input while another one is open; output follows the input as you type.

use std::collections::HashMap;
use std::path::PathBuf;

use gpui_kit::component::input::{EditorState, InputEvent, InputState};
use gpui_kit::*;

use crate::app::BenchApp;
use devtools::{self, Kind, RegexFlags, Tool};

pub struct Tools {
    pub tool: Tool,
    /// One input and one output editor, shared: each tool's input is parked in `inputs`.
    pub input: Entity<EditorState>,
    pub output: Entity<EditorState>,
    /// The regex tester's pattern.
    pub pattern: Entity<InputState>,
    pub flags: RegexFlags,
    pub matches: usize,
    pub inputs: HashMap<Tool, String>,
    /// Each tool's choice in its segmented control, and a generator's count.
    pub modes: HashMap<Tool, usize>,
    pub counts: HashMap<Tool, usize>,
    /// The last error (bad input), shown above the output.
    pub error: Option<String>,
    /// The Base64 tool's decoded image on disk: shown, and what Save copies; with its format,
    /// size and weight for the pane's title.
    pub image: Option<PathBuf>,
    pub image_info: Option<String>,
    /// Markdown Preview: the text being rendered; Rich Text to Markdown: the pasted HTML,
    /// shown as it looked on the page unless `html_source` asks for the code.
    pub preview: String,
    /// The panes that scroll inside the thread: their own scroll, each remembered.
    pub preview_scroll: ScrollHandle,
    pub html_scroll: ScrollHandle,
    pub html_source: bool,
    _subscriptions: Vec<Subscription>,
}

impl Tools {
    pub fn new(window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let input = cx.new(|cx| {
            EditorState::new(window, cx)
                .scroll_beyond_last_line(Some(1))
                .line_number(true)
                .searchable(true)
        });
        let output = cx.new(|cx| {
            EditorState::new(window, cx)
                .scroll_beyond_last_line(Some(1))
                .line_number(true)
                .searchable(true)
        });
        let pattern =
            cx.new(|cx| InputState::new(window, cx).placeholder(r"Regex, e.g. (\w+)@(\w+)\.com"));
        let subscriptions = vec![
            cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, window, cx| {
                if let InputEvent::Change = ev {
                    this.run_tool(window, cx);
                }
            }),
            cx.subscribe_in(&pattern, window, |this, _, ev: &InputEvent, window, cx| {
                if let InputEvent::Change = ev {
                    this.run_tool(window, cx);
                }
            }),
        ];
        Self {
            preview_scroll: ScrollHandle::default(),
            html_scroll: ScrollHandle::default(),
            tool: Tool::HtmlToMarkdown,
            input,
            output,
            pattern,
            flags: RegexFlags::default(),
            matches: 0,
            inputs: HashMap::new(),
            modes: HashMap::new(),
            counts: HashMap::new(),
            error: None,
            image: None,
            image_info: None,
            preview: String::new(),
            html_source: false,
            _subscriptions: subscriptions,
        }
    }

    pub fn mode(&self, tool: Tool) -> usize {
        self.modes.get(&tool).copied().unwrap_or(0)
    }

    pub fn count(&self, tool: Tool) -> usize {
        self.counts.get(&tool).copied().unwrap_or(match tool {
            Tool::Password | Tool::Uuid => 10,
            Tool::Lorem => 3,
            _ => 1,
        })
    }
}

impl BenchApp {
    pub fn select_tool(&mut self, tool: Tool, window: &mut Window, cx: &mut Context<Self>) {
        let t = &mut self.inserts.tools;
        let old = t.input.read(cx).value().to_string();
        t.inputs.insert(t.tool, old);
        t.tool = tool;
        t.error = None;
        let text = t.inputs.get(&tool).cloned().unwrap_or_default();
        let language = match tool {
            Tool::HtmlToMarkdown => "html",
            Tool::MarkdownPreview => "markdown",
            Tool::JsonYaml => "json",
            _ => "text",
        };
        let out_language = match tool {
            Tool::HtmlToMarkdown => "markdown",
            Tool::CurlToCode => ["javascript", "python", "php"][t.mode(tool).min(2)],
            _ => "text",
        };
        t.input.update(cx, |s, cx| {
            s.set_highlighter(language, cx);
            s.set_value(text, window, cx);
        });
        t.output
            .update(cx, |s, cx| s.set_highlighter(out_language, cx));
        self.run_tool(window, cx);
        cx.notify();
    }

    pub fn set_tool_mode(&mut self, mode: usize, window: &mut Window, cx: &mut Context<Self>) {
        let tool = self.inserts.tools.tool;
        self.inserts.tools.modes.insert(tool, mode);
        if tool == Tool::CurlToCode {
            let lang = ["javascript", "python", "php"][mode.min(2)];
            self.inserts.tools
                .output
                .update(cx, |s, cx| s.set_highlighter(lang, cx));
        }
        self.run_tool(window, cx);
    }

    pub fn toggle_regex_flag(&mut self, k: usize, window: &mut Window, cx: &mut Context<Self>) {
        let f = &mut self.inserts.tools.flags;
        match k {
            0 => f.case_insensitive = !f.case_insensitive,
            1 => f.multi_line = !f.multi_line,
            _ => f.dot_all = !f.dot_all,
        }
        self.run_tool(window, cx);
    }

    /// The output for the current input (as you type).
    pub fn run_tool(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tool = self.inserts.tools.tool;
        let input = self.inserts.tools.input.read(cx).value().to_string();
        if tool == Tool::HtmlToMarkdown {
            self.inserts.tools.preview = input.clone();
        }
        let (out, error) = match tool.kind() {
            Kind::Transform => match devtools::transform(tool, &input, self.inserts.tools.mode(tool)) {
                Ok(out) => (Some(out), None),
                Err(e) => (Some(String::new()), Some(e)),
            },
            Kind::Regex => {
                let pattern = self.inserts.tools.pattern.read(cx).value().to_string();
                match devtools::regex_matches(&pattern, &input, self.inserts.tools.flags) {
                    Ok((n, list, _)) => {
                        self.inserts.tools.matches = n;
                        (Some(list), None)
                    }
                    Err(e) => {
                        self.inserts.tools.matches = 0;
                        (Some(String::new()), Some(e))
                    }
                }
            }
            Kind::Preview => {
                self.inserts.tools.preview = input;
                (None, None)
            }
            Kind::Image => {
                // A new file for each image (the image cache keys on the path); the old one goes.
                if let Some(old) = self.inserts.tools.image.take() {
                    let _ = std::fs::remove_file(old);
                }
                self.inserts.tools.image_info = None;
                if input.trim().is_empty() {
                    (None, None)
                } else {
                    match devtools::decode_image(&input) {
                        Ok((bytes, ext)) => {
                            let dir = crate::client::connection::state_dir().join("tools");
                            let file =
                                dir.join(format!("image-{}.{ext}", uuid::Uuid::new_v4().simple()));
                            match std::fs::create_dir_all(&dir)
                                .and_then(|_| std::fs::write(&file, &bytes))
                            {
                                Ok(()) => {
                                    let dims = imagesize::blob_size(&bytes)
                                        .map(|d| format!(" · {}×{} px", d.width, d.height))
                                        .unwrap_or_default();
                                    self.inserts.tools.image_info = Some(format!(
                                        "{}{dims} · {}",
                                        ext.to_uppercase(),
                                        human_bytes(bytes.len())
                                    ));
                                    self.inserts.tools.image = Some(file);
                                    (None, None)
                                }
                                Err(e) => (None, Some(e.to_string())),
                            }
                        }
                        Err(e) => (None, Some(e)),
                    }
                }
            }
            Kind::Generator => {
                let out = devtools::generate(tool, self.inserts.tools.mode(tool), self.inserts.tools.count(tool));
                (Some(out), None)
            }
        };
        self.inserts.tools.error = error;
        if let Some(out) = out {
            self.inserts.tools
                .output
                .update(cx, |s, cx| s.set_value(out, window, cx));
        }
        cx.notify();
    }

    pub fn generate_tool(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tool = self.inserts.tools.tool;
        let out = devtools::generate(tool, self.inserts.tools.mode(tool), self.inserts.tools.count(tool));
        self.inserts.tools
            .output
            .update(cx, |s, cx| s.set_value(out, window, cx));
        cx.notify();
    }

    pub fn tool_input(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        self.inserts.tools
            .input
            .update(cx, |s, cx| s.set_value(text, window, cx));
        self.run_tool(window, cx);
    }

    /// "Paste": the clipboard into the input. Rich Text to Markdown takes the clipboard's HTML
    /// (or RTF) when there is some, else its plain text.
    pub fn paste_tool_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let plain = cx
            .read_from_clipboard()
            .and_then(|c| c.text())
            .unwrap_or_default();
        if self.inserts.tools.tool != Tool::HtmlToMarkdown {
            self.tool_input(plain, window, cx);
            return;
        }
        cx.spawn_in(window, async move |this, cx| {
            let html = cx
                .background_spawn(async move { devtools::clipboard_html() })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                let none = html.is_none();
                this.tool_input(html.unwrap_or(plain), window, cx);
                if none {
                    this.inserts.tools.error = Some(
                        "The clipboard holds plain text only (no formatting to convert)".into(),
                    );
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn copy_tool_output(&mut self, cx: &mut Context<Self>) {
        let text = self.inserts.tools.output.read(cx).value().to_string();
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    /// Base64 Image: an image file into the input, as a data URL.
    pub fn load_tool_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Encode".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let Ok(bytes) = std::fs::read(&path) else {
                return;
            };
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_default();
            let _ = this.update_in(cx, |this, window, cx| {
                this.tool_input(devtools::encode_image(&bytes, &ext), window, cx)
            });
        })
        .detach();
    }

    /// Base64 Image: the decoded image to a file the user picks.
    pub fn save_tool_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(src) = self.inserts.tools.image.clone() else {
            return;
        };
        let ext = src
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_else(|| "png".into());
        let home = PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("Downloads");
        let picked = cx.prompt_for_new_path(&home, Some(&format!("image.{ext}")));
        cx.spawn_in(window, async move |_, _| {
            if let Ok(Ok(Some(dest))) = picked.await {
                let _ = std::fs::copy(&src, dest);
            }
        })
        .detach();
    }
}

fn human_bytes(n: usize) -> String {
    match n {
        0..=1023 => format!("{n} B"),
        1024..=1_048_575 => format!("{:.1} KB", n as f64 / 1024.),
        _ => format!("{:.1} MB", n as f64 / 1_048_576.),
    }
}
