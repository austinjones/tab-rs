use crate::state::{assignment::Assignment, pty::PtyScrollback};
use std::sync::Arc;
use tab_api::{
    chunk::{InputChunk, OutputChunk},
    client::RetaskTarget,
    tab::{TabId, TabMetadata},
};

/// An input (stdin) event for tab, identified by an id.
/// Cheaply clonable and sent over broadcast channels.
///
/// Messaged on the CliBus, and forwarded to active PTYs.
#[derive(Debug, Clone, Eq)]
pub struct TabInput {
    pub id: TabId,
    pub stdin: Arc<InputChunk>,
}

impl PartialEq for TabInput {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && *self.stdin == *other.stdin
    }
}

impl TabInput {
    #[cfg(test)]
    pub fn new(id: TabId, data: Vec<u8>) -> Self {
        Self {
            id,
            stdin: Arc::new(InputChunk { data }),
        }
    }
}

/// An output (stdout) event for tab, identified by an id
/// Cheaply clonable and sent over broadcast channels.
///
/// Messaged on the PtyBus, and forwarded to subscribed CLIs via `TabSend`.
#[derive(Debug, Clone)]
pub struct TabOutput {
    pub id: TabId,
    pub stdout: Arc<OutputChunk>,
}

impl PartialEq for TabOutput {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && *self.stdout == *other.stdout
    }
}

impl Eq for TabOutput {
    fn assert_receiver_is_total_eq(&self) {}
}

/// A message transmitted to a tab, used as a broadcast adapter between CLI and PTY connections.
///
/// Carried over the ListenerBus.
///
/// Usage:
/// - Tx from the `ListenerConnectionCarrier`, to forward lifecycle & stdin from CLI connections
/// - Tx from the `TabManagerService`, to offer tab assignments to PTY connections
/// - Rx from the `ListenerPtyCarrier`, to forward events to an established PTY tab.
/// - Rx from the `RetaskService`, to broadcast retask to subscribed CLI connections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabRecv {
    Assign(Assignment<TabMetadata>),
    Scrollback(TabId),
    /// Resizes the tab to the given number of (cols, rows)
    Resize(TabId, (u16, u16)),
    /// Retasks all clients from the first tab, to the second
    /// If the second argument is None, then clients should disconnect
    Retask(TabId, RetaskTarget),
    Input(TabInput),
    Terminate(TabId),
    TerminateAll,
}

/// A cheaply clonable message with the latest tab scrollback.
/// Receivers can call `msg.scrollback.scrollback()` to clone & iterate over the scrollback buffer.
#[derive(Debug, Clone)]
pub struct TabScrollback {
    pub id: TabId,
    pub scrollback: PtyScrollback,
}

impl Eq for TabScrollback {
    fn assert_receiver_is_total_eq(&self) {}
}

impl PartialEq for TabScrollback {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl TabScrollback {
    #[cfg(test)]
    pub fn empty(id: TabId) -> Self {
        Self {
            id,
            scrollback: PtyScrollback::empty(),
        }
    }

    #[cfg(test)]
    pub async fn push(&self, chunk: OutputChunk) {
        self.scrollback.push(chunk).await;
    }

    pub async fn scrollback(&self) -> impl Iterator<Item = OutputChunk> {
        self.scrollback.scrollback().await
    }
}

/// A message sent from an established tab, to provide lifecycle notification events,
/// and scrollback/stdout.  Also provides retask notifications if the `tab-cli` is invoked from within a tab.
///
/// Carried over the ListenerBus.
///
/// Usage:
/// - Tx from the `ListenerPtyCarrier` to forward raw PTY events.
/// - Tx from the `RetaskService`, to provide retask notifications to subscribed CLI connections
/// - Rx from the `ListenerConnectionCarrier`, to forward notifications to subscribed CLI connections
#[derive(Debug, Clone)]
pub enum TabSend {
    Started(TabMetadata),
    Updated(TabMetadata),
    Scrollback(TabScrollback),
    /// Instructs clients on the first tab to retask to the second
    /// If the second argument is None, clients should disconnect
    Retask(TabId, RetaskTarget),
    Output(TabOutput),
    Stopped(TabId),
}
