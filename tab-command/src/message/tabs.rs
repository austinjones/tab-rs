use tab_api::tab::{TabId, TabMetadata};

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum TabRecv {
    CreateTab(String),
    SelectNamedTab {
        name: String,
        env_tab: Option<TabId>,
    },
}

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
pub enum SelectTabRequest {
    Named {
        name: String,
        env_tab: Option<TabId>,
    },
}
