use crate::service::pty::scrollback::ScrollbackBuffer;

use std::sync::Arc;
use tab_api::{chunk::OutputChunk, tab::TabId};
use tokio::sync::{Mutex, RwLock};

#[derive(Debug, Clone)]
pub enum PtyState {
    None,
    Assigned(TabId),
}

impl Default for PtyState {
    fn default() -> Self {
        Self::None
    }
}

impl PtyState {
    pub fn has_assigned(&self, match_id: TabId) -> bool {
        if let PtyState::Assigned(id) = self {
            *id == match_id
        } else {
            false
        }
    }

    pub fn unwrap(&self) -> TabId {
        match self {
            PtyState::None => panic!("Unwrap called on a PtyState::None value"),
            PtyState::Assigned(id) => *id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PtyScrollback {
    scrollback: Arc<Mutex<ScrollbackBuffer>>,
}

impl PtyScrollback {
    pub fn new(scrollback: Arc<Mutex<ScrollbackBuffer>>) -> Self {
        Self { scrollback }
    }

    pub async fn scrollback(&self) -> impl Iterator<Item = OutputChunk> {
        let scrollback = self.scrollback.lock().await.clone_queue();
        scrollback.into_iter()
    }
}
