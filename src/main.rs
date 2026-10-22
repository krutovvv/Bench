//! Hermes GPUI — a native client for Hermes Agent gateways (local or over SSH).

mod app;
mod audio;
mod connection;
mod diff;
mod gateway;
mod history;
mod icons;
mod model;
mod rest;
mod theme;
mod ui;

use gpui_kit::component::Theme;
use gpui_kit::*;

/// `hermes-gpui --probe [local|<ssh-host>]`: connect without a window and print what the gateway has.
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
            gateway::Evt::Sessions(p, rows) => {
                println!("sessions[{p}]: {}", rows.len());
                if let Some(c) = &conn {
                    println!("skills[{p}]: {}", rest::skills(c, &p)?.len());
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
                                .map(|j| (&j.name, &j.schedule, &j.next_run))
                                .collect::<Vec<_>>()
                        ),
                        Err(e) => println!("cron[{p}]: {e:#}"),
                    }
                }
                let pick = std::env::var("PROBE_MATCH").unwrap_or_default();
                let Some(first) = rows.iter().find(|r| r.title.contains(&pick)) else {
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
                turns,
                ..
            } => {
                println!(
                    "open: stored={session} runtime={runtime} turns={}",
                    turns.len()
                );
                for t in turns.iter().rev().take(3) {
                    let took = match (t.started, t.finished) {
                        (Some(a), Some(b)) => format!("{:.0}s", b - a),
                        _ => "?".into(),
                    };
                    let src: Vec<_> = t.sources.iter().take(3).collect();
                    println!(
                        "  turn: steps={} sources={src:?} took={took}",
                        t.steps.len()
                    );
                    for f in &t.files {
                        let (add, del) = f.diff.as_deref().map(diff::stats).unwrap_or((0, 0));
                        println!("    file: {} +{add} -{del}", f.path);
                    }
                    let detailed = t
                        .steps
                        .iter()
                        .filter(|s| matches!(s, model::Step::Tool { args, output, .. } if !args.is_empty() && !output.is_empty()))
                        .count();
                    println!("    tool steps with args+output: {detailed}");
                    if let (Some(c), Some(f)) = (&conn, t.files.first()) {
                        let diffs: Vec<String> = t
                            .files
                            .iter()
                            .filter(|x| x.path == f.path)
                            .filter_map(|x| x.diff.clone())
                            .collect();
                        match rest::read_text(
                            c,
                            &std::env::var("PROBE_PROFILE").unwrap_or_default(),
                            &f.path,
                        ) {
                            Ok(file) => println!(
                                "    before: {} (file {} bytes, truncated={})",
                                diff::before(&file.text, &diffs)
                                    .map(|b| format!("reconstructed, {} bytes", b.len()))
                                    .unwrap_or_else(|| "not recoverable".into()),
                                file.text.len(),
                                file.truncated
                            ),
                            Err(e) => println!("    read: {e:#}"),
                        }
                    }
                }
                return Ok(());
            }
            _ => {}
        }
    }
    Ok(())
}

fn main() {
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
            Theme::sync_system_appearance(None, cx);
            theme::apply(cx);

            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1240.), px(800.)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Hermes".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(14.), px(14.))),
                }),
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| app::HermesApp::new(window, cx))
            })
            .expect("open window");
            cx.activate(true);
        });
}
