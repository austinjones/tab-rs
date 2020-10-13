use crate::state::{terminal::TerminalMode, workspace::WorkspaceTab};

#[derive(Debug, Clone)]
pub enum TerminalRecv {
    FuzzyTabs(Vec<WorkspaceTab>),
    Mode(TerminalMode),
}

#[derive(Debug, Clone)]
pub enum TerminalSend {
    FuzzySelection(String),
}

impl TerminalSend {
    pub fn fuzzy_selection(&self) -> Option<String> {
        match self {
            TerminalSend::FuzzySelection(name) => Some(name.clone()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TerminalInput {
    Stdin(Vec<u8>),
    Resize((u16, u16)),
}

#[derive(Debug, Clone)]
pub enum TerminalOutput {
    Stdout(Vec<u8>),
}

#[derive(Debug, Clone)]
pub struct TerminalShutdown {}
