use tab_api::tab::{TabId, TabMetadata};

#[derive(Clone, Debug)]
pub enum TabStateSelect {
    None,
    Selected(String),
}

impl Default for TabStateSelect {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TabState {
    None,
    Awaiting(String),
    Selected(TabId, String),
}

impl TabState {
    pub fn is_awaiting(&self, target_name: &str) -> bool {
        match self {
            TabState::None => false,
            TabState::Awaiting(name) => name.as_str() == target_name,
            TabState::Selected(_, _) => false,
        }
    }

    pub fn is_selected(&self, target_id: &TabId) -> bool {
        match self {
            TabState::None => false,
            TabState::Awaiting(_) => false,
            TabState::Selected(id, _name) => id == target_id,
        }
    }

    pub fn is_selected_name(&self, target: &str) -> bool {
        match self {
            TabState::None => false,
            TabState::Awaiting(_) => false,
            TabState::Selected(_id, name) => target == name.as_str(),
        }
    }
}

impl Default for TabState {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Clone, Debug)]
pub struct TabStateAvailable(pub Vec<TabMetadata>);

impl Default for TabStateAvailable {
    fn default() -> Self {
        TabStateAvailable(vec![])
    }
}
