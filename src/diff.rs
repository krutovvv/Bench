//! Unified diffs from file tools: line stats and reconstructing the version before them.

/// (+added, -removed) lines of a unified diff.
pub fn stats(diff: &str) -> (usize, usize) {
    let mut add = 0;
    let mut del = 0;
    for l in diff.lines() {
        if l.starts_with("+++") || l.starts_with("---") {
            continue;
        }
        if l.starts_with('+') {
            add += 1;
        } else if l.starts_with('-') {
            del += 1;
        }
    }
    (add, del)
}

struct Hunk {
    new_start: usize,
    /// (op, text): ' ' context, '+' added, '-' removed
    lines: Vec<(char, String)>,
}

fn hunks(diff: &str) -> Vec<Hunk> {
    let mut out: Vec<Hunk> = vec![];
    for l in diff.lines() {
        if let Some(rest) = l.strip_prefix("@@ ") {
            // @@ -a,b +c,d @@
            let new_start = rest
                .split_whitespace()
                .find(|p| p.starts_with('+'))
                .and_then(|p| p[1..].split(',').next())
                .and_then(|n| n.parse::<usize>().ok())
                .unwrap_or(1);
            out.push(Hunk {
                new_start,
                lines: vec![],
            });
        } else if l.starts_with("+++") || l.starts_with("---") || l.starts_with('\\') {
            continue;
        } else if let Some(h) = out.last_mut() {
            let (op, text) = match l.chars().next() {
                Some(c @ ('+' | '-' | ' ')) => (c, l[1..].to_string()),
                _ => (' ', l.to_string()),
            };
            h.lines.push((op, text));
        }
    }
    out
}

/// Undo one diff on the text it produced. None if the text no longer matches.
fn reverse_one(after: &str, diff: &str) -> Option<String> {
    let mut lines: Vec<String> = after.lines().map(str::to_string).collect();
    let trailing_newline = after.ends_with('\n');
    // Bottom-up so earlier hunks keep their line numbers.
    for h in hunks(diff).into_iter().rev() {
        let new_part: Vec<&String> = h
            .lines
            .iter()
            .filter(|(op, _)| *op != '-')
            .map(|(_, t)| t)
            .collect();
        let old_part: Vec<String> = h
            .lines
            .iter()
            .filter(|(op, _)| *op != '+')
            .map(|(_, t)| t.clone())
            .collect();
        let start = h.new_start.saturating_sub(1);
        let end = start + new_part.len();
        if end > lines.len()
            || lines[start..end]
                .iter()
                .zip(&new_part)
                .any(|(a, b)| a != *b)
        {
            return None;
        }
        lines.splice(start..end, old_part);
    }
    let mut text = lines.join("\n");
    if trailing_newline {
        text.push('\n');
    }
    Some(text)
}

/// The file before a series of diffs (applied in order), given its current text.
pub fn before(current: &str, diffs: &[String]) -> Option<String> {
    let mut text = current.to_string();
    for d in diffs.iter().rev() {
        text = reverse_one(&text, d)?;
    }
    Some(text)
}

/// The whole file as one unified diff (every unchanged line kept as context), so a viewer
/// can show the full text with the changes highlighted in place.
pub fn full(path: &str, before: &str, after: &str) -> String {
    similar::TextDiff::from_lines(before, after)
        .unified_diff()
        .context_radius(usize::MAX / 4)
        .header(&format!("a/{path}"), &format!("b/{path}"))
        .to_string()
}

/// A combined unified diff between two texts' hunks is not needed: the session diff is
/// simply the tool diffs in order.
pub fn joined(diffs: &[String]) -> String {
    diffs.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverses_a_patch() {
        let before_text = "a\nb\nc\nd\n";
        let after_text = "a\nB\nc\nd\ne\n";
        let diff = "--- a/f\n+++ b/f\n@@ -1,4 +1,5 @@\n a\n-b\n+B\n c\n d\n+e\n";
        assert_eq!(
            before(after_text, &[diff.to_string()]).as_deref(),
            Some(before_text)
        );
        assert_eq!(stats(diff), (2, 1));
    }

    #[test]
    fn refuses_when_file_moved_on() {
        let diff = "@@ -1,1 +1,1 @@\n-x\n+y\n";
        assert!(before("z\n", &[diff.to_string()]).is_none());
    }

    #[test]
    fn full_diff_parses_with_every_line() {
        let full = full("src/x.rs", "a\nb\nc\n", "a\nB\nc\nd\n");
        let files = gpui_kit::component::diff::DiffFile::parse(&full).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!((files[0].additions(), files[0].deletions()), (2, 1));
        assert_eq!(full.lines().filter(|l| l.starts_with(' ')).count(), 2);
        assert!(files[0].path().ends_with("src/x.rs"));
    }
}
