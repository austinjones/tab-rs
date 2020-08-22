use std::collections::{HashMap};
use tab_api::tab::{TabId, TabMetadata};

type TabsMap = HashMap<TabId, TabMetadata>;
#[derive(Debug, Clone, Default)]
pub struct TabsState {
    pub tabs: TabsMap,
}

impl TabsState {
    pub fn new(tabs: &TabsMap) -> Self {
        Self { tabs: tabs.clone() }
    }
}
