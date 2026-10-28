//! Unified diffs from file tools: their line stats.

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

#[cfg(test)]
mod tests {
    #[test]
    fn counts_lines() {
        let diff = "--- a/f\n+++ b/f\n@@ -1,4 +1,5 @@\n a\n-b\n+B\n c\n d\n+e\n";
        assert_eq!(super::stats(diff), (2, 1));
    }
}
