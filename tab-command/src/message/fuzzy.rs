use crate::state::{fuzzy::TabEntry, workspace::WorkspaceTab};

#[derive(Debug, Clone)]
pub struct FuzzyRecv {
    pub tabs: Vec<WorkspaceTab>,
}

#[derive(Debug, Clone)]
pub enum FuzzyEvent {
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    Insert(char),
    Delete,
    Enter,
}

#[derive(Debug, Clone)]
pub struct FuzzySelection(pub String);

#[derive(Debug, Clone)]
pub struct FuzzyShutdown;
