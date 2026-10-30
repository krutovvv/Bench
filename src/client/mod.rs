//! Talking to the agent, like Zed's `client` crate: where it runs and how to reach it
//! (`connection`), installing it and bringing it up (`launch`) as its machine's service
//! (`service`), its JSON-RPC session (`rpc`), its REST API (`rest`), `bench --probe` (`probe`),
//! and what answers instead of it in the demo (`fake`).

pub mod connection;
pub mod fake;
pub mod launch;
#[cfg(not(target_family = "wasm"))]
pub mod probe;
pub mod rest;
pub mod rpc;
mod service;
