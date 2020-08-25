use super::tab::TabScrollback;

use tab_api::{
    chunk::{InputChunk, OutputChunk},
    tab::{CreateTabMetadata, TabId, TabMetadata},
};

#[derive(Debug, Clone)]
pub enum CliSend {
    Input(TabId, InputChunk),
    CreateTab(CreateTabMetadata),
    RequestScrollback(TabId),
    CloseTab(TabId),
    CloseNamedTab(String),
}

#[derive(Debug, Clone)]
pub enum CliRecv {
    TabStarted(TabMetadata),
    Scrollback(TabScrollback),
    TabStopped(TabId),
    Output(TabId, OutputChunk),
}

#[derive(Debug, Clone)]
pub struct CliShutdown {}
