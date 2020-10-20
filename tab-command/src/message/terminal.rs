use std::sync::Arc;

use lifeline::barrier::Barrier;

use crate::state::{terminal::TerminalMode, workspace::WorkspaceTab};

#[derive(Debug, Clone)]
pub enum TerminalRecv {
    Mode(TerminalMode),
}

#[derive(Debug, Clone)]
pub enum TerminalSend {
    FuzzyRequest,
    FuzzySelection(String),
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
