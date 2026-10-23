//! Unix time, JSON ⇄ YAML, string cases, line sorting.

use chrono::{DateTime, Datelike, Local, NaiveDateTime, TimeZone, Utc};
use heck::{
    ToKebabCase, ToLowerCamelCase, ToShoutySnakeCase, ToSnakeCase, ToTitleCase, ToTrainCase,
    ToUpperCamelCase,
};

/// Zones shown under every time, after the local one; a zone typed after the input is added.
const ZONES: &[&str] = &[
    "UTC",
    "Europe/Moscow",
    "Europe/London",
    "Europe/Berlin",
    "America/New_York",
    "America/Los_Angeles",
    "Asia/Tokyo",
];

/// Seconds or milliseconds since 1970 (by size), or a date ("2026-10-23 14:05[:30]", local; an
/// RFC 3339 / ISO 8601 date with its own offset). An IANA zone name may follow
/// ("1761200000 Asia/Dubai").
pub fn unix_time(input: &str) -> Result<String, String> {
    let input = input.trim();
    let (value, extra_zone) = match input.rsplit_once(char::is_whitespace) {
        Some((v, z)) if z.contains('/') && z.parse::<chrono_tz::Tz>().is_ok() => {
            (v.trim(), Some(z))
        }
        _ => (input, None),
    };
    let at: DateTime<Utc> = if let Ok(n) = value.replace('_', "").parse::<i64>() {
        let ms = n.abs() >= 100_000_000_000;
        let dt = if ms {
            Utc.timestamp_millis_opt(n).single()
        } else {
            Utc.timestamp_opt(n, 0).single()
        };
        dt.ok_or("Out of range")?
    } else if let Ok(dt) = DateTime::parse_from_rfc3339(value) {
        dt.with_timezone(&Utc)
    } else {
        let naive = [
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%d %H:%M",
            "%Y-%m-%dT%H:%M:%S",
            "%Y-%m-%dT%H:%M",
        ]
        .iter()
        .find_map(|f| NaiveDateTime::parse_from_str(value, f).ok())
        .or_else(|| {
            chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
        })
        .ok_or("Not a Unix time or a date (try 1761200000 or 2026-10-23 14:05)")?;
        Local
            .from_local_datetime(&naive)
            .single()
            .ok_or("That local time does not exist (a clock change)")?
            .with_timezone(&Utc)
    };
    let local = at.with_timezone(&Local);
    let mut out = vec![
        format!("Unix seconds   {}", at.timestamp()),
        format!("Unix millis    {}", at.timestamp_millis()),
        format!(
            "UTC (ISO 8601) {}",
            at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        ),
        format!(
            "Local          {}",
            local.format("%a %d %b %Y %H:%M:%S %:z")
        ),
        format!("Relative       {}", relative(at)),
        format!(
            "Day / week     day {} of the year, ISO week {}",
            local.ordinal(),
            local.iso_week().week()
        ),
        String::new(),
    ];
    for name in extra_zone.into_iter().chain(ZONES.iter().copied()) {
        if let Ok(tz) = name.parse::<chrono_tz::Tz>() {
            let t = at.with_timezone(&tz);
            out.push(format!(
                "{:<20} {}",
                name,
                t.format("%a %d %b %Y %H:%M:%S %:z")
            ));
        }
    }
    Ok(out.join("\n"))
}

fn relative(at: DateTime<Utc>) -> String {
    let secs = (Utc::now() - at).num_seconds();
    let (n, future) = (secs.unsigned_abs(), secs < 0);
    let (k, unit) = match n {
        0..=59 => (n, "second"),
        60..=3599 => (n / 60, "minute"),
        3600..=86_399 => (n / 3600, "hour"),
        86_400..=2_591_999 => (n / 86_400, "day"),
        2_592_000..=31_535_999 => (n / 2_592_000, "month"),
        _ => (n / 31_536_000, "year"),
    };
    let span = format!("{k} {unit}{}", if k == 1 { "" } else { "s" });
    if future {
        format!("in {span}")
    } else {
        format!("{span} ago")
    }
}

/// JSON in → YAML out, anything else → parsed as YAML, JSON out (pretty).
pub fn json_yaml(input: &str) -> Result<String, String> {
    let t = input.trim_start();
    if t.starts_with('{') || t.starts_with('[') {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(input) {
            return serde_yaml::to_string(&v).map_err(|e| e.to_string());
        }
    }
    let v: serde_json::Value =
        serde_yaml::from_str(input).map_err(|e| format!("Neither JSON nor YAML: {e}"))?;
    serde_json::to_string_pretty(&v).map_err(|e| e.to_string())
}

pub fn cases(input: &str) -> String {
    let s = input.trim();
    [
        ("camelCase", s.to_lower_camel_case()),
        ("PascalCase", s.to_upper_camel_case()),
        ("snake_case", s.to_snake_case()),
        ("CONSTANT_CASE", s.to_shouty_snake_case()),
        ("kebab-case", s.to_kebab_case()),
        ("Train-Case", s.to_train_case()),
        ("Title Case", s.to_title_case()),
        ("lower case", s.to_lowercase()),
        ("UPPER CASE", s.to_uppercase()),
    ]
    .iter()
    .map(|(name, v)| format!("{name:<14} {v}"))
    .collect::<Vec<_>>()
    .join("\n")
}

/// Lines trimmed, empty ones and duplicates dropped; `mode`: 0 A→Z, 1 Z→A, 2 keep the order.
pub fn sort_lines(input: &str, mode: usize) -> String {
    let mut seen = std::collections::HashSet::new();
    let mut lines: Vec<&str> = input
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && seen.insert(l.to_string()))
        .collect();
    match mode {
        0 => lines.sort_by_key(|l| l.to_lowercase()),
        1 => {
            lines.sort_by_key(|l| l.to_lowercase());
            lines.reverse();
        }
        _ => {}
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times() {
        let out = unix_time("0").unwrap();
        assert!(out.contains("1970-01-01T00:00:00Z"));
        assert!(
            unix_time("1761200000000")
                .unwrap()
                .contains("Unix seconds   1761200000")
        );
        assert!(
            unix_time("2026-10-23T10:00:00Z")
                .unwrap()
                .contains("Unix seconds   1792749600")
        );
        assert!(
            unix_time("1761200000 Asia/Dubai")
                .unwrap()
                .contains("Asia/Dubai")
        );
        assert!(unix_time("tomorrow").is_err());
    }

    #[test]
    fn data() {
        assert_eq!(json_yaml("{\"a\": 1}").unwrap().trim(), "a: 1");
        assert!(json_yaml("a: 1\nb: [x]").unwrap().contains("\"b\""));
        assert!(cases("hello world").contains("helloWorld"));
        assert_eq!(sort_lines("b\na\n\nb\n", 0), "a\nb");
        assert_eq!(sort_lines("b\na\nb", 2), "b\na");
    }
}
