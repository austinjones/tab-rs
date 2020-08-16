use crate::{
    chunk::Chunk,
    tab::{TabId, TabMetadata},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    Chunk(TabId, Chunk),
    TabUpdate(TabMetadata),
    TabList(Vec<TabMetadata>),
}
