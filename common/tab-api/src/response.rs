use crate::{
    chunk::OutputChunk,
    tab::{TabId, TabMetadata},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    Unauthorized,
    Output(TabId, OutputChunk),
    TabUpdate(TabMetadata),
    TabList(Vec<TabMetadata>),
    TabTerminated(TabId),
}
