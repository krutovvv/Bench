//! Numbers and text as the UI says them.

/// "1 action", "3 actions" (`_few` is kept for call-site symmetry).
pub fn plural(n: usize, one: &str, _few: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Seconds as "42s" or "3:07".
pub fn clock(secs: u64) -> String {
    if secs >= 60 {
        format!("{}:{:02}", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

/// 1234 → "1k", 2500000 → "2.5M".
pub fn compact(n: u64) -> String {
    match n {
        n if n >= 1_000_000 => format!("{:.1}M", n as f64 / 1e6),
        n if n >= 1_000 => format!("{}k", n / 1000),
        n => n.to_string(),
    }
}

/// One line, cut to `n` characters with "…".
pub fn clip(s: &str, n: usize) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > n {
        format!("{}…", one.chars().take(n).collect::<String>())
    } else {
        one
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(plural(1, "action", "", "actions"), "1 action");
        assert_eq!(plural(3, "action", "", "actions"), "3 actions");
        assert_eq!(clock(42), "42s");
        assert_eq!(clock(187), "3:07");
        assert_eq!(compact(1234), "1k");
        assert_eq!(compact(2_500_000), "2.5M");
        assert_eq!(clip("a  b\nc", 10), "a b c");
        assert_eq!(clip("abcdef", 3), "abc…");
    }
}
