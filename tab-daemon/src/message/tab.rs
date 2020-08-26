use crate::state::{assignment::Assignment, pty::PtyScrollback};
use std::sync::Arc;
use tab_api::{
    chunk::{InputChunk, OutputChunk},
    tab::{TabId, TabMetadata},
};

#[derive(Debug, Clone)]
pub struct TabInput {
    pub id: TabId,
    pub stdin: Arc<InputChunk>,
}

#[derive(Debug, Clone)]
pub struct TabOutput {
    pub id: TabId,
    pub stdout: Arc<OutputChunk>,
}

#[derive(Debug, Clone)]
pub enum TabRecv {
    Assign(Assignment<TabMetadata>),
    Scrollback(TabId),
    /// Resizes the tab to the given number of (cols, rows)
    Resize(TabId, (u16, u16)),
    // Retasks all clients from the first tab, to the second
    Retask(TabId, TabId),
    Input(TabInput),
    Terminate(TabId),
}

#[derive(Debug, Clone)]
pub struct TabScrollback {
    pub id: TabId,
    pub scrollback: PtyScrollback,
}

impl TabScrollback {
    pub async fn scrollback(&self) -> impl Iterator<Item = OutputChunk> {
        self.scrollback.scrollback().await
    }
}

#[derive(Debug, Clone)]
pub enum TabSend {
    Started(TabMetadata),
    Scrollback(TabScrollback),
    Retask(TabId, TabId),
    Output(TabOutput),
    Stopped(TabId),
}
