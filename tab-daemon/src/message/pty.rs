use super::tab::TabInput;
use crate::state::pty::PtyScrollback;

use tab_api::{chunk::OutputChunk, tab::TabMetadata};

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
