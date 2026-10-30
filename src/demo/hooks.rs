//! The demo's hooks: what `BENCH_DEMO…` and its neighbours ask for, read once at start. The table
//! of them is `.agents/skills/bench-kit/references/build-verification.md`.

use std::sync::OnceLock;

use protocol::ThreadId;

macro_rules! hooks {
    ($($(#[$doc:meta])* $field:ident = $var:literal),* $(,)?) => {
        /// Each hook's value, as its variable says it; `None` when not set.
        pub struct DemoHooks {
            $($(#[$doc])* pub $field: Option<String>,)*
        }

        impl DemoHooks {
            fn from_env() -> Self {
                Self { $($field: std::env::var($var).ok(),)* }
            }
        }
    };
}

hooks! {
    /// What the window opens on: 1 (the browser thread), landing, top, long, server, tool, new,
    /// new-project, explorer, env.
    mode = "BENCH_DEMO",
    /// In the Explorer: settings, part, lsp, lsp-new, diff, terminal, problems, thread, specs, comment.
    explorer = "BENCH_EXPLORER",
    /// Its settings page scrolled: telegram, model.
    env_scroll = "BENCH_ENV",
    /// threads: the tree's place given to the project's threads.
    side = "BENCH_DEMO_SIDE",
    /// The tools insert's pane: html, preview, long.
    tool = "BENCH_DEMO_TOOL",
    /// A saved `thread.open` answer drawn in the landing's place.
    journal = "BENCH_DEMO_JOURNAL",
    /// <n>[:<i>]: the n-th work open, its i-th call's card open.
    work = "BENCH_DEMO_WORK",
    /// <c>: of the works with c calls.
    work_calls = "BENCH_DEMO_WORK_CALLS",
    /// <px>: that much further down into the opened work.
    work_down = "BENCH_DEMO_WORK_DOWN",
    /// 0: the milestone open, not its card.
    card = "BENCH_DEMO_CARD",
    /// end, or <item>:<px>: the thread scrolled there.
    scroll = "BENCH_DEMO_SCROLL",
    /// 1: the changed files unfolded; diff: the first one's diff under its row.
    files = "BENCH_DEMO_FILES",
    /// The background processes' card unfolded with its output.
    procs = "BENCH_DEMO_PROCS",
    /// The turn reviewed, its notes unfolded.
    review = "BENCH_DEMO_REVIEW",
    /// <key>: that delete waiting for its second press (thread:d-api, p:p-web).
    arm = "BENCH_DEMO_ARM",
    /// "Jump to latest" pressed two seconds in.
    jump = "BENCH_DEMO_JUMP",
    /// 1, env, long: the new project's page, with an exported environment or a long folder.
    project = "BENCH_DEMO_PROJECT",
    /// The dock panel's "+" pressed two seconds in.
    new_in_panel = "BENCH_DEMO_NEW",
    /// 1, or words: typed into the composer two seconds in.
    typed = "BENCH_DEMO_TYPE",
    /// Send pressed three seconds in.
    send = "BENCH_DEMO_SEND",
    /// A run plays in the open thread.
    run = "BENCH_DEMO_RUN",
    /// The run holds mid-work.
    paused = "BENCH_DEMO_PAUSED",
    /// 1 or answered: the agent's question over the thread.
    clarify = "BENCH_CLARIFY",
    /// It takes several answers.
    clarify_multi = "BENCH_CLARIFY_MULTI",
    /// No main model set, as a fresh install.
    no_model = "BENCH_DEMO_NOMODEL",
    /// providers: the gear open.
    panel = "BENCH_PANEL",
    /// <0|1|2>: on that tab.
    gear_tab = "BENCH_GEAR_TAB",
    /// Two messages and a plan waiting.
    queue = "BENCH_DEMO_QUEUE",
    /// These words in the composer.
    draft = "BENCH_DEMO_DRAFT",
    /// These words in the composer, from the first frame.
    prefill = "BENCH_PREFILL",
    /// <thread id>: that thread beside the open one.
    beside = "BENCH_DEMO_BESIDE",
    /// 1, or words: ⌘K open, with those words typed.
    search = "BENCH_DEMO_SEARCH",
    /// <thread id>: that thread being renamed.
    rename = "BENCH_DEMO_RENAME",
    /// <group>:above|below: the sidebar's drop line over a project's group.
    drop_line = "BENCH_DEMO_DROPLINE",
    /// agent: its popover open, with long lines in its log.
    pop = "BENCH_DEMO_POP",
    /// chat, between, right, foot, side, tint: a file carried into the window there.
    drop = "BENCH_DEMO_DROP",
    /// <x>,<y>: the left button pressed and let go there.
    click = "BENCH_DEMO_CLICK",
    /// The composer's hover tools shown at rest.
    tools_shown = "BENCH_TOOLS_SHOWN",
}

static HOOKS: OnceLock<DemoHooks> = OnceLock::new();

/// The hooks when this run is the demo: `BENCH_DEMO` is set, or the browser build, which has
/// nothing to connect to.
pub fn read() -> Option<&'static DemoHooks> {
    let hooks = HOOKS.get_or_init(DemoHooks::from_env);
    (hooks.mode.is_some() || cfg!(target_family = "wasm")).then_some(hooks)
}

impl DemoHooks {
    pub fn mode(&self) -> &str {
        self.mode.as_deref().unwrap_or_default()
    }

    /// The thread the window opens on; None: a new one.
    pub fn opens(&self) -> Option<ThreadId> {
        match self.mode() {
            "server" => Some("d-web1".into()),
            "tool" => Some("d-tool".into()),
            "landing" | "top" | "long" => Some("d-landing".into()),
            "new" => None,
            _ => Some("d-site".into()),
        }
    }
}
