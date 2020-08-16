use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Chunk {
    pub index: usize,
    pub channel: ChunkType,
    pub data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StdinChunk {
    pub data: Vec<u8>,
}

impl Chunk {
    pub fn len(&self) -> usize {
        self.data.len()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ChunkType {
    Stdout,
    Stderr,
}
