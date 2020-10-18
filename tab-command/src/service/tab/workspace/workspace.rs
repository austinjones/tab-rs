use std::path::Path;

use crate::state::workspace::{Config, Workspace, WorkspaceItem, WorkspaceTab};

use super::{
    loader::{load_yml, TabIter},
    repo::repo_iter,
};
use anyhow::anyhow;

pub fn workspace_iter(path: &Path, workspace: Workspace) -> TabIter {
    let mut iter = TabIter::new();

    if let Some(tab) = workspace_tab(path, &workspace) {
        iter.and(tab);
    }

    for item in workspace.workspace.iter() {
        match item {
            WorkspaceItem::Workspace(link) => {
                let mut workspace_path = path.to_path_buf();
                workspace_path.push(link.workspace.as_str());

                if let Some(workspace) = load_yml(workspace_path.as_path()) {
                    if let Err(e) = workspace {
                        iter.and_err(e.context(format!(
                            "Loading workspace: {}/tab.yml",
                            workspace_path.to_string_lossy()
                        )));
                        continue;
                    }

                    let workspace = workspace.unwrap();

                    if let Config::Workspace(workspace) = workspace {
                        if let Some(tab) = workspace_tab(workspace_path.as_path(), &workspace) {
                            iter.and(tab);
                        }
                    }
                }
            }

            WorkspaceItem::Repo(repo) => {
                let mut repo_path = path.to_path_buf();
                repo_path.push(repo.repo.as_str());
                if let Some(repo) = load_yml(repo_path.as_path()) {
                    if let Err(e) = repo {
                        iter.and_err(e);
                        continue;
                    }

                    let repo = repo.unwrap();
                    if let Config::Repo(repo) = repo {
                        iter.append(repo_iter(repo_path.as_path(), repo));
                    } else {
                        iter.and_err(anyhow!(
                            "Expected a repository config at path: {}",
                            repo_path.to_string_lossy()
                        ));
                    }
                } else if repo_path.exists() {
                    let tab = WorkspaceTab::new(repo.repo.as_str(), repo_path);
                    iter.and(tab);
                }
            }

            WorkspaceItem::Tab(tab) => {
                let mut directory = path.to_path_buf();

                if let Some(ref dir) = tab.dir {
                    directory.push(dir);
                }

                let options = tab.options.clone().or(workspace.options.clone());

                let tab = WorkspaceTab::with_options(tab.tab.as_str(), directory, options);
                iter.and(tab);
            }
        }
    }

    iter
}

fn workspace_tab(path: &Path, workspace: &Workspace) -> Option<WorkspaceTab> {
    workspace_tab_name(path, &workspace)
        .map(|name| {
            WorkspaceTab::with_options(name.as_str(), path.to_owned(), workspace.options.clone())
        })
        .map(|mut tab| {
            tab.doc = Some(workspace_tab_doc(path, workspace));
            tab
        })
}

fn workspace_tab_name(path: &Path, workspace: &Workspace) -> Option<String> {
    if let Some(ref tab) = workspace.tab {
        return Some(tab.clone());
    }

    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
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
