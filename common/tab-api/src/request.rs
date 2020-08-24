use crate::{
    chunk::InputChunk,
    tab::{CreateTabMetadata, TabId},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Request {
    /// Subscribes to stdout/stderr on the given tab
    /// The WebSocket will produce a series of Chunk messages,
    /// The messages will have incrementing (but not sequential) indeces.
    /// The messages may begin with data from the scrollback buffer
    Subscribe(TabId),

    /// Deactivates the subscription for the given tab.
    Unsubscribe(TabId),

    /// Sends the stdin data to the given tab
    Input(TabId, InputChunk),

    /// Terminates the shell on the given tab
    CreateTab(CreateTabMetadata),

    /// Terminates the shell on the given tab
    CloseTab(TabId),

    /// Terminates the shell on the given tab, by name
    CloseNamedTab(String),
}
