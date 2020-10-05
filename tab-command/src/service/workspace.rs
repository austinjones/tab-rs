use crate::{
    prelude::*,
    state::workspace::{
        Config, Repo, Workspace, WorkspaceIndex, WorkspaceItem, WorkspaceState, WorkspaceTab,
    },
};
use anyhow::anyhow;
use anyhow::Context;
use lifeline::Service;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs::File,
    io::BufReader,
    io::BufWriter,
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
    pub tabs: Vec<WorkspaceTab>,
    pub workspace: Option<Workspace>,
}

// An index of known workspace roots
fn load_index() -> anyhow::Result<WorkspaceIndex> {
    let path = tab_api::config::workspace_index()?;
    if !path.is_file() {
        return Ok(WorkspaceIndex { workspaces: vec![] });
    }

    load_file(path.as_path())
}

fn save_index(index: &WorkspaceIndex) -> anyhow::Result<()> {
    let path = tab_api::config::workspace_index()?;
    save_file(path.as_path(), index)?;

    Ok(())
}

fn load_state() -> anyhow::Result<LoaderState> {
    let mut loader_state = LoaderState {
        repos: Vec::new(),
        tabs: Vec::new(),
        workspace: None,
    };

    let init_dir = std::env::current_dir()?;
    let mut working_dir: Option<&Path> = Some(init_dir.as_path());

    load_workspace_tabs(&mut loader_state)?;

    while let Some(dir) = working_dir {
        let config = load_yml(dir);
        if let Some(config) = config {
            let config = config.context(dir.to_string_lossy().to_string())?;
            match config {
                Config::Workspace(workspace) => {
                    load_items(dir, &workspace, &mut loader_state)?;
                    loader_state.workspace = Some(workspace);
                    save_workspace_index(dir)?;
                    break;
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

fn load_workspace_tabs(loader_state: &mut LoaderState) -> anyhow::Result<()> {
    let workspace_index = load_index()?;
    for str_path in workspace_index.workspaces {
        let path: &Path = &Path::new(str_path.as_str());

        let config = load_yml(path);

        if !config.is_some() {
            continue;
        }

        let config = config.unwrap();

        if !config.is_ok() {
            warn!("Failed to parse indexed workspace at path: {}", str_path);
            continue;
        }

        let config = config.unwrap();

        if let Config::Workspace(workspace) = config {
            if let Some(tab) = workspace.tab {
                let tab = WorkspaceTab {
                    name: normalize_name(tab.as_str()),
                    directory: path.to_owned(),
                    doc: workspace.doc.unwrap_or("".to_owned()),
                };

                loader_state.tabs.push(tab);
            } else if let Some(dir_name) = path.file_name() {
                let name = dir_name.to_string_lossy().to_string();

                let tab = WorkspaceTab {
                    name: normalize_name(name.as_str()),
                    directory: path.to_owned(),
                    doc: format!("workspace tab for {}", name),
                };

                loader_state.tabs.push(tab);
            }
        }
    }

    Ok(())
}

fn save_workspace_index(dir: &Path) -> anyhow::Result<()> {
    let mut workspace_index = load_index()?;
    let string = dir
        .to_str()
        .ok_or_else(|| {
            anyhow!(format!(
                "workspace path is not a valid rust string: {}",
                dir.to_string_lossy()
            ))
        })?
        .to_owned();

    if workspace_index.workspaces.contains(&string) {
        return Ok(());
    }

    workspace_index.workspaces.push(string);
    save_index(&workspace_index)?;

    Ok(())
}

fn load_file<T>(path: &Path) -> anyhow::Result<T>
where
    T: DeserializeOwned,
{
    // TODO: figure out how to get rid fo the blocking IO
    let reader = File::open(path)?;
    let buf_reader = BufReader::new(reader);
    let config = serde_yaml::from_reader(buf_reader)?;
    Ok(config)
}

fn save_file<T>(path: &Path, data: &T) -> anyhow::Result<()>
where
    T: Serialize,
{
    // TODO: figure out how to get rid fo the blocking IO
    let reader = File::create(path)?;
    let buf_writer = BufWriter::new(reader);
    serde_yaml::to_writer(buf_writer, data)?;

    Ok(())
}

fn load_items(path: &Path, workspace: &Workspace, target: &mut LoaderState) -> anyhow::Result<()> {
    for item in workspace.workspace.iter() {
        match item {
            WorkspaceItem::Repo(repo) => {
                let mut repo_path = path.to_path_buf();
                repo_path.push(repo.repo.as_str());
                if let Some(repo) = load_yml(repo_path.as_path()) {
                    let repo = repo?;
                    if let Config::Repo(repo) = repo {
                        target.repos.push((repo_path, repo));
                    }
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
                    // command: tab.command.clone(),
                };

                target.tabs.push(tab);
            }
        }
    }

    Ok(())
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
            // command: None,
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
