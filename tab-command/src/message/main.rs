use tab_api::{
    chunk::{InputChunk, OutputChunk},
    tab::TabId,
};

#[derive(Debug)]
pub struct MainShutdown {}

#[derive(Debug, Clone)]
pub enum MainRecv {
    SelectTab(String),
    ListTabs,
    SelectInteractive,
    CloseTab(String),
    AutocompleteTab,
    AutocompleteCloseTab,
    GlobalShutdown,
}

#[derive(Debug)]
pub struct SendStdout(pub TabId, pub OutputChunk);

#[derive(Debug)]
pub struct SendStdin(pub TabId, pub InputChunk);
