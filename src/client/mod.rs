//! Talking to the agent, like Zed's `client` crate: where it runs and how to reach it
//! (`connection`), installing and starting it (`launch`), its JSON-RPC session (`rpc`) and its
//! REST API (`rest`).

pub mod connection;
pub mod launch;
pub mod rest;
pub mod rpc;
