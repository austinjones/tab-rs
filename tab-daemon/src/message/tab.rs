use crate::state::{pty::PtyScrollback, tab::TabAssignment};
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
    Assign(TabAssignment),
    Scrollback(TabId),
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
    Output(TabOutput),
    Stopped(TabId),
}
