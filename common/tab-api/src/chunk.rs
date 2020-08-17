use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OutputChunk {
    pub index: usize,
    pub data: Vec<u8>,
}

impl OutputChunk {
    pub fn len(&self) -> usize {
        self.data.len()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InputChunk {
    pub data: Vec<u8>,
}

impl InputChunk {
    pub fn len(&self) -> usize {
        self.data.len()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ChunkType {
    Stdout,
    Stderr,
}
