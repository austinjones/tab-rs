use crate::state::fuzzy::{FuzzyMatchState, FuzzyQueryState};

#[derive(Debug, Clone)]
pub struct FuzzyRecv {
    pub tabs: Vec<(String, String)>,
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
pub struct FuzzyShutdown;

#[derive(Debug, Clone)]
pub enum FuzzyInterfaceRecv {
    Query(FuzzyQueryState),
    Matches(FuzzyMatchState),
}
