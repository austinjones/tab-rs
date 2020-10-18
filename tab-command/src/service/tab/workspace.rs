use crate::{prelude::*, state::workspace::WorkspaceState};
use lifeline::Service;

use self::loader::scan_config;

mod loader;
mod repo;
mod workspace;

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
            let dir = std::env::current_dir()?;
            let state = scan_config(dir.as_path(), None);
            let tabs = state.unwrap_log();
            let state = WorkspaceState { tabs };
            tx.send(Some(state)).await.ok();

            Ok(())
        });

        Ok(Self { _monitor })
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
        let tabs = scan_config(path.as_path(), Some(path.as_path())).unwrap();
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
        let tabs = scan_config(inner.as_path(), Some(outer.as_path())).unwrap();

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
