use super::tab::TabScrollback;

use tab_api::{
    chunk::{InputChunk, OutputChunk},
    tab::{CreateTabMetadata, TabId, TabMetadata},
};

#[derive(Debug, Clone)]
pub enum ConnectionSend {
    Input(TabId, InputChunk),
    CreateTab(CreateTabMetadata),
    RequestScrollback(TabId),
    CloseTab(TabId),
}

#[derive(Debug, Clone)]
pub enum ConnectionRecv {
    TabStarted(TabMetadata),
    Scrollback(TabScrollback),
    TabStopped(TabId),
    Output(TabId, OutputChunk),
}

#[derive(Debug, Clone)]
pub struct ConnectionShutdown {}
