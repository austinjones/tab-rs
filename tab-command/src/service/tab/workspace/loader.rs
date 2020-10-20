use std::{collections::HashSet, fs::File, io::BufReader, path::Path, path::PathBuf};

use crate::state::{
    workspace::{Config, WorkspaceTab},
    workspace_err::LoadYamlError,
    workspace_err::WorkspaceError,
    workspace_err::WorkspaceResult,
};

use super::{repo::build_repo, workspace::build_workspace};

pub struct WorkspaceBuilder {
    elems: Vec<WorkspaceResult>,
    tabs: HashSet<String>,
    workspaces: HashSet<PathBuf>,
    repos: HashSet<PathBuf>,
}

pub struct WorkspaceTabs {
    elems: Vec<WorkspaceResult>,
}

impl WorkspaceBuilder {
    pub fn new() -> WorkspaceBuilder {
        Self {
            elems: Vec::new(),
            tabs: HashSet::new(),
            workspaces: HashSet::new(),
            repos: HashSet::new(),
        }
    }

    pub fn contains_workspace(&self, path: &Path) -> bool {
        self.workspaces.contains(path)
    }

    pub fn contains_repo(&self, path: &Path) -> bool {
        self.repos.contains(path)
    }

    pub fn workspace(&mut self, path: PathBuf) {
        self.workspaces.insert(path);
    }

    pub fn repo(&mut self, path: PathBuf) {
        self.repos.insert(path);
    }

    pub fn tab(&mut self, tab: WorkspaceTab) -> &mut Self {
        if self.tabs.contains(&tab.name) {
            self.err(WorkspaceError::duplicate_tab(tab.name));
            return self;
        }

        self.validate(&tab);
        self.tabs.insert(tab.name.clone());
        self.elems.push(Ok(tab));
        self
    }

    pub fn err(&mut self, err: WorkspaceError) -> &mut Self {
        self.elems.push(Err(err));
        self
    }

    pub fn result(&mut self, result: Result<WorkspaceTab, WorkspaceError>) -> &mut Self {
        self.elems.push(result);
        self
    }

    fn validate(&mut self, tab: &WorkspaceTab) {
        if !tab.directory.exists() {
            self.err(WorkspaceError::tab_directory_not_found(
                tab.name.clone(),
                tab.directory.clone(),
            ));
        }

        if let Err(e) = validate_tab_name(tab.name.as_str()) {
            self.err(WorkspaceError::tab_name_invalid(tab.name.clone(), e));
        }
    }

    pub fn build(self) -> WorkspaceTabs {
        WorkspaceTabs { elems: self.elems }
    }
}

fn validate_tab_name(name: &str) -> Result<(), String> {
    if name.starts_with('-') {
        return Err("tab name may not begin with a dash".into());
    }

    if name.contains(' ') || name.contains('\t') {
        return Err("tab name may not contain whitespace".into());
    }

    if name.contains('\\') {
        return Err("tab name may not contain backslashes".into());
    }

    Ok(())
}

impl WorkspaceTabs {
    #[cfg(test)]
    pub fn unwrap(self) -> Vec<WorkspaceTab> {
        let mut tabs = Vec::with_capacity(self.elems.len());

        for elem in self.elems {
            let elem = elem.unwrap();
            tabs.push(elem);
        }

        tabs
    }

    pub fn ok(self) -> Vec<WorkspaceTab> {
        let mut tabs = Vec::with_capacity(self.elems.len());

        for elem in self.elems {
            if let Ok(tab) = elem {
                tabs.push(tab);
            }
        }

        tabs
    }

    pub fn errors(&self) -> Vec<&WorkspaceError> {
        let mut errors = Vec::with_capacity(self.elems.len());

        for elem in &self.elems {
            if let Err(e) = elem {
                errors.push(e);
            }
        }

        errors
    }

    pub fn len(&self) -> usize {
        self.elems.len()
    }

    pub fn as_name_set(&self) -> HashSet<&String> {
        let mut set = HashSet::with_capacity(self.elems.len());

        for result in self.elems.iter() {
            if let Ok(tab) = result {
                set.insert(&tab.name);
            }
        }

        set
    }
}

pub fn scan_config(dir: &Path, base: Option<&Path>) -> WorkspaceTabs {
    let mut builder = WorkspaceBuilder::new();
    let mut working_dir = Some(dir);

    while let Some(dir) = working_dir {
        if let Some(base) = base {
            if dir != base && base.starts_with(dir) {
                break;
            }
        }

        match load_yml(dir) {
            YmlResult::Ok(Config::Workspace(workspace)) => {
                build_workspace(&mut builder, dir, workspace);
            }
            YmlResult::Ok(Config::Repo(repo)) => build_repo(&mut builder, dir, repo),
            YmlResult::Ok(Config::None) => {
                builder.err(WorkspaceError::none_error(
                    dir.to_path_buf(),
                    "Workspace or Repo",
                ));
            }
            YmlResult::Err(err) => {
                builder.err(WorkspaceError::load_error(err));
            }
            YmlResult::None(_) => {}
        };

        working_dir = dir.parent();
    }

    builder.build()
}

pub enum YmlResult {
    Ok(Config),
    Err(LoadYamlError),
    None(PathBuf),
}

impl YmlResult {
    pub fn required(self) -> Result<Config, LoadYamlError> {
        match self {
            YmlResult::Ok(config) => Ok(config),
            YmlResult::Err(e) => Err(e),
            YmlResult::None(path) => Err(LoadYamlError::ExpectedError(path)),
        }
    }
}

impl From<Result<Config, LoadYamlError>> for YmlResult {
    fn from(result: Result<Config, LoadYamlError>) -> Self {
        match result {
            Ok(yml) => Self::Ok(yml),
            Err(e) => Self::Err(e),
        }
    }
}

pub fn load_yml(dir: &Path) -> YmlResult {
    let path = yml_path(dir);
    if let None = path {
        return YmlResult::None(dir.to_path_buf());
    }

    let path = path.unwrap();
    load_file(path.as_path()).into()
}

fn yml_path(dir: &Path) -> Option<PathBuf> {
    let mut path_buf = dir.to_owned();
    path_buf.push("tab.yml");

    if path_buf.is_file() {
        return Some(path_buf);
    }

    path_buf.pop();

    if path_buf.is_file() {
        return Some(path_buf);
    }

    None
}

fn load_file(path: &Path) -> Result<Config, LoadYamlError> {
    // TODO: figure out how to get rid fo the blocking IO
    let reader = File::open(path).map_err(|err| LoadYamlError::IoError(path.to_owned(), err))?;

    let buf_reader = BufReader::new(reader);

    serde_yaml::from_reader(buf_reader)
        .map_err(|err| LoadYamlError::SerdeError(path.to_owned(), err))
}
