use tab_api::tab::{TabId, TabMetadata};

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum TabRecv {
    SelectNamedTab {
        name: String,
        env_tab: Option<TabId>,
    },
    DeselectTab,
    ScanWorkspace,
}

#[derive(Debug, Clone)]
pub enum TabsRecv {
    Init(HashMap<TabId, TabMetadata>),
    Update(TabMetadata),
}

#[derive(Debug, Clone)]
pub struct ScanWorkspace {}

#[derive(Debug, Clone)]
pub struct TabShutdown {}

#[derive(Debug, Clone)]
pub struct RequestTabClose(pub TabId);

#[derive(Debug, Clone)]
pub enum CreateTabRequest {
    Named(String),
}
