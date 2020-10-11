use tab_api::tab::{TabId, TabMetadata};

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum TabsRecv {
    Init(HashMap<TabId, TabMetadata>),
    Update(TabMetadata),
    Terminated(TabId),
}

#[derive(Debug, Clone)]
pub struct TabShutdown {}

#[derive(Debug, Clone)]
pub struct RequestTabClose(pub TabId);

#[derive(Debug, Clone)]
pub enum CreateTabRequest {
    Named(String),
}

#[derive(Debug, Clone)]
pub struct FuzzyRecv {
    pub tabs: Vec<(String, String)>,
}
