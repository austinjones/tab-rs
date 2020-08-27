use crate::{
    normalize_name,
    prelude::*,
    state::workspace::{Config, Repo, Tab, Workspace, WorkspaceItem, WorkspaceState, WorkspaceTab},
};
use anyhow::Context;
use lifeline::Service;
use std::{
    collections::HashMap,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};
use time::Duration;
use tokio::time;

pub struct WorkspaceService {
    _monitor: Lifeline,
}

impl Service for WorkspaceService {
    type Bus = TabBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut tx = bus.tx::<WorkspaceState>()?;
        let _monitor = Self::try_task("monitor", async move {
            // find the workspace root
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
                // read the workspace root, and all children
                time::delay_for(Duration::from_millis(250)).await;
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

fn load_state() -> anyhow::Result<LoaderState> {
    // let config = Config::Workspace(Workspace {
    //     workspace: vec![WorkspaceItem::Tab(Tab {
    //         tab: "foo".to_owned(),
    //         dir: None,
    //         command: None,
    //     })],
    // });

    // let string = serde_yaml::to_string(&config)?;
    // error!("serialized: {}", string);

    let mut loader_state = LoaderState {
        repos: Vec::new(),
        tabs: Vec::new(),
        workspace: None,
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
                    loader_state.workspace = Some(workspace);
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

enum LoadError {
    NoConfig,
    SerdeError,
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
        for tab in repo.tabs {
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
