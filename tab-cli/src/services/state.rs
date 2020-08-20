use super::tab_state::TabStateSelect;
use tab_api::tab::{TabId, TabMetadata};
use tab_service::{service_bus, Message};
use tokio::sync::{broadcast, watch};

service_bus!(pub StateBus);

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
            TabState::Selected(id, name) => id == target_id,
        }
    }

    pub fn is_selected_name(&self, target: &str) -> bool {
        match self {
            TabState::None => false,
            TabState::Awaiting(_) => false,
            TabState::Selected(id, name) => target == name.as_str(),
        }
    }
}

impl Default for TabState {
    fn default() -> Self {
        Self::None
    }
}

impl Message<StateBus> for TabState {
    type Channel = watch::Sender<Self>;
}

impl Message<StateBus> for TabStateSelect {
    type Channel = watch::Sender<Self>;
}

impl Message<StateBus> for TabMetadata {
    type Channel = broadcast::Sender<Self>;
}

#[derive(Clone, Debug)]
pub struct TerminalSizeState(pub (u16, u16));

impl Default for TerminalSizeState {
    fn default() -> Self {
        let dimensions = crossterm::terminal::size().expect("failed to get terminal size");

        TerminalSizeState(dimensions)
    }
}

impl Message<StateBus> for TerminalSizeState {
    type Channel = watch::Sender<Self>;
}

#[derive(Clone, Debug)]
pub struct TabStateAvailable(pub Vec<TabMetadata>);

impl Default for TabStateAvailable {
    fn default() -> Self {
        TabStateAvailable(vec![])
    }
}

impl Message<StateBus> for TabStateAvailable {
    type Channel = watch::Sender<Self>;
}
