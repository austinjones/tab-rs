use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Chunk {
    pub channel: ChunkType,
    pub data: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub enum ChunkType {
    Stdout,
    Stderr,
}
