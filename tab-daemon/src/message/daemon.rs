use tab_api::tab::{CreateTabMetadata, TabId};

#[derive(Debug, Clone)]
pub struct DaemonShutdown;

#[derive(Debug, Clone)]
pub struct CreateTab(pub CreateTabMetadata);

#[derive(Debug, Clone)]
pub struct CloseTab(pub TabId);
