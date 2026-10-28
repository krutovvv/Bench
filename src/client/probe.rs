//! `bench --probe [local|<ssh-host>]`: connect without a window and print what the agent has —
//! its threads, and with `PROBE_MATCH=<title words>` the last rows of that thread. Env:
//! `PROBE_SEND="prompt"` sends it there and prints the events until the run ends (with
//! `PROBE_CONTEXT`), `PROBE_COMMAND="/model"` runs a slash command, `PROBE_GET=/api/a,/api/b`
//! prints REST answers, `PROBE_SEARCH=words` searches the threads.

use anyhow::{Result, bail};
use protocol::{Event, ThreadEvent};

use crate::app::model::{GENERAL_AGENT, Target};
use crate::client::rest;
use crate::client::rpc::{Client, Incoming};

pub fn run(target: &str) -> Result<()> {
    let target = if target == "local" {
        Target::Local
    } else {
        Target::Ssh(target.into())
    };
    let (tx, rx) = async_channel::unbounded();
    let client = Client::start(target, tx);
    let conn = loop {
        match rx.recv_blocking()? {
            Incoming::Connected(c) => break c,
            Incoming::Lost(why) => bail!(why),
            Incoming::Event(_) => {}
        }
    };
    let list = client.request_blocking(&protocol::ListThreads {})?;
    println!("threads: {}", list.len());
    if let Ok(paths) = std::env::var("PROBE_GET") {
        for path in paths.split(',') {
            let v = rest::get_raw(&conn, GENERAL_AGENT, path)?;
            let s = serde_json::to_string_pretty(&v)?;
            println!("GET {path}:\n{}", s.chars().take(2500).collect::<String>());
        }
    }
    if let Ok(q) = std::env::var("PROBE_SEARCH") {
        for (id, s) in rest::search_threads(&conn, GENERAL_AGENT, &q)? {
            println!("  hit {id}: {}", s.chars().take(100).collect::<String>());
        }
    }
    let Ok(pick) = std::env::var("PROBE_MATCH") else {
        return Ok(());
    };
    let Some(first) = list.iter().find(|r| r.title.contains(&pick)) else {
        bail!("no thread titled like {pick:?}");
    };
    let thread = first.id.clone();
    let open = client.request_blocking(&protocol::OpenThread {
        thread: thread.clone(),
        since: 0,
    })?;
    let rows = ::thread::Thread::open(open).rows();
    println!("open {thread}: {} rows", rows.len());
    for r in rows.iter().rev().take(6) {
        println!("  {r:?}");
    }
    if let Ok(command) = std::env::var("PROBE_COMMAND") {
        let r = client.request_blocking(&protocol::RunCommand {
            thread: thread.clone(),
            command,
        })?;
        println!("command: {r:?}");
    }
    if let Ok(text) = std::env::var("PROBE_SEND") {
        client.request_blocking(&protocol::SendMessage {
            thread: thread.clone(),
            text,
            context: std::env::var("PROBE_CONTEXT").unwrap_or_default(),
            request: None,
            queue: false,
            effort: None,
        })?;
        while let Ok(incoming) = rx.recv_blocking() {
            let Incoming::Event(Event::Thread { thread: t, event }) = incoming else {
                continue;
            };
            if t != thread {
                continue;
            }
            match event {
                ThreadEvent::Stream(_, delta) => print!("{delta}"),
                ThreadEvent::Run(false) => {
                    println!("\n[run ended]");
                    break;
                }
                e => println!("\n[{e:?}]"),
            }
        }
    }
    Ok(())
}
