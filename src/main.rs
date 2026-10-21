//! Hermes GPUI — a native client for Hermes Agent gateways (local or over SSH).

mod app;
mod audio;
mod connection;
mod gateway;
mod history;
mod model;
mod rest;
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
                gw.send(gateway::Cmd::Sessions(
                    p.first().map(|p| p.name.clone()).unwrap_or_default(),
                ));
            }
            gateway::Evt::Sessions(p, rows) => {
                println!("sessions[{p}]: {}", rows.len());
                if let Some(c) = &conn {
                    println!("skills[{p}]: {}", rest::skills(c, &p)?.len());
                    println!("mcp[{p}]: {} bytes of JSON", rest::mcp_json(c, &p)?.len());
                }
                let Some(first) = rows.first() else {
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
            // Zed-like: square, flat, dense.
            let theme = Theme::global_mut(cx);
            theme.radius = px(0.);
            theme.radius_lg = px(0.);
            theme.shadow = false;
            theme.font_size = px(14.);
            theme.mono_font_size = px(13.);

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
