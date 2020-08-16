use crate::{
    chunk::StdinChunk,
    tab::{CreateTabMetadata, TabId},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum Request {
    /// Authenticates with the daemon
    Auth(Vec<u8>),

    /// Subscribes to stdout/stderr on the given tab
    /// The WebSocket will produce a series of Chunk messages,
    /// The messages will have incrementing (but not sequential) indeces.
    /// The messages may begin with data from the scrollback buffer
    Subscribe(TabId),

    /// Deactivates the subscription for the given tab.
    Unsubscribe(TabId),

    /// Sends the stdin data to the given tab
    Stdin(TabId, StdinChunk),

    /// Terminates the shell on the given tab
    CreateTab(CreateTabMetadata),

    /// Terminates the shell on the given tab
    CloseTab(TabId),

    /// Lists all active tabs
    ListTabs,
}
