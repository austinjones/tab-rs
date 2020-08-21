use tab_api::tab::TabId;

#[derive(Debug, Clone)]
pub struct DaemonShutdown;

#[derive(Debug, Clone)]
pub struct CreateTab(pub String);

#[derive(Debug, Clone)]
pub struct CloseTab(pub TabId);
