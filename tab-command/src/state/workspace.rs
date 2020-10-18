use super::tabs::ActiveTabsState;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, collections::HashSet, path::PathBuf};
use tab_api::tab::normalize_name;
use typed_builder::TypedBuilder;

/// The client's view of the workspace configuration
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorkspaceState {
    pub tabs: Vec<WorkspaceTab>,
}

impl WorkspaceState {
    pub fn with_active_tabs(&self, active_tabs: &ActiveTabsState) -> WorkspaceState {
        let mut tabs = self.tabs.clone();

        let workspace_tabs = self.into_name_set();

        for (_id, metadata) in active_tabs.tabs.iter() {
            if workspace_tabs.contains(&metadata.name) {
                continue;
            }

            let tab = WorkspaceTab {
                name: metadata.name.clone(),
                doc: metadata.doc.clone(),
                directory: PathBuf::from(&metadata.dir),
                shell: None,
                env: None,
            };

            tabs.push(tab);
        }

        tabs.sort_by(|a, b| a.name.cmp(&b.name));
        tabs.dedup_by_key(|tab| tab.name.clone());

        WorkspaceState { tabs }
    }

    pub fn into_name_set(&self) -> HashSet<String> {
        self.tabs.iter().map(|tab| tab.name.clone()).collect()
    }
}

/// A user-configured workspace tab, which may or may not be running
#[derive(TypedBuilder, Debug, Clone, Eq, PartialEq)]
pub struct WorkspaceTab {
    pub name: String,
    #[builder(default, setter(strip_option))]
    pub doc: Option<String>,
    pub directory: PathBuf,
    #[builder(default, setter(strip_option))]
    pub shell: Option<String>,
    #[builder(default, setter(strip_option))]
    pub env: Option<HashMap<String, String>>, // pub command: Option<String>,
}

impl WorkspaceTab {
    pub fn new(name: &str, directory: PathBuf) -> Self {
        let name = normalize_name(name);

        Self {
            name,
            directory,
            shell: None,
            doc: None,
            env: None,
        }
    }

    pub fn with_options(name: &str, directory: PathBuf, options: TabOptions) -> Self {
        let name = normalize_name(name);

        Self {
            name,
            directory,
            shell: options.shell,
            doc: options.doc,
            env: options.env,
        }
    }
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
    #[serde(flatten)]
    pub options: TabOptions,
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
    #[serde(flatten)]
    pub tab_options: TabOptions,
    pub tabs: Option<Vec<Tab>>,
}

/// A tab within the workspace or repository configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tab {
    pub tab: String,
    pub dir: Option<String>,
    #[serde(flatten)]
    pub options: TabOptions,
    // pub command: Option<String>,
}

/// A tab within the workspace or repository configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabOptions {
    pub doc: Option<String>,
    pub shell: Option<String>,
    pub env: Option<HashMap<String, String>>,
}

impl Default for TabOptions {
    fn default() -> Self {
        Self {
            doc: None,
            shell: None,
            env: None,
        }
    }
}

impl TabOptions {
    /// Computes a new TabOptions struct, delegating properties to Other if not set in Self
    /// Doc is not inherited.
    pub fn or(self, other: Self) -> Self {
        let env = if let Some(mut env) = self.env {
            if let Some(other_env) = other.env {
                for (key, value) in other_env.into_iter() {
                    if !env.contains_key(&key) {
                        env.insert(key, value);
                    }
                }
            }

            Some(env)
        } else {
            other.env
        };

        Self {
            doc: self.doc,
            shell: self.shell.or(other.shell),
            env,
        }
    }
}
