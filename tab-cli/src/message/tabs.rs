use tab_api::tab::{TabId, TabMetadata};

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum TabsRecv {
    Init(HashMap<TabId, TabMetadata>),
    Update(TabMetadata),
    Terminated(TabId),
}
