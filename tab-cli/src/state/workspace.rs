use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum WorkspaceState {
    Loading,
    Ready(Vec<WorkspaceTab>),
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self::Loading
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceTab {
    pub name: String,
    pub directory: PathBuf,
    pub doc: String,
    // pub command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Config {
    #[serde(rename = "workspace")]
    Workspace(Workspace),
    #[serde(rename = "repo")]
    Repo(Repo),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub workspace: Vec<WorkspaceItem>,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorkspaceItem {
    Repo(WorkspaceRepoLink),
    Tab(Tab),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRepoLink {
    pub repo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repo {
    pub repo: String,
    pub doc: Option<String>,
    pub tabs: Vec<Tab>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tab {
    pub tab: String,
    pub doc: Option<String>,
    pub dir: Option<String>,
    // pub command: Option<String>,
}
