use crate::service::pty::scrollback::ScrollbackBuffer;

use std::sync::Arc;
use tab_api::{chunk::OutputChunk, tab::TabId};
use tokio::sync::Mutex;

/// The state of the pty connection, either None, or Assigned
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
    /// Whether the pty connection has a tab assigned
    pub fn is_assigned(&self) -> bool {
        match self {
            PtyState::Assigned(_) => true,
            PtyState::None => false,
        }
    }

    /// Whether the pty connection has the given tab assigned
    pub fn has_assigned(&self, match_id: TabId) -> bool {
        if let PtyState::Assigned(id) = self {
            *id == match_id
        } else {
            false
        }
    }

    /// Unwraps the state, returning a TabId if assigned, or panic if unassigned
    pub fn unwrap(&self) -> TabId {
        match self {
            PtyState::None => panic!("Unwrap called on a PtyState::None value"),
            PtyState::Assigned(id) => *id,
        }
    }
}

/// A wrapper around a scrollback buffer that can be cheaply cloned, and transmitted over broadcast channels.
/// Can produce a cloned copy of the scrollback contents.
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
