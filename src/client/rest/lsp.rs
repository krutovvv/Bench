//! A project's language servers through its agent (`agent/src/modules/lsp.ts`), for the Explorer's
//! editor: a file's problems, the hover at a place, where a symbol is defined; the project's
//! Problems. The file's text is
//! sent as the editor has it, saved or not.

use super::send;
use crate::client::connection::Connection;
use anyhow::Result;
use serde_json::{Value, json};

/// The file asked about: its project (the id at its agent), its machine, its path, its text.
pub struct Asked<'a> {
    pub project: &'a str,
    pub machine: Option<&'a str>,
    pub path: &'a str,
    pub text: &'a str,
}

impl Asked<'_> {
    fn body(&self, at: Option<lsp_types::Position>) -> Value {
        let mut b = json!({
            "project": self.project,
            "machine": self.machine,
            "path": self.path,
            "text": self.text,
        });
        if let Some(at) = at {
            b["line"] = at.line.into();
            b["character"] = at.character.into();
        }
        b
    }
}

/// Its problems; None: no server of the project knows this kind of file (or none is up yet).
pub fn diagnostics(
    conn: &Connection,
    profile: &str,
    a: &Asked,
) -> Result<Option<Vec<lsp_types::Diagnostic>>> {
    let v = send(conn, "POST", "/api/lsp/diagnostics", profile, a.body(None), 30)?;
    if !v["known"].as_bool().unwrap_or(false) {
        return Ok(None);
    }
    Ok(Some(serde_json::from_value(v["problems"].clone())?))
}

/// Every file's errors and warnings the project's servers know now: (path, its problems).
pub fn problems(
    conn: &Connection,
    profile: &str,
    project: &str,
) -> Result<Vec<(String, Vec<lsp_types::Diagnostic>)>> {
    let v = send(conn, "POST", "/api/lsp/problems", profile, json!({ "project": project }), 30)?;
    Ok(v["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let path = f["path"].as_str()?.to_string();
            Some((path, serde_json::from_value(f["problems"].clone()).ok()?))
        })
        .collect())
}

/// What a server says of the symbol at `at`, as Markdown; empty: nothing.
pub fn hover(
    conn: &Connection,
    profile: &str,
    a: &Asked,
    at: lsp_types::Position,
) -> Result<String> {
    let v = send(conn, "POST", "/api/lsp/hover", profile, a.body(Some(at)), 10)?;
    Ok(v["markdown"].as_str().unwrap_or_default().to_string())
}

/// Where the symbol at `at` is defined: a path on the file's machine and the range there.
pub fn definition(
    conn: &Connection,
    profile: &str,
    a: &Asked,
    at: lsp_types::Position,
) -> Result<Vec<(String, lsp_types::Range)>> {
    let v = send(conn, "POST", "/api/lsp/definition", profile, a.body(Some(at)), 10)?;
    Ok(v["places"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let path = p["path"].as_str()?.to_string();
            let range = serde_json::from_value(p["range"].clone()).ok()?;
            Some((path, range))
        })
        .collect())
}
