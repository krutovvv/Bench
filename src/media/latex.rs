//! LaTeX as Unicode text, so a formula reads in the answer's own font.


/// LaTeX as Unicode text: x², α, √x, ∑ᵢ, a/b. Fractions, text and roots are unwrapped here;
/// the symbols are unicodeit's.
pub fn latex_text(tex: &str) -> String {
    fn group(s: &str) -> Option<(String, &str)> {
        let s = s.trim_start();
        let rest = s.strip_prefix('{')?;
        let mut depth = 1;
        for (i, c) in rest.char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((rest[..i].to_string(), &rest[i + 1..]));
                    }
                }
                _ => {}
            }
        }
        None
    }
    let wrap = |s: String| {
        if s.chars().count() <= 1 || s.chars().all(|c| c.is_alphanumeric()) {
            s
        } else {
            format!("({s})")
        }
    };
    let mut out = String::new();
    let mut rest = tex;
    while let Some(at) = rest.find('\\') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let name: String = after
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();
        let tail = &after[name.len()..];
        match name.as_str() {
            "frac" | "dfrac" | "tfrac" => match group(tail).and_then(|(a, t)| Some((a, group(t)?)))
            {
                Some((a, (b, t))) => {
                    out.push_str(&format!(
                        "{}/{}",
                        wrap(latex_text(&a)),
                        wrap(latex_text(&b))
                    ));
                    rest = t;
                    continue;
                }
                None => out.push_str("\\frac"),
            },
            "sqrt" => match group(tail) {
                Some((a, t)) => {
                    out.push_str(&format!("√{}", wrap(latex_text(&a))));
                    rest = t;
                    continue;
                }
                None => out.push_str("\\sqrt"),
            },
            "text" | "mathrm" | "operatorname" | "textrm" | "mathit" | "textbf" | "mathbf" => {
                match group(tail) {
                    Some((a, t)) => {
                        out.push_str(&a);
                        rest = t;
                        continue;
                    }
                    None => {}
                }
            }
            "left" | "right" | "displaystyle" => {
                rest = tail;
                continue;
            }
            "" => {
                // \, \; \! \  : spacing.
                let c = after.chars().next().unwrap_or(' ');
                if matches!(c, ',' | ';' | ':' | ' ' | '!') {
                    out.push(if c == '!' { '\u{200b}' } else { ' ' });
                    rest = &after[c.len_utf8()..];
                    continue;
                }
                out.push('\\');
                rest = after;
                continue;
            }
            _ => {}
        }
        out.push('\\');
        out.push_str(&name);
        rest = tail;
    }
    out.push_str(rest);
    unicodeit::replace(&out)
        .replace(['{', '}'], "")
        .replace("\u{200b}", "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latex_reads_as_text() {
        assert_eq!(latex_text("E = mc^2"), "E = mc²");
        assert_eq!(latex_text("\\frac{a+b}{2}"), "(a+b)/2");
        assert_eq!(latex_text("\\sqrt{x} \\leq \\alpha"), "√x ≤ α");
        assert_eq!(latex_text("\\text{cost} \\times 3"), "cost × 3");
        assert_eq!(latex_text("\\int_0^1 f(x)\\,dx"), "∫₀¹ f(x) dx");
    }
}
