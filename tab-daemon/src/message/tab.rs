use std::sync::Arc;
use tab_api::tab::TabId;

#[derive(Debug, Clone)]
pub struct TabInput {
    pub id: TabId,
    pub stdin: Arc<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct TabOutput {
    pub id: TabId,
    pub stdout: Arc<Vec<u8>>,
}
