use crate::normalize_name;
use crate::{
    message::tabs::CreateTabRequest,
    prelude::*,
    state::{
        tabs::TabsState,
        terminal::TerminalSizeState,
        workspace::{WorkspaceState, WorkspaceTab},
    },
};
use tab_api::tab::CreateTabMetadata;
use time::Duration;
use tokio::{sync::watch, time};
pub struct CreateTabService {
    _request_tab: Lifeline,
}

impl Service for CreateTabService {
    type Bus = TabBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut rx = bus.rx::<CreateTabRequest>()?;
        let rx_tabs_state = bus.rx::<TabsState>()?.into_inner();
        let rx_terminal_size = bus.rx::<TerminalSizeState>()?.into_inner();
        let rx_workspace = bus.rx::<WorkspaceState>()?.into_inner();
        let mut tx_websocket = bus.tx::<Request>()?;

        let _request_tab = Self::try_task("request_tab", async move {
            while let Some(request) = rx.recv().await {
                match request {
                    CreateTabRequest::Named(name) => {
                        let tab_exists = rx_tabs_state
                            .borrow()
                            .tabs
                            .values()
                            .find(|tab| tab.name == name)
                            .is_some();

                        if !tab_exists {
                            let workspace = Self::await_workspace(&rx_workspace).await;

                            Self::create_named(
                                name,
                                workspace,
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

        // let _create_tab = {
        //     let mut rx_tab_state = self.rx::<TabState>()?;
        //     let rx_terminal_size = self.rx::<TerminalSizeState>()?.into_inner();
        //     let mut tx_request = from.tx::<Request>()?;

        //     let shell = std::env::var("SHELL").unwrap_or("/usr/bin/env bash".to_string());

        //     Self::try_task("request_tab", async move {
        //         while let Some(update) = rx_tab_state.recv().await {
        //             if let TabState::Awaiting(name) = update {
        //                 let terminal_size = rx_terminal_size.borrow().clone();
        //                 let dimensions = terminal_size.0;

        //                 let current_dir = std::env::current_dir()?;
        //                 tx_request
        //                     .send(Request::CreateTab(CreateTabMetadata {
        //                         name,
        //                         dimensions,
        //                         shell: shell.clone(),
        //                         dir: current_dir.to_string_lossy().to_string(),
        //                     }))
        //                     .await
        //                     .context("tx Request::CreateTab")?;
        //             }
        //         }

        //         Ok(())
        //     })
        // };
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
        let tab = workspace.into_iter().find(|tab| tab.name == name);

        let dimensions = rx_terminal_size.borrow().0.clone();
        let shell = std::env::var("SHELL").unwrap_or("/usr/bin/env bash".to_string());
        let metadata = if let Some(tab) = tab {
            CreateTabMetadata {
                name: tab.name,
                dir: tab.directory.to_string_lossy().to_string(),
                dimensions,
                shell,
            }
        } else {
            let current_dir = std::env::current_dir()?;
            CreateTabMetadata {
                name: name.clone(),
                dir: current_dir.to_string_lossy().to_string(),
                dimensions,
                shell,
            }
        };
        // TODO: move this to a helper and unify across all creation points

        let request = Request::CreateTab(metadata);
        tx_websocket.send(request).await?;
        Ok(())
    }

    async fn await_workspace(rx_workspace: &watch::Receiver<WorkspaceState>) -> Vec<WorkspaceTab> {
        loop {
            if let WorkspaceState::Ready(ref state) = *rx_workspace.borrow() {
                return state.clone();
            }

            time::delay_for(Duration::from_millis(25)).await;
        }
    }
}
