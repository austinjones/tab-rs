use tab_api::tab::{CreateTabMetadata, TabId};

#[derive(Debug, Clone)]
pub enum TabManagerRecv {
    CreateTab(CreateTabMetadata),
    CloseNamedTab(String),
    CloseTab(TabId),
}

#[derive(Debug, Clone)]
pub enum TabManagerSend {
    TabTerminated(TabId),
}
