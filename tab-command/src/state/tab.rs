use tab_api::tab::{TabId, TabMetadata};

/// The select tab action, either by name or id
#[derive(Debug, Clone, PartialEq)]
pub enum SelectTab {
    NamedTab(String),
    Tab(TabId),
}

/// The client's selected tab state.
#[derive(Debug, Clone, PartialEq)]
pub enum TabState {
    None,
    Awaiting(String),
    AwaitingId(TabId),
    Selected(TabMetadata),
}

impl TabState {
    pub fn is_awaiting(&self, target_name: &str) -> bool {
        match self {
            TabState::Awaiting(name) => name.as_str() == target_name,
            _ => false,
        }
    }

    pub fn is_awaiting_id(&self, target: TabId) -> bool {
        match self {
            TabState::AwaitingId(id) => *id == target,
            _ => false,
        }
    }

    pub fn is_selected(&self, target_id: TabId) -> bool {
        match self {
            TabState::Selected(ref metadata) => metadata.id == target_id,
            _ => false,
        }
    }
}

impl Default for TabState {
    fn default() -> Self {
        Self::None
    }
}
