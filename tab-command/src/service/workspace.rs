use crate::{
    prelude::*,
    state::workspace::{Config, Repo, Workspace, WorkspaceItem, WorkspaceState, WorkspaceTab},
};
use anyhow::Context;
use lifeline::Service;
use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};
use tab_api::tab::normalize_name;
use time::Duration;
use tokio::time;

/// Loads the workspace configuration using the current directory
pub struct WorkspaceService {
    _monitor: Lifeline,
}

impl Service for WorkspaceService {
    type Bus = TabBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut tx = bus.tx::<WorkspaceState>()?;

        #[allow(unreachable_code)]
        let _monitor = Self::try_task("monitor", async move {
            loop {
                let state = load_state();

                if let Err(err) = state {
                    error!("failed to load config: {:?}", err);
                } else {
                    let loader_state = state.unwrap();
                    let tabs = tabs(loader_state);
                    let state = WorkspaceState::Ready(tabs);
                    tx.send(state).await.ok();
                }

                time::delay_for(Duration::from_millis(1000)).await;
            }

            Ok(())
        });

        Ok(Self { _monitor })
    }
}

struct LoaderState {
    pub repos: Vec<(PathBuf, Repo)>,

    /// The list of compiled tabs
    pub tabs: Vec<WorkspaceTab>,

    /// The hierarchy of workspaces, starting with the innermost
    pub workspaces: Vec<Workspace>,
}

fn load_state() -> anyhow::Result<LoaderState> {
    let mut loader_state = LoaderState {
        repos: Vec::new(),
        tabs: Vec::new(),
        workspaces: Vec::new(),
    };

    let init_dir = std::env::current_dir()?;
    let mut working_dir: Option<&Path> = Some(init_dir.as_path());

    while let Some(dir) = working_dir {
        let config = load_yml(dir);
        if let Some(config) = config {
            let config = config.context(dir.to_string_lossy().to_string())?;
            match config {
                Config::Workspace(workspace) => {
                    load_items(dir, &workspace, &mut loader_state)?;
                    loader_state.workspaces.push(workspace);
                }
                Config::Repo(repo) => {
                    let repo_path = dir.to_path_buf();
                    loader_state.repos.push((repo_path, repo));
                }
            }
        }

        working_dir = dir.parent();
    }

    Ok(loader_state)
}

fn load_yml(dir: &Path) -> Option<anyhow::Result<Config>> {
    let mut path_buf = dir.to_owned();
    path_buf.push("tab.yml");

    if path_buf.is_file() {
        return Some(load_file(path_buf.as_path()));
    }

    let mut path_buf = dir.to_owned();
    path_buf.push(".tab.yml");

    if path_buf.is_file() {
        return Some(load_file(path_buf.as_path()));
    }

    None
}

fn load_file(path: &Path) -> anyhow::Result<Config> {
    // TODO: figure out how to get rid fo the blocking IO
    let reader = File::open(path)?;
    let buf_reader = BufReader::new(reader);
    let config = serde_yaml::from_reader(buf_reader)?;
    Ok(config)
}

fn load_items(path: &Path, workspace: &Workspace, target: &mut LoaderState) -> anyhow::Result<()> {
    if let Some(tab) = workspace_tab(path, workspace) {
        target.tabs.push(tab);
    }

    for item in workspace.workspace.iter() {
        match item {
            WorkspaceItem::Workspace(link) => {
                let mut workspace_path = path.to_path_buf();
                workspace_path.push(link.workspace.as_str());

                if let Some(workspace) = load_yml(workspace_path.as_path()) {
                    let workspace = workspace?;
                    if let Config::Workspace(workspace) = workspace {
                        if let Some(tab) = workspace_tab(workspace_path.as_path(), &workspace) {
                            target.tabs.push(tab);
                        }
                    }
                }
            }

            WorkspaceItem::Repo(repo) => {
                let mut repo_path = path.to_path_buf();
                repo_path.push(repo.repo.as_str());
                if let Some(repo) = load_yml(repo_path.as_path()) {
                    let repo = repo?;
                    if let Config::Repo(repo) = repo {
                        target.repos.push((repo_path, repo));
                    }
                } else if repo_path.exists() {
                    let tab = WorkspaceTab {
                        name: normalize_name(repo.repo.as_str()),
                        directory: repo_path,
                        doc: "".to_string(),
                    };

                    target.tabs.push(tab);
                }
            }

            WorkspaceItem::Tab(tab) => {
                let mut directory = path.to_path_buf();

                if let Some(ref dir) = tab.dir {
                    directory.push(dir);
                }

                let tab = WorkspaceTab {
                    name: normalize_name(tab.tab.as_str()),
                    directory,
                    doc: tab.doc.as_ref().unwrap_or(&"".to_string()).clone(),
                };

                target.tabs.push(tab);
            }
        }
    }

    Ok(())
}

fn workspace_tab(path: &Path, workspace: &Workspace) -> Option<WorkspaceTab> {
    workspace_tab_name(path, &workspace).map(|name| WorkspaceTab {
        name: normalize_name(name.as_str()),
        directory: path.to_owned(),
        doc: workspace_tab_doc(path, workspace),
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
    if let Some(ref doc) = workspace.doc {
        return doc.clone();
    }

    if let Some(name) = path.file_name() {
        format!("workspace tab for {}", name.to_string_lossy())
    } else {
        format!("workspace tab")
    }
}

fn tabs(mut loader: LoaderState) -> Vec<WorkspaceTab> {
    let mut tabs = Vec::new();
    tabs.append(&mut loader.tabs);

    for (path, repo) in loader.repos.into_iter() {
        let repo_name = normalize_name(repo.repo.as_str());

        // push a tab for the repo
        let tab = WorkspaceTab {
            name: repo_name.clone(),
            directory: path.clone(),
            doc: repo.doc.unwrap_or("".to_string()),
        };
        tabs.push(tab);

        // and then for any tabs the user defined
        for tab in repo.tabs.into_iter().flat_map(|t| t.into_iter()) {
            let mut directory = path.to_path_buf();
            if let Some(subdir) = tab.dir {
                directory.push(subdir);
            }

            let tab_name = normalize_name(tab.tab.as_str());
            let tab_name = repo_name.clone() + tab_name.as_str();

            let tab = WorkspaceTab {
                name: tab_name,
                directory,
                doc: tab.doc.unwrap_or("".to_string()), // command: tab.command,
            };

            tabs.push(tab);
        }
    }

    tabs
}
