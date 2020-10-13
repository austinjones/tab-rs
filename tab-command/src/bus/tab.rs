use std::time::Duration;

use crate::{
    message::{
        client::TabTerminated,
        fuzzy::FuzzyRecv,
        main::MainShutdown,
        tabs::{CreateTabRequest, TabRecv, TabShutdown, TabsRecv},
    },
    prelude::*,
    state::{
        tab::{SelectOrRetaskTab, SelectTab, TabState},
        tabs::ActiveTabsState,
        terminal::TerminalSizeState,
        workspace::WorkspaceState,
    },
};
use anyhow::Context;

use tab_api::tab::TabMetadata;
use tokio::{
    sync::{broadcast, mpsc, watch},
    time,
};

lifeline_bus!(pub struct TabBus);

impl Message<TabBus> for Request {
    type Channel = mpsc::Sender<Self>;
}

impl Message<TabBus> for SelectTab {
    type Channel = mpsc::Sender<Self>;
}

impl Message<TabBus> for SelectOrRetaskTab {
    type Channel = mpsc::Sender<Self>;
}

impl Message<TabBus> for TabState {
    type Channel = watch::Sender<Self>;
}

impl Message<TabBus> for TabMetadata {
    type Channel = broadcast::Sender<Self>;
}

impl Message<TabBus> for TabTerminated {
    type Channel = mpsc::Sender<Self>;
}

impl Message<TabBus> for TerminalSizeState {
    type Channel = watch::Sender<Self>;
}

impl Message<TabBus> for TabsRecv {
    type Channel = mpsc::Sender<Self>;
}

impl Message<TabBus> for TabShutdown {
    type Channel = mpsc::Sender<Self>;
}

impl Message<TabBus> for Option<ActiveTabsState> {
    type Channel = watch::Sender<Self>;
}

impl Message<TabBus> for CreateTabRequest {
    type Channel = mpsc::Sender<Self>;
}

impl Message<TabBus> for Option<WorkspaceState> {
    type Channel = watch::Sender<Self>;
}

impl Message<TabBus> for FuzzyRecv {
    type Channel = mpsc::Sender<Self>;
}

/// Carries messages between the MainBus, and the TabBus
///
/// Forwards Request messages, propagates shutdowns, and translates Response messages.
/// Forwards TabState.
pub struct MainTabCarrier {
    pub(super) _tx_selected: Lifeline,
    pub(super) _forward_recv: Lifeline,
    pub(super) _forward_request: Lifeline,
    pub(super) _forward_shutdown: Lifeline,
    pub(super) _forward_active_tabs: Lifeline,
    pub(super) _forward_workspace: Lifeline,
    // pub(super) _create_tab: Lifeline,
    pub(super) _rx_response: Lifeline,
}

impl CarryFrom<MainBus> for TabBus {
    type Lifeline = anyhow::Result<MainTabCarrier>;

    fn carry_from(&self, from: &MainBus) -> Self::Lifeline {
        let _forward_recv = {
            let mut rx_tab = from.rx::<TabRecv>()?;
            let mut tx_create = self.tx::<CreateTabRequest>()?;
            let mut tx_select = self.tx::<SelectOrRetaskTab>()?;
            Self::try_task("forward_create", async move {
                while let Some(msg) = rx_tab.recv().await {
                    match msg {
                        TabRecv::CreateTab(name) => {
                            tx_create.send(CreateTabRequest::Named(name)).await?;
                        }
                        TabRecv::SelectNamedTab { name, env_tab } => {
                            tx_select.send(SelectOrRetaskTab { name, env_tab }).await?;
                        }
                        _ => {}
                    }
                }

                Ok(())
            })
        };

        let _forward_request = {
            let mut rx_request = self.rx::<Request>()?;
            let mut tx_request = from.tx::<Request>()?;

            Self::try_task("forward_request", async move {
                while let Some(request) = rx_request.recv().await {
                    tx_request.send(request).await.context("tx Request")?;
                }

                Ok(())
            })
        };

        let _forward_shutdown = {
            let mut rx = self.rx::<TabShutdown>()?;
            let mut tx = from.tx::<MainShutdown>()?;
            Self::try_task("forward_shutdown", async move {
                rx.recv().await;
                tx.send(MainShutdown {}).await.ok();
                Ok(())
            })
        };

        let _forward_active_tabs = {
            let mut rx = self.rx::<Option<ActiveTabsState>>()?;
            let mut tx = from.tx::<Option<ActiveTabsState>>()?;

            Self::task("forward_tabs_state", async move {
                while let Some(msg) = rx.recv().await {
                    tx.send(msg).await.ok();
                }
            })
        };

        let _forward_workspace = {
            let mut rx = self.rx::<Option<WorkspaceState>>()?;
            let mut tx = from.tx::<Option<WorkspaceState>>()?;

            Self::task("forward_workspace_state", async move {
                while let Some(msg) = rx.recv().await {
                    tx.send(msg).await.ok();
                }
            })
        };

        let _rx_response = {
            let rx_tab_state = self.rx::<TabState>()?.into_inner();
            let mut rx_response = from.rx::<Response>()?;

            let mut tx_tabs = self.tx::<TabsRecv>()?;
            let mut tx_tab_metadata = self.tx::<TabMetadata>()?;
            let mut tx_tab_terminated = self.tx::<TabTerminated>()?;
            let mut tx_select_tab = self.tx::<SelectTab>()?;

            let mut tx_shutdown = from.tx::<MainShutdown>()?;

            Self::try_task("rx_response", async move {
                while let Some(response) = rx_response.recv().await {
                    match response {
                        Response::Init(init) => {
                            tx_tabs
                                .send(TabsRecv::Init(init.tabs.clone()))
                                .await
                                .context("tx TabsRecv::Init")?;
                        }
                        Response::TabUpdate(tab) => {
                            tx_tab_metadata
                                .send(tab.clone())
                                .await
                                .context("send TabMetadata")?;

                            tx_tabs
                                .send(TabsRecv::Update(tab))
                                .await
                                .context("tx TabsRecv::Update")?;
                        }
                        Response::TabTerminated(id) => {
                            tx_tabs.send(TabsRecv::Terminated(id)).await?;

                            tx_tab_terminated.send(TabTerminated(id)).await?;
                            if rx_tab_state.borrow().is_selected(&id) {
                                // wait just a few moments for messages to settle.
                                // if we terminate immediately, there could be terminal I/O going on.
                                // example:
                                //   05:39:38 [ERROR] ERR: TerminalEchoService/stdout: task was cancelled
                                time::delay_for(Duration::from_millis(25)).await;

                                tx_shutdown
                                    .send(MainShutdown {})
                                    .await
                                    .context("tx MainShutdown")?;
                            }
                        }
                        Response::Retask(to_id) => {
                            let state = SelectTab::Tab(to_id);
                            tx_select_tab.send(state).await?;
                        }
                        _ => {}
                    }
                }

                tx_shutdown.send(MainShutdown {}).await?;

                Ok(())
            })
        };

        let _tx_selected = {
            let mut rx_tab_state = self.rx::<TabState>()?;
            let mut tx_tab_state = from.tx::<TabState>()?;

            Self::try_task("tx_selected", async move {
                while let Some(tab) = rx_tab_state.recv().await {
                    tx_tab_state.send(tab).await?;
                }

                Ok(())
            })
        };

        Ok(MainTabCarrier {
            _tx_selected,
            _forward_recv,
            _forward_request,
            _forward_shutdown,
            _forward_active_tabs,
            _forward_workspace,
            // _create_tab,
            _rx_response,
        })
    }
}

impl TabBus {}
