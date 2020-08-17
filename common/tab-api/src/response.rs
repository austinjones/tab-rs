use crate::{
    chunk::OutputChunk,
    tab::{TabId, TabMetadata},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    Output(TabId, OutputChunk),
    TabUpdate(TabMetadata),
    TabList(Vec<TabMetadata>),
    TabTerminated(TabId),
    Close,
}

impl Response {
    pub fn is_close(&self) -> bool {
        if let Response::Close = self {
            true
        } else {
            false
        }
    }
}
