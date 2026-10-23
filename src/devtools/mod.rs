//! Tools mode: small text and data utilities that are always at hand (⌘3). Each tool is a pure
//! function of its input and options, so the views stay thin and everything here is testable.

mod convert;
mod encode;
mod generate;
mod markdown;
mod parse;

pub use markdown::clipboard_html;

/// One tool in the sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    HtmlToMarkdown,
    MarkdownPreview,
    Base64,
    Url,
    HtmlEntities,
    Backslash,
    Base64Image,
    UnixTime,
    JsonYaml,
    Case,
    LineSort,
    Password,
    Uuid,
    Lorem,
    Hash,
    Regex,
    UrlParser,
    Cron,
    CurlToCode,
}

/// How a tool's main area is laid out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Input on the left, output on the right, recomputed as you type.
    Transform,
    /// Input on the left, rendered Markdown on the right.
    Preview,
    /// Options and a Generate button; output only.
    Generator,
    /// A pattern and flags above, text and matches below.
    Regex,
    /// Base64 on the left, the image (and file buttons) on the right.
    Image,
}

pub struct Group {
    pub name: &'static str,
    pub tools: &'static [Tool],
}

/// The sidebar, in order: what serves work with agents and AI first.
pub const GROUPS: &[Group] = &[
    Group {
        name: "Markdown",
        tools: &[Tool::HtmlToMarkdown, Tool::MarkdownPreview],
    },
    Group {
        name: "Generate",
        tools: &[Tool::Password, Tool::Uuid, Tool::Lorem, Tool::Hash],
    },
    Group {
        name: "Encode",
        tools: &[
            Tool::Base64,
            Tool::Base64Image,
            Tool::Url,
            Tool::HtmlEntities,
            Tool::Backslash,
        ],
    },
    Group {
        name: "Convert",
        tools: &[
            Tool::UnixTime,
            Tool::JsonYaml,
            Tool::Case,
            Tool::LineSort,
            Tool::CurlToCode,
        ],
    },
    Group {
        name: "Inspect",
        tools: &[Tool::Regex, Tool::UrlParser, Tool::Cron],
    },
];

impl Tool {
    pub fn name(self) -> &'static str {
        match self {
            Tool::HtmlToMarkdown => "Rich Text to Markdown",
            Tool::MarkdownPreview => "Markdown Preview",
            Tool::Base64 => "Base64",
            Tool::Url => "URL Encode / Decode",
            Tool::HtmlEntities => "HTML Entities",
            Tool::Backslash => "Backslash Escape",
            Tool::Base64Image => "Base64 Image",
            Tool::UnixTime => "Unix Time",
            Tool::JsonYaml => "JSON ⇄ YAML",
            Tool::Case => "String Case",
            Tool::LineSort => "Sort & Dedupe Lines",
            Tool::Password => "Password",
            Tool::Uuid => "UUID",
            Tool::Lorem => "Lorem Ipsum",
            Tool::Hash => "Hash",
            Tool::Regex => "Regex Tester",
            Tool::UrlParser => "URL Parser",
            Tool::Cron => "Cron Parser",
            Tool::CurlToCode => "cURL to Code",
        }
    }

    /// A short line under the name in the tool's header.
    pub fn hint(self) -> &'static str {
        match self {
            Tool::HtmlToMarkdown => {
                "Copy from a web page, Word or Notes, press Paste: tables come out as Markdown tables"
            }
            Tool::MarkdownPreview => "Type or paste Markdown",
            Tool::Base64 => "UTF-8 text",
            Tool::Url => "Percent-encoding (RFC 3986)",
            Tool::HtmlEntities => "&amp; &lt; &gt; &quot; &#39; and named or numeric entities",
            Tool::Backslash => "\\n \\t \\\" \\\\ \\u{…}",
            Tool::Base64Image => "Paste Base64 or a data URL, or load an image file",
            Tool::UnixTime => "Seconds or milliseconds since 1970, or a date like 2026-10-23 14:05",
            Tool::JsonYaml => "Either way: the input's format is detected",
            Tool::Case => "Every case at once",
            Tool::LineSort => "One item per line",
            Tool::Password => "Letters and digits, one length, no symbols",
            Tool::Uuid => "Random (version 4)",
            Tool::Lorem => "Placeholder text",
            Tool::Hash => "UTF-8 text",
            Tool::Regex => "Rust regex syntax",
            Tool::UrlParser => "A full URL",
            Tool::Cron => "minute hour day-of-month month day-of-week",
            Tool::CurlToCode => "Paste a curl command (e.g. Copy as cURL from the browser)",
        }
    }

    pub fn kind(self) -> Kind {
        match self {
            Tool::MarkdownPreview => Kind::Preview,
            Tool::Password | Tool::Uuid | Tool::Lorem => Kind::Generator,
            Tool::Regex => Kind::Regex,
            Tool::Base64Image => Kind::Image,
            _ => Kind::Transform,
        }
    }

    /// The choices shown as a segmented control in the header (the first is the default).
    pub fn modes(self) -> &'static [&'static str] {
        match self {
            Tool::Base64 | Tool::Url | Tool::HtmlEntities => &["Encode", "Decode"],
            Tool::Backslash => &["Escape", "Unescape"],
            Tool::LineSort => &["A → Z", "Z → A", "Keep order"],
            Tool::Password => &["16", "20", "24", "32"],
            Tool::Lorem => &["Paragraphs", "Sentences", "Words"],
            Tool::CurlToCode => &["JS fetch", "Python", "PHP"],
            _ => &[],
        }
    }

    /// Input to show the tool with ("Sample").
    pub fn sample(self) -> &'static str {
        match self {
            Tool::HtmlToMarkdown => {
                "<h2>Plans</h2><table><tr><th>Plan</th><th>Price</th></tr>\
                 <tr><td>Basic</td><td>$10</td></tr><tr><td>Pro</td><td>$25</td></tr></table>\
                 <p>Prices <b>include</b> <a href=\"https://example.com/tax\">tax</a>.</p>"
            }
            Tool::MarkdownPreview => "# Title\n\nSome **bold** text and a list:\n\n- one\n- two\n",
            Tool::Base64 | Tool::Url | Tool::HtmlEntities | Tool::Hash | Tool::Case => {
                "Hello world & friends <3"
            }
            Tool::Backslash => "Line one\nLine \"two\"\twith a tab",
            Tool::UnixTime => "1761200000",
            Tool::JsonYaml => {
                "{\"name\": \"Bench\", \"tools\": [\"base64\", \"regex\"], \"ok\": true}"
            }
            Tool::LineSort => "pear\napple\nbanana\napple\n",
            Tool::Regex => "Mail john@example.com or anna.k@mail.ru by 2026-10-23.",
            Tool::UrlParser => "https://user@example.com:8443/path/to/page?q=rust&lang=en#top",
            Tool::Cron => "*/15 9-18 * * 1-5",
            Tool::CurlToCode => {
                "curl 'https://api.example.com/v1/items' -X POST -H 'Content-Type: application/json' \
                 -H 'Authorization: Bearer TOKEN' --data-raw '{\"name\":\"test\"}'"
            }
            Tool::Base64Image => {
                "iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAIAAABLbSncAAAAEUlEQVR4nGOIqXiEFTEMLQkAp75tgQ/pgtMAAAAASUVORK5CYII="
            }
            _ => "",
        }
    }
}

/// A transform tool's output for `input` in `mode` (an index into `modes()`).
pub fn transform(tool: Tool, input: &str, mode: usize) -> Result<String, String> {
    if input.trim().is_empty() {
        return Ok(String::new());
    }
    match tool {
        Tool::HtmlToMarkdown => markdown::html_to_markdown(input),
        Tool::Base64 => encode::base64(input, mode == 0),
        Tool::Url => encode::url(input, mode == 0),
        Tool::HtmlEntities => Ok(encode::html_entities(input, mode == 0)),
        Tool::Backslash => encode::backslash(input, mode == 0),
        Tool::UnixTime => convert::unix_time(input),
        Tool::JsonYaml => convert::json_yaml(input),
        Tool::Case => Ok(convert::cases(input)),
        Tool::LineSort => Ok(convert::sort_lines(input, mode)),
        Tool::Hash => Ok(generate::hashes(input)),
        Tool::UrlParser => parse::url(input),
        Tool::Cron => parse::cron(input),
        Tool::CurlToCode => parse::curl_to_code(input, mode),
        _ => Ok(String::new()),
    }
}

/// A generator's output (`mode` from `modes()`, `count` items).
pub fn generate(tool: Tool, mode: usize, count: usize) -> String {
    match tool {
        Tool::Password => generate::passwords([16, 20, 24, 32][mode.min(3)], count),
        Tool::Uuid => generate::uuids(count),
        Tool::Lorem => generate::lorem(mode, count),
        _ => String::new(),
    }
}

pub use encode::{decode_image, encode_image};
pub use parse::{RegexFlags, regex_matches};
