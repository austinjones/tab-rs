use tab_api::{
    chunk::{InputChunk, OutputChunk},
    tab::TabId,
};

#[derive(Debug)]
pub struct MainShutdown {}

#[derive(Debug, Clone)]
pub enum MainRecv {
    AutocompleteCloseTab,
    AutocompleteTab,
    CheckWorkspace,
    CloseTabs(Vec<String>),
    DisconnectTabs(Vec<String>),
    GlobalShutdown,
    ListTabs,
    SelectInteractive,
    SelectTab(String),
}

#[derive(Debug)]
pub struct SendStdout(pub TabId, pub OutputChunk);

#[derive(Debug)]
pub struct SendStdin(pub TabId, pub InputChunk);
