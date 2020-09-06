use std::collections::HashMap;
use tab_api::tab::{TabId, TabMetadata};

type TabsMap = HashMap<TabId, TabMetadata>;

/// The currently running tabs
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TabsState {
    pub tabs: TabsMap,
}

impl TabsState {
    pub fn new(tabs: &TabsMap) -> Self {
        Self { tabs: tabs.clone() }
    }
}
