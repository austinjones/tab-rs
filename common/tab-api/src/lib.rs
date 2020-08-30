//! Shared API, exported to `tab-cli`, `tab-daemon`, and `tab-pty`
//!
//! All inter-process communication is described in this crate.

pub mod chunk;
pub mod client;
pub mod config;
pub mod env;
pub mod launch;
pub mod pty;
pub mod tab;
