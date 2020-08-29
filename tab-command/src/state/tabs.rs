use std::collections::HashMap;
use tab_api::tab::{TabId, TabMetadata};

#[derive(Clone, Debug)]
pub struct TabsState {
    pub initialized: bool,
    pub tabs: HashMap<TabId, TabMetadata>,
}

impl Default for TabsState {
    fn default() -> Self {
        Self {
            initialized: false,
            tabs: HashMap::new(),
        }
    }
}
