use std::path::Path;

use log::info;

use crate::state::{
    workspace::{Workspace, WorkspaceItem, WorkspaceTab},
    workspace_err::WorkspaceError,
};

use super::{
    loader::{load_yml, WorkspaceBuilder, YmlResult},
    repo::build_repo,
};

pub fn build_workspace(builder: &mut WorkspaceBuilder, path: &Path, workspace: Workspace) {
    if builder.contains_workspace(path) {
        return;
    }

    info!("Loading workspace: {}", path.to_string_lossy());

    builder.result(workspace_tab(path, &workspace));

    for item in workspace.workspace.iter() {
        match item {
            WorkspaceItem::Workspace(link) => {
                let mut workspace_path = path.to_path_buf();
                workspace_path.push(link.workspace.as_str());

                if !workspace_path.exists() {
                    builder.err(WorkspaceError::workspace_not_found(workspace_path));
                    continue;
                }

                let workspace_path = workspace_path
                    .canonicalize()
                    .map_err(|err| WorkspaceError::canonicalize_error(workspace_path.clone(), err));

                if let Err(e) = workspace_path {
                    builder.err(e);
                    continue;
                }

                let workspace_path = workspace_path.unwrap();

                let workspace = load_yml(workspace_path.as_path())
                    .required()
                    .map_err(WorkspaceError::load_error)
                    .and_then(|config| config.as_workspace(workspace_path.as_path()));

                if let Err(e) = workspace {
                    builder.err(e);
                    continue;
                }

                let workspace = workspace.unwrap();
                let tab = workspace_tab(workspace_path.as_path(), &workspace);
                builder.result(tab);
            }

            WorkspaceItem::Repo(repo) => {
                let mut repo_path = path.to_path_buf();
                repo_path.push(repo.repo.as_str());

                if !repo_path.exists() {
                    builder.err(WorkspaceError::repo_not_found(repo_path));
                    continue;
                }

                let repo_path = repo_path
                    .canonicalize()
                    .map_err(|err| WorkspaceError::canonicalize_error(repo_path.clone(), err));

                if let Err(e) = repo_path {
                    builder.err(e);
                    continue;
                }

                let repo_path = repo_path.unwrap();

                let repo = match load_yml(repo_path.as_path()) {
                    YmlResult::Ok(config) => config.as_repo(repo_path.as_path()),
                    YmlResult::Err(e) => {
                        builder.err(WorkspaceError::load_error(e));
                        continue;
                    }
                    YmlResult::None(_path) => {
                        let tab_name = repo_tab_name(repo_path.as_path());
                        if let Err(e) = tab_name {
                            builder.err(e);
                            continue;
                        }

                        let tab = WorkspaceTab::new(tab_name.unwrap().as_str(), repo_path);
                        builder.tab(tab);
                        continue;
                    }
                };

                if let Err(e) = repo {
                    builder.err(e);
                    continue;
                }

                build_repo(builder, repo_path.as_path(), repo.unwrap());
            }

            WorkspaceItem::Tab(tab) => {
                let mut directory = path.to_path_buf();

                if let Some(ref dir) = tab.dir {
                    directory.push(dir);
                }

                let options = tab.options.clone().or(workspace.options.clone());

                let tab = WorkspaceTab::with_options(tab.tab.as_str(), directory, options);
                builder.tab(tab);
            }
        }
    }

    builder.workspace(path.to_path_buf());
}

fn workspace_tab(path: &Path, workspace: &Workspace) -> Result<WorkspaceTab, WorkspaceError> {
    workspace_tab_name(path, &workspace)
        .map(|name| {
            WorkspaceTab::with_options(name.as_str(), path.to_owned(), workspace.options.clone())
        })
        .map(|mut tab| {
            tab.doc = Some(workspace_tab_doc(path, workspace));
            tab
        })
}

fn workspace_tab_name(path: &Path, workspace: &Workspace) -> Result<String, WorkspaceError> {
    if let Some(ref tab) = workspace.tab {
        return Ok(tab.clone());
    }

    if let Some(file_name) = path.file_name() {
        return Ok(file_name.to_string_lossy().to_string());
    }

    Err(WorkspaceError::workspace_tab_name_error(path.to_path_buf()))
}

fn repo_tab_name(path: &Path) -> Result<String, WorkspaceError> {
    if let Some(file_name) = path.file_name() {
        return Ok(file_name.to_string_lossy().to_string());
    }

    Err(WorkspaceError::repo_tab_name_error(path.to_path_buf()))
}

fn workspace_tab_doc(path: &Path, workspace: &Workspace) -> String {
    if let Some(ref doc) = workspace.options.doc {
        return doc.clone();
    }

    if let Some(name) = path.file_name() {
        format!("workspace tab for {}", name.to_string_lossy())
    } else {
        format!("workspace tab")
    }
}
