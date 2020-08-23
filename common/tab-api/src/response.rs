use crate::{
    chunk::OutputChunk,
    tab::{TabId, TabMetadata},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    Init(InitResponse),
    Output(TabId, OutputChunk),
    TabUpdate(TabMetadata),
    TabList(Vec<TabMetadata>),
    TabTerminated(TabId),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InitResponse {
    pub tabs: HashMap<TabId, TabMetadata>,
}
