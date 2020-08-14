use crate::{chunk::Chunk, tab::TabId};

pub enum Response {
    Chunk(Chunk),
    TabList(Vec<TabId>),
}
