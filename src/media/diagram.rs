//! Mermaid diagrams drawn as SVG in plain Rust, cached on disk by source and theme.

use std::path::PathBuf;
use anyhow::{Result, bail};
use crate::client::connection::state_dir;
use super::hash_of;

/// A diagram as an SVG file and its size in points. Cached on disk by source and theme.
pub type Diagram = (PathBuf, (f32, f32));

pub fn mermaid_svg(code: &str, dark: bool) -> Result<Diagram> {
    let dir = state_dir().join("diagrams");
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(format!(
        "{}.svg",
        hash_of(&[code, if dark { "dark" } else { "light" }])
    ));
    let svg = match std::fs::read_to_string(&file) {
        Ok(svg) => svg,
        Err(_) => {
            let mut options = mermaid_rs_renderer::RenderOptions::modern();
            if dark {
                options.theme = mermaid_rs_renderer::Theme::dark();
            }
            let svg = mermaid_rs_renderer::render_with_options(code, options)?;
            std::fs::write(&file, &svg)?;
            svg
        }
    };
    let head = &svg[..svg.find('>').unwrap_or(svg.len())];
    let num = |attr: &str| -> Option<f32> {
        let at = head.find(attr)? + attr.len();
        head[at..].split('"').next()?.parse().ok()
    };
    match (num(" width=\""), num(" height=\"")) {
        (Some(w), Some(h)) if w > 0. && h > 0. => Ok((file, (w, h))),
        _ => bail!("diagram has no size"),
    }
}
