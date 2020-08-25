use super::tab::TabInput;
use crate::{service::pty::scrollback::ScrollbackBuffer, state::pty::PtyScrollback};
use std::sync::Arc;
use tab_api::{
    chunk::OutputChunk,
    tab::{TabId, TabMetadata},
};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct PtyShutdown {}

#[derive(Debug, Clone)]
pub enum PtyRecv {
    Init(TabMetadata),
    Scrollback,
    Input(TabInput),
    Terminate,
}

#[derive(Debug, Clone)]
pub enum PtySend {
    Started(TabMetadata),
    Output(OutputChunk),
    Scrollback(PtyScrollback),
    Stopped,
}
