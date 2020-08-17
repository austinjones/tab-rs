pub struct ClientState {
    pub selected_tab: Option<usize>,
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
