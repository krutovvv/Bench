//! Rich text → Markdown: the HTML a browser, Word or Mail puts on the clipboard (or the RTF that
//! Notes, Pages and TextEdit put there), converted to clean Markdown — tables included.

use std::io::Write as _;
use std::process::{Command, Stdio};

pub fn html_to_markdown(html: &str) -> Result<String, String> {
    let converter = htmd::HtmlToMarkdown::builder()
        .options(htmd::options::Options {
            bullet_list_marker: htmd::options::BulletListMarker::Dash,
            ul_bullet_spacing: 1,
            ol_number_spacing: 1,
            ..Default::default()
        })
        .skip_tags(vec!["script", "style", "head", "meta", "link", "noscript"])
        .build();
    // Link tooltips (`title="…"`, on every Wikipedia link) only repeat the text.
    let titles = regex::Regex::new(r#"\stitle\s*=\s*("[^"]*"|'[^']*')"#).expect("valid regex");
    let html = titles.replace_all(html, "");
    let md = converter.convert(&html).map_err(|e| e.to_string())?;
    // Converters leave runs of blank lines where layout divs were: one is enough.
    let mut out = String::with_capacity(md.len());
    let mut blank = 0;
    let mut fenced = false;
    for line in md.lines() {
        let line = line.trim_end();
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        blank = if line.is_empty() { blank + 1 } else { 0 };
        if blank < 2 {
            match (fenced, compact_table_row(line)) {
                (false, Some(row)) => out.push_str(&row),
                _ => out.push_str(line),
            }
            out.push('\n');
        }
    }
    Ok(out.trim().to_string())
}

/// A table row without the padding htmd adds to line columns up: one long cell would make
/// every row of a page's table hundreds of spaces wide. `| a  | b |` → `| a | b |`,
/// `| :---- |` → `| :--- |`. None for a line that isn't a table row.
fn compact_table_row(line: &str) -> Option<String> {
    let inner = line.strip_prefix('|')?.strip_suffix('|')?;
    let cells: Vec<String> = inner
        .split('|')
        .map(|cell| {
            let cell = cell.split_whitespace().collect::<Vec<_>>().join(" ");
            let dashes = cell.trim_matches(':');
            if !dashes.is_empty() && dashes.chars().all(|c| c == '-') {
                let left = if cell.starts_with(':') { ":" } else { "" };
                let right = if cell.ends_with(':') { ":" } else { "" };
                format!("{left}---{right}")
            } else {
                cell
            }
        })
        .collect();
    Some(format!("| {} |", cells.join(" | ")))
}

/// The clipboard's rich text as HTML: its HTML flavor, else its RTF converted by macOS's
/// `textutil`. None when it holds plain text only.
pub fn clipboard_html() -> Option<String> {
    if let Some(bytes) = clipboard_flavor("HTML") {
        return Some(String::from_utf8_lossy(&bytes).into_owned());
    }
    let rtf = clipboard_flavor("RTF ")?;
    let mut child = Command::new("/usr/bin/textutil")
        .args(["-stdin", "-format", "rtf", "-convert", "html", "-stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    child.stdin.take()?.write_all(&rtf).ok()?;
    let out = child.wait_with_output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// One flavor of the clipboard through AppleScript, which prints it as «data HTML3C68…» (hex).
fn clipboard_flavor(class: &str) -> Option<Vec<u8>> {
    let out = Command::new("/usr/bin/osascript")
        .args(["-e", &format!("the clipboard as «class {class}»")])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_data_literal(&String::from_utf8_lossy(&out.stdout))
}

fn parse_data_literal(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    let start = s.find("«data ")? + "«data ".len();
    let hex = s[start..].trim_end_matches('»');
    let hex = hex.get(4..)?; // the 4-character class code
    (0..hex.len() / 2)
        .map(|i| u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_and_text() {
        let md = html_to_markdown(
            "<h2>Plans</h2><table><tr><th>Plan</th><th>Price</th></tr>\
             <tr><td>Basic</td><td>$10</td></tr></table><ul><li>a</li><li>b</li></ul>\
             <p>x <b>y</b> <a href=\"https://e.com\" title=\"Z page\">z</a></p><script>bad()</script>",
        )
        .unwrap();
        assert!(md.contains("## Plans"));
        // Rows without the column padding.
        assert!(md.contains("| Plan | Price |\n| --- | --- |\n| Basic | $10 |"));
        assert!(md.contains("- a\n- b"));
        assert!(md.contains("**y**") && md.contains("[z](https://e.com)"));
        assert!(!md.contains("bad()"));
    }

    #[test]
    fn data_literal() {
        assert_eq!(parse_data_literal("«data HTML3C623E»\n").unwrap(), b"<b>");
        assert!(parse_data_literal("plain").is_none());
    }
}
