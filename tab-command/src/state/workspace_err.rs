use std::path::PathBuf;
use thiserror::Error;

use super::workspace::WorkspaceTab;

pub type WorkspaceResult = std::result::Result<WorkspaceTab, WorkspaceError>;

#[derive(Error, Debug)]
pub enum WorkspaceError {
    #[error("{0}")]
    ConfigVariantError(ConfigVariantError),
    #[error("{0}")]
    NoConfigVariant(NoConfigVariantError),

    #[error("{0}")]
    WorkspaceNotFound(WorkspaceNotFoundError),
    #[error("{0}")]
    WorkspaceTabNameError(WorkspaceTabNameError),

    #[error("{0}")]
    RepoNotFound(RepoNotFoundError),
    #[error("{0}")]
    RepoTabNameError(RepoTabNameError),

    #[error("{0}")]
    CanonicalizeError(CanonicalizeError),
    #[error("{0}")]
    LoadYamlError(LoadYamlError),

    #[error("{0}")]
    TabDirectoryNotFound(TabDirectoryNotFoundError),

    #[error("{0}")]
    TabNameInvalid(TabNameInvalidError),
    #[error("{0}")]
    TabDuplicate(TabDuplicateError),
}

impl WorkspaceError {
    pub fn canonicalize_error(path: PathBuf, err: std::io::Error) -> Self {
        Self::CanonicalizeError(CanonicalizeError { path, err })
    }

    pub fn none_error(path: PathBuf, expected: &'static str) -> Self {
        Self::NoConfigVariant(NoConfigVariantError { path, expected })
    }

    pub fn load_error(err: LoadYamlError) -> Self {
        Self::LoadYamlError(err)
    }

    pub fn workspace_not_found(path: PathBuf) -> Self {
        Self::WorkspaceNotFound(WorkspaceNotFoundError { path })
    }

    pub fn workspace_tab_name_error(path: PathBuf) -> Self {
        Self::WorkspaceTabNameError(WorkspaceTabNameError { path })
    }

    pub fn repo_not_found(path: PathBuf) -> Self {
        Self::RepoNotFound(RepoNotFoundError { path })
    }

    pub fn repo_tab_name_error(path: PathBuf) -> Self {
        Self::RepoTabNameError(RepoTabNameError { path })
    }

    pub fn tab_directory_not_found(tab: String, path: PathBuf) -> Self {
        Self::TabDirectoryNotFound(TabDirectoryNotFoundError { tab, path })
    }

    pub fn tab_name_invalid(tab: String, reason: String) -> Self {
        Self::TabNameInvalid(TabNameInvalidError { tab, reason })
    }

    pub fn duplicate_tab(tab: String) -> Self {
        Self::TabDuplicate(TabDuplicateError { tab })
    }
}

#[derive(Error, Debug)]
#[error("Could not canonicalize path: {path}, error: {err}")]
pub struct CanonicalizeError {
    path: PathBuf,
    err: std::io::Error,
}

#[derive(Error, Debug)]
#[error("Workspace path does not exist: {path}")]
pub struct WorkspaceNotFoundError {
    pub path: PathBuf,
}

#[derive(Error, Debug)]
#[error("Workspace config has no 'tab' property, and no directory name could be resolved: {path}")]
pub struct WorkspaceTabNameError {
    path: PathBuf,
}

#[derive(Error, Debug)]
#[error(
    "Repository config has no 'repo' property, and no directory name could be resolved: {path}"
)]
pub struct RepoTabNameError {
    path: PathBuf,
}

#[derive(Error, Debug)]
pub enum LoadYamlError {
    #[error("Failed to load config at path: {0} - error: {1}")]
    IoError(PathBuf, std::io::Error),
    #[error("Failed to deserialize config at path: {0} - error: {1}")]
    SerdeError(PathBuf, serde_yaml::Error),
    #[error("Expected 'tab.yml' within directory: {0}")]
    ExpectedError(PathBuf),
}

#[derive(Error, Debug)]
#[error("Expected {expected} config, but found {found}")]
pub struct ConfigVariantError {
    pub path: PathBuf,
    pub expected: &'static str,
    pub found: &'static str,
}

#[derive(Error, Debug)]
#[error("Repository path does not exist: {path}")]
pub struct RepoNotFoundError {
    pub path: PathBuf,
}

#[derive(Error, Debug)]
#[error("Expected {expected} config, but none could be parsed.  Add 'repo: name' or 'workspace: [entries...]' so this config can be parsed")]
pub struct NoConfigVariantError {
    pub path: PathBuf,
    pub expected: &'static str,
}
#[derive(Error, Debug)]
#[error("Tab {tab} has a working directory that does not exist: {path}")]
pub struct TabDirectoryNotFoundError {
    pub tab: String,
    pub path: PathBuf,
}

#[derive(Error, Debug)]
#[error("Tab {tab} has an invalid name: {reason}")]
pub struct TabNameInvalidError {
    pub tab: String,
    pub reason: String,
}
#[derive(Error, Debug)]
#[error("Tab {tab} has duplicate definitions")]
pub struct TabDuplicateError {
    pub tab: String,
}
