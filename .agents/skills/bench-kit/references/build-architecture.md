# Stack and architecture

## Crates

```toml
[package]
edition = "2024"

[dependencies]
# UI: gpui-kit 0.8 = gpui-pre 0.3.9 (Zed's GPUI) + gpui-component 0.8 (widgets).
# List tree-sitter grammars explicitly: the umbrella "tree-sitter-languages" feature
# conflicts on the `cc` version.
gpui-kit = { version = "0.8.0", features = ["tree-sitter", "tree-sitter-rust", "tree-sitter-python",
  "tree-sitter-javascript", "tree-sitter-typescript", "tree-sitter-tsx", "tree-sitter-html",
  "tree-sitter-css", "tree-sitter-bash", "tree-sitter-go", "tree-sitter-toml", "tree-sitter-yaml",
  "tree-sitter-markdown", "tree-sitter-lua"] }   # full list: ~/Projects/bench-kit/Cargo.toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"        # YAML 1.2: `off` stays a string
anyhow = "1"
async-channel = "2"       # worker thread -> UI events
ureq = { version = "2.12", features = ["json"] }   # blocking HTTP, run off the UI thread
tungstenite = "0.30"      # blocking WebSocket on a worker thread
uuid = { version = "1", features = ["v4"] }
similar = "2"             # text diffs (full-file diff for the Diff view)
# audio when needed: cpal (mic), hound (WAV), rodio (playback)

[profile.dev]
opt-level = 1
[profile.dev.package."*"]
opt-level = 2             # dependencies optimized: the dev build is usable, still fast to link
```

The gpui-kit sources are the documentation: `~/.cargo/registry/src/*/gpui-component-0.8.0/src`
(widgets) and `gpui-pre-0.3.9/src` (core). Grep them for `pub fn` before guessing an API.

## Module layout (reference app, ~12k lines)

```
src/main.rs          window factory, --probe headless mode, app entry
src/theme.rs         palettes, fonts, accents, Appearance prefs (palettes: assets/themes/)
src/icons.rs         Lucide icons embedded with a macro (assets/icons/*.svg)
src/model.rs         plain data: rows, turns, steps, usage, prompts…
src/connection.rs    how to reach the agent (local / SSH tunnel), state_dir(), git/ssh helpers
src/pi_agent.rs      installs and starts the agent (agent/) on this Mac and over SSH
src/gateway.rs       worker thread: WebSocket JSON-RPC, Cmd in / Evt out, pending-call table
src/rest.rs          blocking REST calls, one fn per endpoint
src/app.rs           BenchApp (the name is older than the agent): all state + event handling + actions (the "controller")
src/app/*.rs         feature slices as `impl BenchApp` in child modules (can use private items):
                     project_list.rs (projects: the sidebar's only unit), project_env.rs (a project's
                     environment block), remotes.rs (projects on a server's agent), projects.rs (git
                     repo + branch label, Open in Zed), inserts.rs (put in / patch / sync inserts),
                     subjects.rs (opening a thread, the `/` rows, `!command`), local_files.rs,
                     remote_files.rs, servers.rs, sites.rs (the browser), tasks.rs, tools.rs,
                     prompts.rs, providers.rs, slash.rs, marks.rs, asks.rs, attachments.rs, processes.rs
src/task_store.rs    tasks: the agent's bench.sqlite via its REST `/api/tasks` (its `tasks` tool sees the same list)
src/ui/mod.rs        Render for BenchApp: layout skeleton, header, shared helpers (block_head,
                     editor_height, BLOCK_HEAD_H / FIELD_H, bar_h, plural…)
src/ui/*.rs          one file per area: sidebar, project_list, chat, composer, statusbar, panels (settings),
                     providers; the bodies the inserts reuse (tasks, tools, servers, terminal, remote_files, sites)
src/ui/inserts/      the inserts registry: `InsertKind` + `KINDS` + `words()` in mod.rs; terminal,
                     dir (/files), files (/ftp), tasks, tools, browser (`build-insert.md`)
--- servers and terminals ---
src/sshconfig.rs     servers = ~/.ssh/config Host blocks (+ `# folder:`/`# path:`), safe writes
src/ssh.rs           the shared ssh connection (ControlMaster in ~/.ssh/cm) for terminal/files/agent
src/term.rs          PTY (portable-pty) + alacritty_terminal grid; keys, mouse reports, links
src/remote.rs        a server's file kept as a local cache copy: where it came from
src/lang.rs          highlighter by file name / `#!`
src/app/keys.rs      window-wide shortcuts (root capture_key_down)
scripts/bundle-macos.sh, assets/icons/*.svg, assets/themes/*.json, assets/logo.svg
```

Rule: views (`ui/`) only read state and call `BenchApp` methods; I/O lives in `rest.rs`,
`gateway.rs`, `connection.rs`; feature logic in `app/*.rs`. Child modules of `app` see its private
fields — no need to make everything `pub`.

## Threads and async

- **UI thread** owns `BenchApp` (`Entity<BenchApp>`). Never block it.
- **Long-lived connection** (WebSocket): a `std::thread` with `mpsc::Receiver<Cmd>` in and an
  `async_channel::Sender<Evt>` out. The UI drains events in a task:

```rust
let events = cx.spawn_in(window, async move |this, cx| {
    while let Ok(evt) = evt_rx.recv().await {
        if this.update_in(cx, |this, window, cx| this.on_event(evt, window, cx)).is_err() {
            break;
        }
    }
});
```

- **One-off blocking work** (REST, git, ssh): a helper that runs a closure on the background
  executor and hands the result back on the UI thread:

```rust
fn rest<T: Send + 'static>(&mut self, window: &mut Window, cx: &mut Context<Self>,
    job: impl FnOnce(&Connection, &str) -> anyhow::Result<T> + Send + 'static,
    done: impl FnOnce(&mut Self, anyhow::Result<T>, &mut Window, &mut Context<Self>) + 'static) {
    let Some(conn) = self.conn.clone() else { return };
    let profile = self.current_profile();
    cx.spawn_in(window, async move |this, cx| {
        let result = cx.background_spawn(async move { job(&conn, &profile) }).await;
        let _ = this.update_in(cx, |this, window, cx| done(this, result, window, cx));
    }).detach();
}
```

- **Tickers** (elapsed clock, live speed, waveform): `cx.spawn` loop with
  `cx.background_executor().timer(Duration::from_millis(500)).await`, `cx.notify()` only while
  something is live; break when `this.update` fails (window closed).
- **Debounce** (search as you type): bump a sequence number, wait 300 ms on a timer, act only if
  the number is still current.
- Keep every `Task` you don't `detach()` in a field (`_tasks: Vec<Task<()>>`) or it is dropped.

## Windows

Factor the window creation into one function and call it from `main` and from the app:

```rust
pub fn open_main_window(cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1240.), px(800.)), cx))),
        titlebar: Some(TitlebarOptions {
            title: Some("App".into()),
            appears_transparent: true,                       // content under the title bar
            traffic_light_position: Some(point(px(14.), px(14.))),
        }),
        ..Default::default()
    };
    let _ = gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| app::App::new(window, cx)));
}
```

Each window is a full app state (own connection, project, thread) — simpler than tabs and lets two
windows point at different backends. To open "this item in a new window", put the start
parameters in a `static NEXT_WINDOW: Mutex<Option<…>>` and let `new()` take it.

## Persistence

Small JSON files in `state_dir()` (Bench: `~/.bench/ui`, beside the agent's home; a template
app: `~/Library/Application Support/<app>/`): appearance, last
place (backend + session per backend), session colors/pins, per-backend caches, logs.
Merge with what is on disk before writing (another window may have written too). User content
that belongs to the backend (prompts, skills) goes to the backend's files, as Markdown, not here.

## Headless probe mode

`app --probe <target>` connects without a window and prints what the UI would show (lists, a
session, REST answers, a one-off GET via `PROBE_GET=/api/...`). Every protocol question was
answered there first — far cheaper and safer than clicking through the UI.
