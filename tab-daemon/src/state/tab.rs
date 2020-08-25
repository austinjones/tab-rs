use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
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

#[derive(Debug, Clone)]
pub struct TabAssignment {
    metadata: TabMetadata,
    taken: Arc<AtomicBool>,
}

impl TabAssignment {
    pub fn new(metadata: TabMetadata) -> TabAssignment {
        Self {
            metadata,
            taken: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn take(&self) -> Option<TabMetadata> {
        let taken = self.taken.swap(false, Ordering::SeqCst);
        if !taken {
            Some(self.metadata.clone())
        } else {
            None
        }
    }
}
