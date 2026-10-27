//! Bench — one window for its own agent (bench-agent, `agent/`), its sessions, the user's servers
//! (terminal over SSH), their files (a mini IDE), everyday tools and tasks. Crate, binary and
//! bundle id (local.bench.app) are "bench"; its state lives in `~/.bench/ui` (copied over from
//! `~/Library/Application Support/bench` or `…/hermes-gpui` on the first start,
//! `connection::migrate_state`).

// The browser build (web/) leaves out what needs a real system (terminals, probes, the state
// folder); what only the desktop calls is unused there.
#![cfg_attr(target_family = "wasm", allow(dead_code, unused_imports))]

mod app;
mod attach;
mod audio;
mod bench;
mod connection;
mod demo;
mod diff;
mod env;
mod explorer;
mod gateway;
mod icons;
mod inserts;
mod lang;
mod media;
mod model;
#[cfg(target_os = "macos")]
mod notify;
#[cfg(not(target_os = "macos"))]
#[path = "notify_other.rs"]
mod notify;
mod pi_agent;
mod rest;
mod speed;
mod ssh;
mod sshconfig;
mod theme;
mod ui;
mod video;

use gpui_kit::*;

/// `bench --probe [local|<ssh-host>]`: connect without a window and print what the gateway has.
#[cfg(not(target_family = "wasm"))]
fn probe(target: &str) -> anyhow::Result<()> {
    let target = if target == "local" {
        model::Target::Local
    } else {
        model::Target::Ssh(target.into())
    };
    let (tx, rx) = async_channel::unbounded();
    let gw = gateway::Gateway::start(target, tx);
    let mut conn = None;
    while let Ok(evt) = rx.recv_blocking() {
        match evt {
            gateway::Evt::Connected(c) => conn = Some(c),
            gateway::Evt::Error(e) => anyhow::bail!(e),
            gateway::Evt::Profiles(p) => {
                println!(
                    "profiles: {:?}",
                    p.iter().map(|p| &p.name).collect::<Vec<_>>()
                );
                let want = std::env::var("PROBE_PROFILE").ok();
                let name =
                    want.unwrap_or_else(|| p.first().map(|p| p.name.clone()).unwrap_or_default());
                gw.send(gateway::Cmd::Sessions(name));
            }
            gateway::Evt::Created { session, runtime } => {
                println!("created {session} (runtime {runtime}); setting effort");
                gw.send(gateway::Cmd::SetEffort {
                    session: runtime.clone(),
                    effort: "low".into(),
                });
                std::thread::sleep(std::time::Duration::from_secs(3));
                gw.send(gateway::Cmd::Delete {
                    profile: std::env::var("PROBE_PROFILE").unwrap_or_default(),
                    session: session.clone(),
                    runtime: Some(runtime),
                });
                std::thread::sleep(std::time::Duration::from_secs(3));
                return Ok(());
            }
            gateway::Evt::Catalog(list) => {
                println!("catalog: {} commands", list.len());
                for c in list
                    .iter()
                    .filter(|c| ["/goal", "/help", "/new", "/approvals"].contains(&c.name.as_str()))
                {
                    println!("  {} [{}] {}", c.name, c.category, c.desc);
                }
                if std::env::var("PROBE_DISPATCH").is_err() {
                    return Ok(());
                }
            }
            gateway::Evt::Dispatched { output, send, .. } => {
                println!(
                    "dispatched: output={:?} send={:?}",
                    output.map(|o| o.chars().take(300).collect::<String>()),
                    send
                );
                return Ok(());
            }
            gateway::Evt::Sessions(p, rows) => {
                println!("sessions[{p}]: {}", rows.len());
                if std::env::var("PROBE_NEWSESSION").is_ok() {
                    gw.send(gateway::Cmd::Create {
                        profile: p.clone(),
                        cwd: None,
                        subject: String::new(),
                        project: String::new(),
                    });
                    continue;
                }
                if std::env::var("PROBE_CATALOG").is_ok() {
                    gw.send(gateway::Cmd::Catalog { profile: p.clone() });
                }
                if let Some(c) = &conn {
                    println!("skills[{p}]: {}", rest::skills(c, &p)?.len());
                    if let Ok(id) = std::env::var("PROBE_DELETE") {
                        rest::delete_session(c, &p, &id)?;
                        println!("deleted {id}");
                        return Ok(());
                    }
                    if let Ok(paths) = std::env::var("PROBE_GET") {
                        for path in paths.split(',') {
                            let v = rest::get_raw(c, &p, path)?;
                            let s = serde_json::to_string_pretty(&v)?;
                            println!("GET {path}:\n{}", s.chars().take(2500).collect::<String>());
                        }
                    }
                    if let Ok(q) = std::env::var("PROBE_SEARCH") {
                        for (id, s) in rest::search_sessions(c, &p, &q)? {
                            println!("  hit {id}: {}", s.chars().take(100).collect::<String>());
                        }
                    }
                    println!("mcp[{p}]: {} bytes of JSON", rest::mcp_json(c, &p)?.len());
                    match rest::cron_jobs(c, &p) {
                        Ok(jobs) => println!(
                            "cron[{p}]: {:?}",
                            jobs.iter()
                                .map(|j| (&j.id, &j.name, &j.expr, &j.schedule, &j.next_run))
                                .collect::<Vec<_>>()
                        ),
                        Err(e) => println!("cron[{p}]: {e:#}"),
                    }
                }
                let pick = std::env::var("PROBE_MATCH").unwrap_or_default();
                let Some(first) = rows.iter().find(|r| r.title.contains(&pick)) else {
                    if std::env::var("PROBE_CATALOG").is_ok() {
                        continue;
                    }
                    return Ok(());
                };
                gw.send(gateway::Cmd::Open {
                    profile: p,
                    session: first.id.clone(),
                });
            }
            gateway::Evt::History {
                session,
                runtime,
                open,
            } => {
                let thread = ::thread::Thread::open(open);
                let rows = thread.rows();
                println!(
                    "open: stored={session} runtime={runtime} rows={}",
                    rows.len()
                );
                // PROBE_SEND: one prompt into this session, printing the events Bench reads.
                if let Ok(text) = std::env::var("PROBE_SEND") {
                    gw.send(gateway::Cmd::Send {
                        profile: std::env::var("PROBE_PROFILE").unwrap_or_default(),
                        session: runtime.clone(),
                        text,
                        effort: None,
                        context: std::env::var("PROBE_CONTEXT").unwrap_or_default(),
                    });
                    while let Ok(evt) = rx.recv_blocking() {
                        match evt {
                            gateway::Evt::Delta(_, t) => print!("{t}"),
                            gateway::Evt::ToolStart { name, detail, .. } => {
                                println!("\n[tool] {name}: {detail}")
                            }
                            gateway::Evt::Thread(_, e) => println!("[thread] {e:?}"),
                            gateway::Evt::Usage(_, u) => {
                                println!("\n[usage] {}/{}", u.context_used, u.context_max)
                            }
                            gateway::Evt::Complete { text, .. } => {
                                println!("\n[complete] {} chars", text.len());
                                return Ok(());
                            }
                            gateway::Evt::Error(e) => anyhow::bail!(e),
                            _ => {}
                        }
                    }
                }
                if let Ok(cmd) = std::env::var("PROBE_DISPATCH") {
                    let (name, arg) = cmd.split_once(' ').unwrap_or((&cmd, ""));
                    gw.send(gateway::Cmd::Dispatch {
                        profile: std::env::var("PROBE_PROFILE").unwrap_or_default(),
                        session: runtime.clone(),
                        name: name.into(),
                        arg: arg.into(),
                    });
                    continue;
                }
                for r in rows.iter().rev().take(6) {
                    println!("  {r:?}");
                }
                return Ok(());
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    connection::migrate_state();
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--probe") {
        if let Err(e) = probe(args.get(2).map(String::as_str).unwrap_or("local")) {
            eprintln!("probe failed: {e:#}");
            std::process::exit(1);
        }
        return;
    }
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            theme::apply(theme::load(), None, cx);
            app::menu::install(cx);

            open_main_window(cx);
            cx.activate(true);
        });
}

/// The browser build (web/): always the demo — nothing to connect to there. gpui-kit's own
/// icons come over HTTP from `./assets/icons/` next to the page; the browser has no system
/// fonts, so Inter, Fira Mono and Noto Sans (OFL, assets/fonts/) are built in.
#[cfg(target_family = "wasm")]
fn main() {
    console_error_panic_hook::set_once();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets::new("."))
        .run(|cx| {
            gpui_kit::init(cx);
            let fonts = vec![
                std::borrow::Cow::Borrowed(
                    include_bytes!("../assets/fonts/InterVariable.ttf").as_slice(),
                ),
                std::borrow::Cow::Borrowed(
                    include_bytes!("../assets/fonts/FiraMono-Medium.ttf").as_slice(),
                ),
                // GPUI's default text style asks for the system font; Noto Sans is on its
                // fallback list, so whatever isn't styled by the theme still has a font.
                std::borrow::Cow::Borrowed(
                    include_bytes!("../assets/fonts/NotoSans-Regular.ttf").as_slice(),
                ),
            ];
            if let Err(e) = cx.text_system().add_fonts(fonts) {
                log_error(&format!("fonts: {e:#}"));
            }
            theme::apply(theme::load(), None, cx);
            open_main_window(cx);
        });
}

#[cfg(target_family = "wasm")]
fn log_error(text: &str) {
    eprintln!("{text}");
}

/// A Bench window: its own connection to the agent, profile and chat.
pub fn open_main_window(cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1240.), px(800.)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some("Bench".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(14.), px(14.))),
        }),
        ..Default::default()
    };
    if let Err(e) = gpui_kit::open_window(options, cx, |window, cx| {
        let app = cx.new(|cx| app::BenchApp::new(window, cx));
        // The `/` list answers ↑ ↓ Tab Enter. It has to be an interceptor: gpui matches key
        // bindings BEFORE any key listener (Window::dispatch_key_event runs the bindings, and
        // only after that the capture and bubble key listeners), and the composer is a text
        // input that binds ↑ ↓ to moving the caret — so a handler on the root or on the
        // composer's own element sees those arrows only after the caret already moved.
        let watch = app.downgrade();
        let keystrokes = cx.intercept_keystrokes(move |ev, window, cx| {
            let ks = &ev.keystroke;
            if ks.modifiers.modified() {
                return;
            }
            let used = match ks.key.as_str() {
                "up" | "down" | "tab" | "enter" => watch
                    .upgrade()
                    .map(|app| {
                        app.update(cx, |this, cx| this.slash_key(ks.key.as_str(), window, cx))
                    })
                    .unwrap_or(false),
                _ => false,
            };
            if used {
                cx.stop_propagation();
            }
        });
        // The interceptor lives as long as the app.
        app.update(cx, |this, _| this._slash_keys = Some(keystrokes));
        app
    }) {
        eprintln!("open window: {e:#}");
    }
}
