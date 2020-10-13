use std::collections::{HashMap, HashSet};
use tab_api::tab::{TabId, TabMetadata};

/// The client's view of the available tabs.
#[derive(Clone, Debug)]
pub struct ActiveTabsState {
    pub tabs: HashMap<TabId, TabMetadata>,
}

impl ActiveTabsState {
    pub fn into_name_set(&self) -> HashSet<String> {
        self.tabs.values().map(|tab| tab.name.clone()).collect()
    }

    pub fn find_name(&self, name: &str) -> Option<&TabMetadata> {
        self.tabs.values().find(|elem| elem.name == name)
    }

    pub fn contains_name(&self, name: &str) -> bool {
        self.find_name(name).is_some()
    }
}
