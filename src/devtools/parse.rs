//! URL parser, cron parser, cURL → code, regex tester.

use std::str::FromStr as _;

pub fn url(input: &str) -> Result<String, String> {
    let u = url::Url::parse(input.trim()).map_err(|e| format!("Not a URL: {e}"))?;
    let mut out = vec![format!("Scheme    {}", u.scheme())];
    if !u.username().is_empty() {
        out.push(format!("User      {}", u.username()));
    }
    if let Some(h) = u.host_str() {
        out.push(format!("Host      {h}"));
    }
    if let Some(p) = u.port_or_known_default() {
        let note = if u.port().is_none() { " (default)" } else { "" };
        out.push(format!("Port      {p}{note}"));
    }
    out.push(format!("Path      {}", u.path()));
    if let Some(f) = u.fragment() {
        out.push(format!("Fragment  {f}"));
    }
    let pairs: Vec<(String, String)> = u.query_pairs().into_owned().collect();
    if !pairs.is_empty() {
        out.push(String::new());
        out.push("Query".into());
        for (k, v) in &pairs {
            out.push(format!("  {k} = {v}"));
        }
        let mut obj = serde_json::Map::new();
        for (k, v) in pairs {
            obj.insert(k, serde_json::Value::String(v));
        }
        out.push(String::new());
        out.push(serde_json::to_string_pretty(&obj).unwrap_or_default());
    }
    Ok(out.join("\n"))
}

pub fn cron(input: &str) -> Result<String, String> {
    let expr = input.trim();
    let c = croner::Cron::from_str(expr).map_err(|e| format!("Not a cron expression: {e}"))?;
    let fields: Vec<&str> = expr.split_whitespace().collect();
    let names = if fields.len() == 6 {
        &[
            "Seconds",
            "Minutes",
            "Hours",
            "Day of month",
            "Months",
            "Day of week",
        ][..]
    } else {
        &["Minutes", "Hours", "Day of month", "Months", "Day of week"][..]
    };
    let mut out = vec![c.describe(), String::new()];
    for (name, f) in names.iter().zip(&fields) {
        out.push(format!(
            "{name:<13} {}",
            if *f == "*" { "(every)" } else { f }
        ));
    }
    out.push(String::new());
    out.push("Next runs (local time)".into());
    for t in c.iter_after(chrono::Local::now()).take(6) {
        out.push(format!("  {}", t.format("%a %d %b %Y %H:%M")));
    }
    Ok(out.join("\n"))
}

/// A shell command line split into words: '…' and "…" quotes, backslash escapes and
/// backslash-newline continuations.
fn shell_words(s: &str) -> Vec<String> {
    let mut words = vec![];
    let mut cur = String::new();
    let mut started = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                started = true;
                for c in chars.by_ref() {
                    if c == '\'' {
                        break;
                    }
                    cur.push(c);
                }
            }
            '"' => {
                started = true;
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => {
                            if let Some(n) = chars.next() {
                                cur.push(n);
                            }
                        }
                        c => cur.push(c),
                    }
                }
            }
            '\\' => match chars.next() {
                Some('\n') | None => {}
                Some(n) => {
                    started = true;
                    cur.push(n);
                }
            },
            c if c.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            c => {
                started = true;
                cur.push(c);
            }
        }
    }
    if started {
        words.push(cur);
    }
    words
}

struct Request {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
    user: Option<String>,
}

fn parse_curl(input: &str) -> Result<Request, String> {
    let words = shell_words(input);
    let mut it = words.iter().skip_while(|w| *w != "curl").skip(1).peekable();
    if words.first().map(String::as_str) != Some("curl") && !words.iter().any(|w| w == "curl") {
        return Err("Start with curl …".into());
    }
    let mut r = Request {
        method: String::new(),
        url: String::new(),
        headers: vec![],
        body: None,
        user: None,
    };
    while let Some(w) = it.next() {
        let mut arg = || it.next().cloned().unwrap_or_default();
        match w.as_str() {
            "-X" | "--request" => r.method = arg().to_uppercase(),
            "-H" | "--header" => {
                let h = arg();
                if let Some((k, v)) = h.split_once(':') {
                    r.headers.push((k.trim().to_string(), v.trim().to_string()));
                }
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" | "--data-ascii"
            | "--data-urlencode" => {
                let d = arg();
                r.body = Some(match r.body.take() {
                    Some(prev) => format!("{prev}&{d}"),
                    None => d,
                });
            }
            "--json" => {
                r.body = Some(arg());
                r.headers
                    .push(("Content-Type".into(), "application/json".into()));
                r.headers.push(("Accept".into(), "application/json".into()));
            }
            "-u" | "--user" => r.user = Some(arg()),
            "-b" | "--cookie" => r.headers.push(("Cookie".into(), arg())),
            "-A" | "--user-agent" => r.headers.push(("User-Agent".into(), arg())),
            "-e" | "--referer" => r.headers.push(("Referer".into(), arg())),
            "--url" => r.url = arg(),
            // Options with a value that change nothing in the code.
            "-o" | "--output" | "-m" | "--max-time" | "--connect-timeout" | "-w"
            | "--write-out" => {
                arg();
            }
            w if w.starts_with('-') => {} // -L, -s, -k, --compressed, …
            w => {
                if r.url.is_empty() {
                    r.url = w.to_string();
                }
            }
        }
    }
    if r.url.is_empty() {
        return Err("No URL in the command".into());
    }
    if r.method.is_empty() {
        r.method = if r.body.is_some() { "POST" } else { "GET" }.into();
    }
    Ok(r)
}

fn quoted(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

/// `mode`: 0 JavaScript fetch, 1 Python requests, 2 PHP curl.
pub fn curl_to_code(input: &str, mode: usize) -> Result<String, String> {
    let r = parse_curl(input)?;
    let mut headers = r.headers.clone();
    if let Some(user) = &r.user {
        use base64::Engine as _;
        let token = base64::engine::general_purpose::STANDARD.encode(user.as_bytes());
        headers.push(("Authorization".into(), format!("Basic {token}")));
    }
    Ok(match mode {
        1 => {
            let mut s = String::from("import requests\n\n");
            if !headers.is_empty() {
                s.push_str("headers = {\n");
                for (k, v) in &headers {
                    s.push_str(&format!("    {}: {},\n", quoted(k), quoted(v)));
                }
                s.push_str("}\n\n");
            }
            if let Some(b) = &r.body {
                s.push_str(&format!("data = {}\n\n", quoted(b)));
            }
            s.push_str(&format!(
                "response = requests.request({}, {}",
                quoted(&r.method),
                quoted(&r.url)
            ));
            if !headers.is_empty() {
                s.push_str(", headers=headers");
            }
            if r.body.is_some() {
                s.push_str(", data=data");
            }
            s.push_str(")\nprint(response.status_code)\nprint(response.text)\n");
            s
        }
        2 => {
            let mut s = format!(
                "<?php\n$ch = curl_init({});\ncurl_setopt($ch, CURLOPT_CUSTOMREQUEST, {});\n\
                 curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);\n",
                quoted(&r.url),
                quoted(&r.method)
            );
            if !headers.is_empty() {
                s.push_str("curl_setopt($ch, CURLOPT_HTTPHEADER, [\n");
                for (k, v) in &headers {
                    s.push_str(&format!("    {},\n", quoted(&format!("{k}: {v}"))));
                }
                s.push_str("]);\n");
            }
            if let Some(b) = &r.body {
                s.push_str(&format!(
                    "curl_setopt($ch, CURLOPT_POSTFIELDS, {});\n",
                    quoted(b)
                ));
            }
            s.push_str("$response = curl_exec($ch);\ncurl_close($ch);\necho $response;\n");
            s
        }
        _ => {
            let mut opts = vec![format!("  method: {}", quoted(&r.method))];
            if !headers.is_empty() {
                let hs: Vec<String> = headers
                    .iter()
                    .map(|(k, v)| format!("    {}: {}", quoted(k), quoted(v)))
                    .collect();
                opts.push(format!("  headers: {{\n{},\n  }}", hs.join(",\n")));
            }
            if let Some(b) = &r.body {
                opts.push(format!("  body: {}", quoted(b)));
            }
            format!(
                "const response = await fetch({}, {{\n{},\n}});\nconsole.log(response.status);\nconsole.log(await response.text());\n",
                quoted(&r.url),
                opts.join(",\n")
            )
        }
    })
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RegexFlags {
    pub case_insensitive: bool,
    pub multi_line: bool,
    pub dot_all: bool,
}

/// The matches of `pattern` in `text`: (count, a readable list with groups, byte ranges).
pub fn regex_matches(
    pattern: &str,
    text: &str,
    flags: RegexFlags,
) -> Result<(usize, String, Vec<std::ops::Range<usize>>), String> {
    if pattern.is_empty() {
        return Ok((0, String::new(), vec![]));
    }
    let re = regex::RegexBuilder::new(pattern)
        .case_insensitive(flags.case_insensitive)
        .multi_line(flags.multi_line)
        .dot_matches_new_line(flags.dot_all)
        .size_limit(1 << 22)
        .build()
        .map_err(|e| e.to_string())?;
    let names: Vec<Option<&str>> = re.capture_names().collect();
    let mut out = vec![];
    let mut ranges = vec![];
    let mut n = 0;
    for caps in re.captures_iter(text) {
        n += 1;
        let m = caps.get(0).expect("group 0 always matches");
        ranges.push(m.range());
        if n > 500 {
            continue;
        }
        let line = text[..m.start()].matches('\n').count() + 1;
        out.push(format!("#{n}  line {line}  {}", m.as_str()));
        for (k, name) in names.iter().enumerate().skip(1) {
            let v = caps.get(k).map_or("—", |g| g.as_str());
            match name {
                Some(name) => out.push(format!("    {name} = {v}")),
                None => out.push(format!("    ${k} = {v}")),
            }
        }
    }
    if n > 500 {
        out.push(format!("… and {} more", n - 500));
    }
    Ok((n, out.join("\n"), ranges))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_and_cron() {
        let out = url("https://a.com:8443/p?q=1&x=y%20z#top").unwrap();
        assert!(out.contains("Port      8443") && out.contains("x = y z") && out.contains("top"));
        let c = cron("*/15 9-18 * * 1-5").unwrap();
        assert!(c.contains("Next runs"));
        assert!(cron("nonsense").is_err());
    }

    #[test]
    fn curl() {
        let cmd = "curl 'https://x.io/a' -H 'A: 1' --data-raw '{\"k\":\"v\"}' \\\n  --compressed";
        let js = curl_to_code(cmd, 0).unwrap();
        assert!(
            js.contains("\"POST\"") && js.contains("\"A\": \"1\"") && js.contains("{\\\"k\\\"")
        );
        assert!(
            curl_to_code(cmd, 1)
                .unwrap()
                .contains("requests.request(\"POST\"")
        );
        assert!(
            curl_to_code("curl -u me:pw https://x.io", 2)
                .unwrap()
                .contains("Basic bWU6cHc=")
        );
    }

    #[test]
    fn regex() {
        let (n, list, ranges) =
            regex_matches(r"(?P<user>\w+)@(\w+)", "a@b c@d", RegexFlags::default()).unwrap();
        assert_eq!(n, 2);
        assert!(list.contains("user = a") && list.contains("$2 = d"));
        assert_eq!(ranges[1], 4..7);
        assert!(regex_matches("(", "x", RegexFlags::default()).is_err());
    }
}
