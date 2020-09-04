//! Requests and Responses, communicated between `tab-cli` and `tab-daemon`.

use crate::chunk::OutputChunk;
use crate::{
    chunk::InputChunk,
    tab::{CreateTabMetadata, TabId, TabMetadata},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
/// A request, sent from a CLI connection to the daemon process.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Subscribes to stdout/stderr on the given tab
    /// The WebSocket will produce a series of Chunk messages,
    /// The messages will have incrementing (but not sequential) indices.
    /// The messages may begin with data from the scrollback buffer
    Subscribe(TabId),

    /// Deactivates the subscription for the given tab.
    Unsubscribe(TabId),

    /// Sends the stdin data to the given tab
    Input(TabId, InputChunk),

    /// Terminates the shell on the given tab
    CreateTab(CreateTabMetadata),

    /// Resizes the given tab, to the provided (cols, rows)
    ResizeTab(TabId, (u16, u16)),

    /// Re-tasks clients with the tabid selected to the given tab
    Retask(TabId, TabId),

    /// Terminates the shell on the given tab
    CloseTab(TabId),

    /// Terminates the shell on the given tab, by name
    CloseNamedTab(String),

    /// Shuts down all tab processes, including the daemon and all ptys
    GlobalShutdown,
}

/// A response, sent from the daemon process to a connected CLI
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Response {
    /// An initial 'hello' message with introductory state, including a full list of running tabs.
    Init(InitResponse),
    /// A raw output chunk, identified by a `TabId` and an index.
    Output(TabId, OutputChunk),
    /// A notification that metadata about a running tab has changed.
    TabUpdate(TabMetadata),
    /// A notification that the client is being re-tasks, and will now be serving the user on another tab.
    Retask(TabId),
    /// A notification that the tab has been terminated
    TabTerminated(TabId),
}

/// An initialization message sent to CLI connections.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct InitResponse {
    /// A complete set of active tabs, identified by TabId values.
    pub tabs: HashMap<TabId, TabMetadata>,
}
