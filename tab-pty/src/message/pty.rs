use lifeline::impl_storage_clone;
use std::{collections::HashMap, path::PathBuf, process::ExitStatus};
use tab_api::chunk::{InputChunk, OutputChunk};

#[derive(Debug, Clone)]
pub struct MainShutdown {}

#[derive(Debug, Clone)]
pub struct PtyShutdown {}

#[derive(Debug, Clone)]
pub enum PtyRequest {
    Resize((u16, u16)),
    Input(InputChunk),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum PtyResponse {
    Output(OutputChunk),
    Terminated(ExitStatus),
}

#[derive(Debug, Clone)]
pub struct PtyOptions {
    pub dimensions: (u16, u16),
    pub command: String,
    pub args: Vec<String>,
    pub working_directory: PathBuf,
    pub env: HashMap<String, String>,
}

impl_storage_clone!(PtyOptions);
