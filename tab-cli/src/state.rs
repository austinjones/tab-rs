use tab_api::tab::TabId;

pub struct ClientState {
    pub selected_tab: Option<TabId>,
    pub awaiting_tab: Option<String>,
}

impl Default for ClientState {
    fn default() -> Self {
        Self {
            selected_tab: None,
            awaiting_tab: None,
        }
    }
}
