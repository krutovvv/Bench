//! Base64, percent-encoding, HTML entities, backslash escapes, Base64 images.

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};

/// Decoding takes standard and URL-safe alphabets, with or without padding, and ignores
/// whitespace (wrapped lines).
fn b64_decode(input: &str) -> Result<Vec<u8>, String> {
    let s: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    for engine in [&STANDARD, &URL_SAFE, &STANDARD_NO_PAD, &URL_SAFE_NO_PAD] {
        if let Ok(bytes) = engine.decode(&s) {
            return Ok(bytes);
        }
    }
    Err("Not valid Base64".into())
}

pub fn base64(input: &str, encode: bool) -> Result<String, String> {
    if encode {
        return Ok(STANDARD.encode(input.as_bytes()));
    }
    let bytes = b64_decode(input)?;
    String::from_utf8(bytes).map_err(|_| "Decoded bytes are not UTF-8 text".into())
}

/// RFC 3986 unreserved characters stay as they are.
const URL_KEEP: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

pub fn url(input: &str, encode: bool) -> Result<String, String> {
    if encode {
        return Ok(utf8_percent_encode(input, URL_KEEP).to_string());
    }
    // "+" is a space in form data.
    percent_decode_str(&input.replace('+', " "))
        .decode_utf8()
        .map(|s| s.into_owned())
        .map_err(|_| "Decoded bytes are not UTF-8 text".into())
}

pub fn html_entities(input: &str, encode: bool) -> String {
    if encode {
        let mut out = String::with_capacity(input.len());
        for c in input.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                '\'' => out.push_str("&#39;"),
                c => out.push(c),
            }
        }
        return out;
    }
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let decoded = rest.find(';').filter(|end| *end <= 12).and_then(|end| {
            let name = &rest[1..end];
            let c = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                "copy" => Some('©'),
                "reg" => Some('®'),
                "trade" => Some('™'),
                "hellip" => Some('…'),
                "mdash" => Some('—'),
                "ndash" => Some('–'),
                "laquo" => Some('«'),
                "raquo" => Some('»'),
                "euro" => Some('€'),
                _ => name
                    .strip_prefix("#x")
                    .or_else(|| name.strip_prefix("#X"))
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| name.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32),
            };
            c.map(|c| (c, end))
        });
        match decoded {
            Some((c, end)) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

pub fn backslash(input: &str, escape: bool) -> Result<String, String> {
    if escape {
        let mut out = String::with_capacity(input.len());
        for c in input.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        return Ok(out);
    }
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('0') => out.push('\0'),
            Some('u') => {
                // \uXXXX or \u{X…}
                let hex: String = if chars.clone().next() == Some('{') {
                    chars.next();
                    chars.by_ref().take_while(|c| *c != '}').collect()
                } else {
                    chars.by_ref().take(4).collect()
                };
                let c = u32::from_str_radix(&hex, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .ok_or_else(|| format!("Bad escape \\u{hex}"))?;
                out.push(c);
            }
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    Ok(out)
}

/// The image in a Base64 string or data URL: (bytes, file extension).
pub fn decode_image(input: &str) -> Result<(Vec<u8>, &'static str), String> {
    let data = input.trim();
    let data = data
        .strip_prefix("data:")
        .and_then(|rest| rest.split_once(',').map(|(_, d)| d))
        .unwrap_or(data);
    let bytes = b64_decode(data)?;
    let ext = match bytes.as_slice() {
        [0x89, b'P', b'N', b'G', ..] => "png",
        [0xFF, 0xD8, ..] => "jpg",
        [b'G', b'I', b'F', ..] => "gif",
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => "webp",
        b if b.starts_with(b"<svg") || b.starts_with(b"<?xml") => "svg",
        _ => return Err("Decoded bytes are not a PNG, JPEG, GIF, WebP or SVG image".into()),
    };
    Ok((bytes, ext))
}

/// A data URL for an image file's bytes.
pub fn encode_image(bytes: &[u8], ext: &str) -> String {
    let mime = match ext.to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "image/png",
    };
    format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let s = "Привет & <b>\"hi\"</b>\n\ttab";
        assert_eq!(base64(&base64(s, true).unwrap(), false).unwrap(), s);
        assert_eq!(url(&url(s, true).unwrap(), false).unwrap(), s);
        assert_eq!(html_entities(&html_entities(s, true), false), s);
        assert_eq!(backslash(&backslash(s, true).unwrap(), false).unwrap(), s);
    }

    #[test]
    fn decodes_variants() {
        assert_eq!(base64("aGk", false).unwrap(), "hi");
        assert_eq!(url("a+b%20c", false).unwrap(), "a b c");
        assert_eq!(
            html_entities("&#x41;&#66;&copy;&bogus;", false),
            "AB©&bogus;"
        );
        assert_eq!(backslash(r"A\u{1F600}", false).unwrap(), "A😀");
    }
}
