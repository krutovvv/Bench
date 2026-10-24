# Working as the Hermes agent: tools, builds, edits

How to use your tools while building a Bench — each line is a failure that already happened.

- **Builds take time.** The first build of a new app compiles gpui: up to ~4 min (apps from
  `scripts/new-app.sh` share one build folder, so later apps start in seconds); a release build
  (`scripts/bundle-macos.sh`) ~5 min the first time. Run `cargo build` in the **foreground with
  `timeout: 600`**. If you do start one in the background, do not edit files until it finished —
  it would compile a half-written mix and report errors that are already gone.
- **Build after every file you write**, not after six. LSP diagnostics on a new project are empty
  or "skipped" for the first 1–2 minutes while rust-analyzer indexes: that does NOT mean "no
  errors". `cargo build` is the truth.
- **Tool arguments that failed before:** booleans are `true`/`false`, not `"True"`;
  `write_file` content is a string; tools that are not in your list (e.g. `process_manage`)
  are called through `tool_call` with `{"calls": [{"name": "...", "arguments": {...}}]}` —
  `tool_describe` shows their arguments. A background process's wait is capped at 180 s: don't
  wait on it, let its completion notice come.
- Edit code with `patch` (exact old → new text). Use `write_file` only for a new file, or after
  reading the WHOLE file (a partial read makes Hermes refuse the overwrite). Never edit with
  `sed -i`, Python scripts or regexes.
- After each `write_file` / `patch`, read the **LSP diagnostics** in its result (rust-analyzer):
  fix every error before the next change. A result may arrive a little later, with your next
  tool call. `cargo build` is the final word.
- Read files with `read_file`, not `sed -n` / `cat`.
- **rust-analyzer as tools** (MCP server `rust_lsp`, every Rust project in `~/Projects`; the tools
  are named `mcp__rust_lsp__hover`, `…__definition`, `…__references`, `…__diagnostics`,
  `…__rename_symbol`):
  - `hover` (file, line, column, 1-based): the type, signature and docs of what is there —
    put the name you want to learn about in your code first (or find it in Bench's code), then
    hover it. This answers "what are this method's arguments" without reading sources.
  - `definition` (symbol name, e.g. `toggle_ide` or `HermesApp.toggle_ide`): its full source,
    for symbols in the projects themselves (not in libraries — hover those).
  - `references` (symbol name): every place it is used, with surrounding lines.
  - `diagnostics` (file): the file's current errors and warnings.
  - `rename_symbol`: rename across the project.
  The first call after a start can come back empty while rust-analyzer indexes (~1–2 min):
  wait and call again. If these tools are not in your tool list, reach them like other
  non-core tools: `tool_call` with `{"calls": [{"name": "mcp__rust_lsp__hover", "arguments":
  {"filePath": "...", "line": 12, "column": 9}}]}`.
- Don't read the `gpui-*` reference pages for a simple app: they are long, fill the context and
  their advice ("grep the crate sources") is what made earlier sessions fail. The `ui-*` catalog
  covers simple apps; open a `gpui-*` page only for a component the catalog doesn't have.
