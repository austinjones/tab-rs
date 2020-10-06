use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The client's view of the workspace configuration
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

/// A user-configured workspace tab, which may or may not be running
#[derive(Debug, Clone)]
pub struct WorkspaceTab {
    pub name: String,
    pub directory: PathBuf,
    pub doc: String,
    // pub command: Option<String>,
}

/// The top-level YAML configuration object, either a workspace root, or repository root
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Config {
    #[serde(rename = "workspace")]
    Workspace(Workspace),
    #[serde(rename = "repo")]
    Repo(Repo),
}

/// The workspace root configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub tab: Option<String>,
    pub doc: Option<String>,
    pub workspace: Vec<WorkspaceItem>,
}

/// An item within the workspace configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorkspaceItem {
    Workspace(WorkspaceLink),
    Repo(WorkspaceRepoLink),
    Tab(Tab),
}

/// A link to a child workspace, from the workspace root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceLink {
    pub workspace: String,
}

/// A link to a repository within the workspace, from the workspace root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRepoLink {
    pub repo: String,
}

/// The repository configuration root
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repo {
    pub repo: String,
    pub doc: Option<String>,
    pub tabs: Option<Vec<Tab>>,
}

/// A tab within the workspace or repository configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tab {
    pub tab: String,
    pub doc: Option<String>,
    pub dir: Option<String>,
    // pub command: Option<String>,
}
