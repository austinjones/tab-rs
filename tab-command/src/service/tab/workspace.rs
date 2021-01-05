use std::{collections::HashSet, path::Path, path::PathBuf, sync::Arc};

use crate::{
    message::tabs::ScanWorkspace,
    prelude::*,
    state::tabs::ActiveTabsState,
    state::workspace::WorkspaceState,
    state::{tab::TabMetadataState, workspace::WorkspaceTab},
};
use lifeline::Service;

use self::loader::{scan_config, WorkspaceTabs};

mod loader;
mod repo;
mod workspace;

/// Loads the workspace configuration using the current directory
pub struct WorkspaceService {
    _scan: Lifeline,
}

#[derive(Debug)]
enum Event {
    ScanWorkspace,
    MetadataState(TabMetadataState),
    ActiveState(Option<ActiveTabsState>),
}

impl Event {
    pub fn scan(_event: ScanWorkspace) -> Self {
        Self::ScanWorkspace
    }

    pub fn metadata(event: TabMetadataState) -> Self {
        Self::MetadataState(event)
    }

    pub fn active(event: Option<ActiveTabsState>) -> Self {
        Self::ActiveState(event)
    }
}

impl Service for WorkspaceService {
    type Bus = TabBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let rx_scan = bus.rx::<ScanWorkspace>()?;
        let rx_metadata = bus.rx::<TabMetadataState>()?;
        let rx_active = bus.rx::<Option<ActiveTabsState>>()?;

        let mut rx = rx_scan
            .map(Event::scan)
            .merge(rx_active.map(Event::active))
            .merge(rx_metadata.map(Event::metadata))
            .log(Level::Debug);

        let mut tx = bus.tx::<Option<WorkspaceState>>()?;

        #[allow(unreachable_code)]
        let _scan = Self::try_task("scan", async move {
            let mut last_active = None;
            let mut current_dir = std::env::current_dir()?;

            while let Some(event) = rx.recv().await {
                // for either event, we update the workspace
                match event {
                    Event::ScanWorkspace => {}
                    Event::ActiveState(active) => {
                        last_active = active;
                    }
                    Event::MetadataState(metadata) => {
                        if let TabMetadataState::Selected(metadata) = metadata {
                            let path = PathBuf::from(&metadata.dir);

                            if !path.exists() {
                                warn!(
                                    "Tab metadata path ({}) does not exist: {}",
                                    &metadata.name, &metadata.dir,
                                );
                                continue;
                            }

                            current_dir = path;
                        }
                    }
                }

                Self::update(&mut tx, last_active.as_ref(), current_dir.as_path()).await?;
            }

            Ok(())
        });

        Ok(Self { _scan })
    }
}

impl WorkspaceService {
    async fn update(
        mut tx: impl Sink<Item = Option<WorkspaceState>> + Unpin,
        active: Option<&ActiveTabsState>,
        current_dir: &Path,
    ) -> anyhow::Result<()> {
        info!("Scanning workspace");
        let global_config = tab_api::config::global_config_file();
        let global_config = global_config.as_ref().map(|c| c.parent()).flatten();

        let scan = scan_config(current_dir, None, global_config);

        let errors: Vec<String> = scan
            .errors()
            .into_iter()
            .map(|err| format!("{}", err))
            .collect();

        let tabs = if let Some(active) = active {
            Self::with_active_tabs(scan, active)
        } else {
            scan.ok()
        };

        let state = WorkspaceState {
            tabs: Arc::new(tabs),
            errors,
        };

        tx.send(Some(state)).await.ok();

        Ok(())
    }

    pub fn with_active_tabs(
        scan: WorkspaceTabs,
        active_tabs: &ActiveTabsState,
    ) -> Vec<WorkspaceTab> {
        // let workspace_tabs = scan.as_name_set();

        let mut tabs = Vec::with_capacity(scan.len());
        tabs.append(&mut scan.ok());
        let scan_tab_names: HashSet<&String> = tabs.iter().map(|tab| &tab.name).collect();

        let mut new_tabs = Vec::with_capacity(active_tabs.tabs.len());
        for (_id, metadata) in active_tabs.tabs.iter() {
            if scan_tab_names.contains(&metadata.name) {
                continue;
            }

            let tab = WorkspaceTab {
                name: metadata.name.clone(),
                doc: metadata.doc.clone(),
                directory: PathBuf::from(&metadata.dir),
                shell: None,
                env: None,
            };

            new_tabs.push(tab);
        }

        drop(scan_tab_names);
        tabs.append(&mut new_tabs);

        tabs.sort_by(|a, b| a.name.cmp(&b.name));
        tabs.dedup_by_key(|tab| tab.name.clone());

        tabs
    }
}

#[cfg(test)]
mod tests {
    use crate::state::workspace::WorkspaceTab;
    use anyhow::bail;
    use pretty_assertions::assert_eq;
    use std::{collections::HashMap, path::PathBuf};

    use super::loader::scan_config;

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
        let tabs = scan_config(path.as_path(), Some(path.as_path()), None).unwrap();
        Ok((path, tabs))
    }

    fn load_ok(name: &str) -> anyhow::Result<(PathBuf, Vec<WorkspaceTab>)> {
        let path = test_dir(name)?;
        let tabs = scan_config(path.as_path(), Some(path.as_path()), None).ok();
        Ok((path, tabs))
    }

    fn convert_env(map: HashMap<&str, &str>) -> HashMap<String, String> {
        let mut output = HashMap::with_capacity(map.len());

        for (k, v) in map.into_iter() {
            output.insert(k.into(), v.into());
        }

        output
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
            $doc.into()
        };
    }

    macro_rules! shell {
        ( $doc:expr ) => {
            $doc.into()
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
        let (dir, tabs) = load_ok("dir")?;

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
        let b_dir = test_dir("workspace-link/b")?;

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
        let tabs = scan_config(inner.as_path(), Some(outer.as_path()), None).unwrap();

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

    #[test]
    fn ignore_test() -> anyhow::Result<()> {
        let dir = test_dir("ignore_dir")?;
        let repo = test_dir("ignore_dir/repo")?;
        let tabs = scan_config(repo.as_path(), Some(dir.as_path()), Some(dir.as_path())).unwrap();

        assert_eq!(
            vec![WorkspaceTab::builder()
                .name("exists/".into())
                .directory(dir!(repo))
                .build()],
            tabs
        );

        Ok(())
    }
}
