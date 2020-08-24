use tab_api::tab::{TabId, TabMetadata};

#[derive(Clone, Debug)]
pub struct TabTerminated(pub TabId);
