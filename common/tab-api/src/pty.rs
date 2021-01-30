//! PtyRequests and PtyResponses, communicated between `tab-pty` and `tab-daemon`.

use crate::{
    chunk::{InputChunk, OutputChunk},
    tab::TabMetadata,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PtyWebsocketResponse {
    Started(TabMetadata),
    Output(OutputChunk),
    Resized((u16, u16)),
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PtyWebsocketRequest {
    Init(TabMetadata),
    Input(InputChunk),
    Resize((u16, u16)),
    Terminate,
}
