//! A made-up session for screenshots and the web demo, as Zed's `FakeFs`: `BENCH_DEMO=1 bench`
//! runs the whole app against a fake agent (`agent.rs`, behind `client/fake.rs`), which answers from
//! invented threads and projects: nothing is connected, nothing of the person's is shown. The rest
//! of Bench does not know. What the hooks (`hooks.rs`) ask of the window, a picture's state, is
//! done from outside once the agent's answers are in (`script.rs`). The media live in
//! `assets/demo/`.

mod agent;
mod env;
mod hooks;
mod journal;
mod probes;
mod script;
mod threads;

use std::sync::Arc;

use gpui_kit::*;

use crate::app::BenchApp;

/// At start, before the first window: whether this run is the demo. Its state stays in a folder
/// of its own, its look is the person's (reading it changes nothing of theirs).
pub fn install() -> bool {
    let Some(hooks) = hooks::read() else {
        return false;
    };
    #[cfg(not(target_family = "wasm"))]
    {
        let look = crate::settings::appearance::load();
        crate::client::connection::use_state_dir(
            std::env::temp_dir().join(format!("bench-demo-{}", std::process::id())),
        );
        crate::settings::appearance::save(&look);
    }
    crate::client::fake::use_fake(Arc::new(agent::DemoAgent::new(hooks)));
    true
}

/// A window of the demo: what the hooks ask of it.
pub fn attach(app: &Entity<BenchApp>, window: &mut Window, cx: &mut App) {
    if let Some(hooks) = hooks::read() {
        script::run(hooks, app, window, cx);
    }
}
