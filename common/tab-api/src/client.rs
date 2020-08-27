use crate::chunk::OutputChunk;
use crate::{
    chunk::InputChunk,
    tab::{CreateTabMetadata, TabId, TabMetadata},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Response {
    Init(InitResponse),
    Output(TabId, OutputChunk),
    TabUpdate(TabMetadata),
    Retask(TabId),
    TabList(Vec<TabMetadata>),
    TabTerminated(TabId),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InitResponse {
    pub tabs: HashMap<TabId, TabMetadata>,
}
