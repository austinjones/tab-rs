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
        let mut tx = bus.tx::<Option<WorkspaceState>>()?;

        #[allow(unreachable_code)]
        let _monitor = Self::try_task("monitor", async move {
            let mut logged = false;
            loop {
                let state = load_state();

                if let Err(err) = state {
                    if !logged {
                        error!("failed to load config: {:?}", err);
                        logged = true;
                    }
                } else {
                    let loader_state = state.unwrap();
                    let tabs = tabs(loader_state);
                    let state = WorkspaceState { tabs };
                    tx.send(Some(state)).await.ok();
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
    let init_dir = std::env::current_dir()?;
    load_state_in(init_dir.as_path(), None)
}

fn load_state_in(path: &Path, base: Option<&Path>) -> anyhow::Result<LoaderState> {
    let mut loader_state = LoaderState {
        repos: Vec::new(),
        tabs: Vec::new(),
        workspaces: Vec::new(),
    };

    let mut working_dir: Option<&Path> = Some(path);

    while let Some(dir) = working_dir {
        if let Some(base) = base {
            if dir != base && base.starts_with(dir) {
                break;
            }
        }

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
                    let tab = WorkspaceTab::new(repo.repo.as_str(), repo_path);
                    target.tabs.push(tab);
                }
            }

            WorkspaceItem::Tab(tab) => {
                let mut directory = path.to_path_buf();

                if let Some(ref dir) = tab.dir {
                    directory.push(dir);
                }

                let options = tab.options.clone().or(workspace.options.clone());

                let tab = WorkspaceTab::with_options(tab.tab.as_str(), directory, options);
                target.tabs.push(tab);
            }
        }
    }

    Ok(())
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

fn tabs(mut loader: LoaderState) -> Vec<WorkspaceTab> {
    let mut tabs = Vec::new();
    tabs.append(&mut loader.tabs);

    for (path, repo) in loader.repos.into_iter() {
        let repo_name = normalize_name(repo.repo.as_str());

        // push a tab for the repo
        let tab =
            WorkspaceTab::with_options(repo_name.as_str(), path.clone(), repo.tab_options.clone());
        tabs.push(tab);

        // and then for any tabs the user defined
        for tab in repo.tabs.into_iter().flat_map(|t| t.into_iter()) {
            let mut directory = path.to_path_buf();
            if let Some(subdir) = tab.dir {
                directory.push(subdir);
            }

            let tab_name = normalize_name(tab.tab.as_str());
            let tab_name = repo_name.clone() + tab_name.as_str();

            let options = tab.options.or(repo.tab_options.clone());

            let tab = WorkspaceTab::with_options(tab_name.as_str(), directory, options);
            tabs.push(tab);
        }
    }

    tabs
}

#[cfg(test)]
mod tests {
    use super::{load_state_in, tabs};
    use crate::state::workspace::WorkspaceTab;
    use anyhow::bail;
    use pretty_assertions::assert_eq;
    use std::{collections::HashMap, path::PathBuf};

    fn test_dir(name: &str) -> anyhow::Result<PathBuf> {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("test_resources");
        path.push(name);

        if !path.exists() {
            bail!("Test directory does not exist: {}", &path.to_string_lossy())
        }

        Ok(path)
    }

    fn load(name: &str) -> anyhow::Result<(PathBuf, Vec<WorkspaceTab>)> {
        let path = test_dir(name)?;
        let state = load_state_in(path.as_path(), Some(path.as_path()))?;
        let tabs = tabs(state);
        Ok((path, tabs))
    }

    fn convert_env(map: HashMap<&str, &str>) -> Option<HashMap<String, String>> {
        let mut output = HashMap::with_capacity(map.len());

        for (k, v) in map.into_iter() {
            output.insert(k.into(), v.into());
        }

        Some(output)
    }

    macro_rules! env {
        ( $($k:expr => $v:expr),* $(,)? ) => {
            convert_env(maplit::hashmap!{
                $($k => $v,)*
            })
        };
    }

    macro_rules! doc {
        ( $doc:expr ) => {
            Some($doc.into())
        };
    }

    macro_rules! shell {
        ( $doc:expr ) => {
            Some($doc.into())
        };
    }

    macro_rules! dir {
        ( $base:expr $(, $($elem:expr),* $(,)? )? ) => {
            {
                #[allow(unused_mut)]
                let mut base = PathBuf::from($base.as_path());

                $(
                    $(
                        base.push($elem);
                    )*
                )?


                base
            }
        };
    }

    #[test]
    fn simple_test() -> anyhow::Result<()> {
        let (dir, tabs) = load("simple")?;

        let expected = vec![
            WorkspaceTab::builder()
                .name("simple/".into())
                .doc(doc!("workspace tab for simple"))
                .directory(dir!(dir))
                .build(),
            WorkspaceTab::builder()
                .name("project/".into())
                .doc(None)
                .directory(dir!(dir, "project"))
                .build(),
        ];

        assert_eq!(expected, tabs);

        Ok(())
    }

    #[test]
    fn doc_test() -> anyhow::Result<()> {
        let (dir, tabs) = load("doc")?;

        let expected = vec![
            WorkspaceTab::builder()
                .name("doc/".into())
                .doc(doc!("doc workspace"))
                .directory(dir!(dir))
                .build(),
            WorkspaceTab::builder()
                .name("workspace-tab/".into())
                .doc(doc!("doc workspace tab"))
                .directory(dir!(dir))
                .build(),
            WorkspaceTab::builder()
                .name("project/".into())
                .doc(doc!("doc project"))
                .directory(dir!(dir, "project"))
                .build(),
            WorkspaceTab::builder()
                .name("project/project-tab/".into())
                .doc(doc!("doc project tab"))
                .directory(dir!(dir, "project"))
                .build(),
        ];

        assert_eq!(expected, tabs);

        Ok(())
    }

    #[test]
    fn dir_test() -> anyhow::Result<()> {
        let (dir, tabs) = load("dir")?;

        let expected = vec![
            WorkspaceTab::builder()
                .name("dir/".into())
                .doc(doc!("workspace tab for dir"))
                .directory(dir!(dir))
                .build(),
            WorkspaceTab::builder()
                .name("workspace-exists/".into())
                .directory(dir!(dir, "project"))
                .build(),
            WorkspaceTab::builder()
                .name("workspace-not-exists/".into())
                .directory(dir!(dir, "not-exists"))
                .build(),
            WorkspaceTab::builder()
                .name("project/".into())
                .directory(dir!(dir, "project"))
                .build(),
            WorkspaceTab::builder()
                .name("project/project-exists/".into())
                .directory(dir!(dir, "project", "subdir"))
                .build(),
            WorkspaceTab::builder()
                .name("project/project-not-exists/".into())
                .directory(dir!(dir, "project", "not-exists"))
                .build(),
        ];

        assert_eq!(expected, tabs);

        Ok(())
    }

    #[test]
    fn env_test() -> anyhow::Result<()> {
        let (dir, tabs) = load("env")?;

        let expected = vec![
            WorkspaceTab::builder()
                .name("env/".into())
                .doc(doc!("workspace tab for env"))
                .directory(dir!(dir))
                .env(env! {
                    "inherit" => "inherit",
                    "override" => "base"
                })
                .build(),
            WorkspaceTab::builder()
                .name("workspace-tab/".into())
                .directory(dir!(dir))
                .env(env! {
                    "inherit" => "inherit",
                    "override" => "override",
                    "unique" => "unique",
                })
                .build(),
            WorkspaceTab::builder()
                .name("project/".into())
                .directory(dir!(dir, "project"))
                .env(env! {
                   "inherit-repo" => "inherit",
                   "override-repo" => "base"
                })
                .build(),
            WorkspaceTab::builder()
                .name("project/project-tab/".into())
                .directory(dir!(dir, "project"))
                .env(env! {
                    "inherit-repo" => "inherit",
                    "override-repo" => "override",
                    "unique-repo" => "unique",
                })
                .build(),
        ];

        assert_eq!(expected, tabs);

        Ok(())
    }

    #[test]
    fn shell_test() -> anyhow::Result<()> {
        let (dir, tabs) = load("shell")?;

        let expected = vec![
            WorkspaceTab::builder()
                .name("shell/".into())
                .doc(doc!("workspace tab for shell"))
                .directory(dir!(dir))
                .shell(shell!("workspace-shell"))
                .build(),
            WorkspaceTab::builder()
                .name("workspace-inherit/".into())
                .directory(dir!(dir))
                .shell(shell!("workspace-shell"))
                .build(),
            WorkspaceTab::builder()
                .name("workspace-override/".into())
                .directory(dir!(dir))
                .shell(shell!("workspace-override-shell"))
                .build(),
            WorkspaceTab::builder()
                .name("project/".into())
                .directory(dir!(dir, "project"))
                .shell(shell!("project-shell"))
                .build(),
            WorkspaceTab::builder()
                .name("project/project-inherit/".into())
                .directory(dir!(dir, "project"))
                .shell(shell!("project-shell"))
                .build(),
            WorkspaceTab::builder()
                .name("project/project-override/".into())
                .directory(dir!(dir, "project"))
                .shell(shell!("project-override-shell"))
                .build(),
        ];

        assert_eq!(expected, tabs);

        Ok(())
    }

    #[test]
    fn workspace_name_test() -> anyhow::Result<()> {
        let (dir, tabs) = load("workspace-tab")?;

        let expected = vec![WorkspaceTab::builder()
            .name("other-name/".into())
            .doc(doc!("other doc"))
            .directory(dir!(dir))
            .build()];

        assert_eq!(expected, tabs);

        Ok(())
    }

    #[test]
    fn workspace_link_test() -> anyhow::Result<()> {
        let (dir, tabs) = load("workspace-link/a")?;
        let b_dir = test_dir("workspace-link/a/../b")?;

        let expected = vec![
            WorkspaceTab::builder()
                .name("a/".into())
                .doc(doc!("workspace tab for a"))
                .directory(dir!(dir))
                .build(),
            WorkspaceTab::builder()
                .name("b/".into())
                .doc(doc!("workspace tab for b"))
                .directory(dir!(b_dir))
                .build(),
        ];

        assert_eq!(expected, tabs);

        Ok(())
    }

    #[test]
    fn workspace_nested_test() -> anyhow::Result<()> {
        let outer = test_dir("workspace-nested/")?;
        let inner = test_dir("workspace-nested/sub-workspace/")?;
        let loader_state = load_state_in(inner.as_path(), Some(outer.as_path()))?;
        let tabs = tabs(loader_state);

        let expected = vec![
            WorkspaceTab::builder()
                .name("sub-workspace/".into())
                .doc(doc!("workspace tab for sub-workspace"))
                .directory(dir!(inner))
                .build(),
            WorkspaceTab::builder()
                .name("workspace-nested/".into())
                .doc(doc!("workspace tab for workspace-nested"))
                .directory(dir!(outer))
                .build(),
        ];

        assert_eq!(expected, tabs);

        Ok(())
    }
}
