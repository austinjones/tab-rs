use crate::{
    message::tabs::CreateTabRequest,
    prelude::*,
    state::{
        tabs::ActiveTabsState,
        terminal::TerminalSizeState,
        workspace::{WorkspaceState, WorkspaceTab},
    },
    utils::await_state,
};
use std::collections::HashMap;
use std::path::PathBuf;
use tab_api::tab::{normalize_name, CreateTabMetadata};
use tokio::sync::watch;

/// Receives CreateTabRequests, and decides whether to send the daemon issue a create request.
/// Assembles the CreateTabMetadata.
pub struct CreateTabService {
    _request_tab: Lifeline,
}

impl Service for CreateTabService {
    type Bus = TabBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut rx = bus.rx::<CreateTabRequest>()?;
        let mut rx_tabs_state = bus.rx::<Option<ActiveTabsState>>()?.into_inner();
        let rx_terminal_size = bus.rx::<TerminalSizeState>()?.into_inner();
        let mut rx_workspace = bus.rx::<Option<WorkspaceState>>()?.into_inner();
        let mut tx_websocket = bus.tx::<Request>()?;

        let _request_tab = Self::try_task("request_tab", async move {
            while let Some(request) = rx.recv().await {
                match request {
                    CreateTabRequest::Named(name) => {
                        let tab_exists = await_state(&mut rx_tabs_state)
                            .await?
                            .tabs
                            .values()
                            .find(|tab| tab.name == name)
                            .is_some();

                        if !tab_exists {
                            let workspace = await_state(&mut rx_workspace).await?;

                            Self::create_named(
                                name,
                                workspace.tabs,
                                &rx_terminal_size,
                                &mut tx_websocket,
                            )
                            .await?;
                        }
                    }
                }
            }

            Ok(())
        });

        Ok(Self { _request_tab })
    }
}

impl CreateTabService {
    pub async fn create_named(
        name: String,
        workspace: Vec<WorkspaceTab>,
        rx_terminal_size: &watch::Receiver<TerminalSizeState>,
        tx_websocket: &mut impl Sender<Request>,
    ) -> anyhow::Result<()> {
        let name = normalize_name(name.as_str());
        let workspace_tab = workspace.into_iter().find(|tab| tab.name == name);

        let dimensions = rx_terminal_size.borrow().0.clone();
        let shell = Self::compute_shell(&workspace_tab);
        let directory = Self::compute_directory(&workspace_tab)?;
        let env = Self::compute_env(&workspace_tab);

        let metadata = CreateTabMetadata {
            name: Self::compute_name(&workspace_tab, name.as_str()),
            dir: directory.to_string_lossy().to_string(),
            env,
            dimensions,
            shell,
        };

        let request = Request::CreateTab(metadata);
        tx_websocket.send(request).await?;

        Ok(())
    }

    fn compute_shell(tab: &Option<WorkspaceTab>) -> String {
        if let Some(ref tab) = tab {
            if let Some(ref shell) = tab.shell {
                return shell.clone();
            }
        }

        std::env::var("SHELL").unwrap_or("/usr/bin/env bash".to_string())
    }

    fn compute_env(tab: &Option<WorkspaceTab>) -> HashMap<String, String> {
        tab.as_ref()
            .map(|tab| tab.env.clone())
            .flatten()
            .unwrap_or(HashMap::with_capacity(0))
    }

    fn compute_directory(tab: &Option<WorkspaceTab>) -> anyhow::Result<PathBuf> {
        if let Some(ref tab) = tab {
            if tab.directory.exists() {
                return Ok(tab.directory.clone());
            } else {
                warn!(
                    "The configured working directory for '{}' was not found: {}",
                    tab.name,
                    tab.directory.as_path().to_string_lossy()
                );
                // fall through to current directory
            }
        }

        std::env::current_dir().map_err(|err| err.into())
    }

    fn compute_name(tab: &Option<WorkspaceTab>, name: &str) -> String {
        if let Some(ref tab) = tab {
            tab.name.clone()
        } else {
            name.to_string()
        }
    }
}
