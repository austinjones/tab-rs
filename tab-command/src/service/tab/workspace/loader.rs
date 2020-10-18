use std::{fs::File, io::BufReader, path::Path};

use anyhow::Context;
use log::error;

use crate::state::workspace::{Config, WorkspaceTab};

use super::{repo::repo_iter, workspace::workspace_iter};

pub struct TabIter {
    elems: Vec<anyhow::Result<WorkspaceTab>>,
}

impl TabIter {
    pub fn new() -> TabIter {
        Self { elems: Vec::new() }
    }

    pub fn and(&mut self, elem: WorkspaceTab) -> &mut Self {
        self.elems.push(Ok(elem));
        self
    }

    pub fn and_err(&mut self, err: anyhow::Error) -> &mut Self {
        self.elems.push(Err(err));
        self
    }

    pub fn append(&mut self, mut iter: TabIter) -> &mut Self {
        self.elems.append(&mut iter.elems);
        self
    }

    #[cfg(test)]
    pub fn unwrap(self) -> Vec<WorkspaceTab> {
        let mut tabs = Vec::with_capacity(self.elems.len());

        for elem in self.elems {
            let elem = elem.unwrap();
            tabs.push(elem);
        }

        tabs
    }

    pub fn unwrap_log(self) -> Vec<WorkspaceTab> {
        let mut tabs = Vec::with_capacity(self.elems.len());

        for elem in self.elems {
            if let Err(e) = elem {
                error!("Failed to load workspace entry: {}", e);
                continue;
            }

            let elem = elem.unwrap();
            tabs.push(elem);
        }

        tabs
    }
}

impl IntoIterator for TabIter {
    type Item = anyhow::Result<WorkspaceTab>;
    type IntoIter = std::vec::IntoIter<anyhow::Result<WorkspaceTab>>;

    fn into_iter(self) -> Self::IntoIter {
        self.elems.into_iter()
    }
}

pub fn scan_config(dir: &Path, base: Option<&Path>) -> TabIter {
    let mut iter = TabIter::new();
    let mut working_dir = Some(dir);

    while let Some(dir) = working_dir {
        if let Some(base) = base {
            if dir != base && base.starts_with(dir) {
                break;
            }
        }

        let config = load_yml(dir);

        if config.is_none() {
            working_dir = dir.parent();
            continue;
        }

        let config = config
            .unwrap()
            .context(format!("Loading {}/tab.yml", dir.to_string_lossy()));

        if let Err(e) = config {
            iter.and_err(e);
            working_dir = dir.parent();
            continue;
        }

        let config = config.unwrap();

        let items = match config {
            Config::Workspace(workspace) => workspace_iter(dir, workspace),
            Config::Repo(repo) => repo_iter(dir, repo),
        };

        iter.append(items);

        working_dir = dir.parent();
    }

    iter
}

pub fn load_yml(dir: &Path) -> Option<anyhow::Result<Config>> {
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
