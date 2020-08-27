use crate::{
    message::{
        client::TabTerminated,
        main::{MainRecv, MainShutdown},
        tabs::{CreateTabRequest, TabShutdown, TabsRecv},
    },
    normalize_name,
    prelude::*,
    state::{
        tab::{SelectTab, TabState, TabStateAvailable},
        tabs::TabsState,
        terminal::TerminalSizeState,
        workspace::{WorkspaceState, WorkspaceTab},
    },
};
use anyhow::Context;

use tab_api::tab::{TabId, TabMetadata};
use tokio::sync::{broadcast, mpsc, watch};

lifeline_bus!(pub struct TabBus);

impl Message<TabBus> for Request {
    type Channel = mpsc::Sender<Self>;
}

impl Message<TabBus> for SelectTab {
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

impl Message<TabBus> for TabStateAvailable {
    type Channel = watch::Sender<Self>;
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

impl Message<TabBus> for TabsState {
    type Channel = watch::Sender<Self>;
}

impl Message<TabBus> for CreateTabRequest {
    type Channel = mpsc::Sender<Self>;
}

impl Message<TabBus> for WorkspaceState {
    type Channel = watch::Sender<Self>;
}

pub struct MainTabCarrier {
    pub(super) _main: Lifeline,
    pub(super) _tx_selected: Lifeline,
    pub(super) _forward_request: Lifeline,
    pub(super) _forward_shutdown: Lifeline,

    // pub(super) _create_tab: Lifeline,
    pub(super) _rx_response: Lifeline,
}

impl CarryFrom<MainBus> for TabBus {
    type Lifeline = anyhow::Result<MainTabCarrier>;

    fn carry_from(&self, from: &MainBus) -> Self::Lifeline {
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

        let _rx_response = {
            let rx_tab_state = self.rx::<TabState>()?.into_inner();
            let mut rx_response = from.rx::<Response>()?;

            let mut tx_tabs = self.tx::<TabsRecv>()?;
            let mut tx_tab_metadata = self.tx::<TabMetadata>()?;
            let mut tx_available_tabs = self.tx::<TabStateAvailable>()?;
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
                        Response::TabList(tabs) => tx_available_tabs
                            .send(TabStateAvailable(tabs))
                            .await
                            .context("tx TabStateAvailable")?,
                        Response::TabTerminated(id) => {
                            tx_tabs.send(TabsRecv::Terminated(id)).await?;

                            tx_tab_terminated.send(TabTerminated(id)).await?;
                            if rx_tab_state.borrow().is_selected(&id) {
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

        let _main = {
            let mut rx_tabs_state = self.rx::<TabsState>()?.into_inner();
            let mut rx_workspace = self.rx::<WorkspaceState>()?.into_inner();
            let mut rx_main = from.rx::<MainRecv>()?;
            let mut tx_shutdown = from.tx::<MainShutdown>()?;
            let mut tx_create = self.tx::<CreateTabRequest>()?;
            let mut tx_select = self.tx::<SelectTab>()?;
            let mut tx_websocket = self.tx::<Request>()?;

            Self::try_task("main_recv", async move {
                while let Some(msg) = rx_main.recv().await {
                    match msg {
                        MainRecv::SelectTab(name) => {
                            let name = normalize_name(name.as_str());
                            if let Ok(id) = std::env::var("TAB_ID") {
                                if let Ok(id) = id.parse() {
                                    if let Ok(env_name) = std::env::var("TAB") {
                                        // we don't need any change.  we can ignore it.
                                        if name.trim() == env_name.trim() {
                                            tx_shutdown.send(MainShutdown {}).await?;
                                            continue;
                                        }
                                    }

                                    info!("retasking tab {} with new selection {}.", id, &name);
                                    let id = TabId(id);

                                    Self::await_initialized(&mut rx_tabs_state).await;

                                    tx_create
                                        .send(CreateTabRequest::Named(name.clone()))
                                        .await?;

                                    debug!("retask - waiting for creation on tab {}", id);
                                    let metadata =
                                        Self::await_created(name, &mut rx_tabs_state).await;

                                    debug!("retask - sending retask to tab {}", id);
                                    let request = Request::Retask(id, metadata.id);
                                    tx_websocket.send(request).await?;
                                    tx_shutdown.send(MainShutdown {}).await?;
                                    continue;
                                }
                            }

                            tx_create
                                .send(CreateTabRequest::Named(name.clone()))
                                .await?;

                            tx_select
                                .send(SelectTab::NamedTab(name))
                                .await
                                .context("send TabStateSelect")?;
                        }
                        MainRecv::ListTabs => {
                            let running_tabs = Self::await_initialized(&mut rx_tabs_state).await;
                            let workspace_tabs = Self::await_workspace(&mut rx_workspace).await;
                            let tabs = Self::merge_tabs(running_tabs, workspace_tabs);

                            Self::echo_tabs(&tabs);
                            tx_shutdown.send(MainShutdown {}).await?;
                        }
                        MainRecv::AutocompleteTab => {
                            // the the list of available tabs, both running (ad-hoc), and from the workspace library
                            let running_tabs = Self::await_initialized(&mut rx_tabs_state).await;
                            let workspace_tabs = Self::await_workspace(&mut rx_workspace).await;
                            let tabs = Self::merge_tabs(running_tabs, workspace_tabs);
                            let tabs = tabs.into_iter().map(|(name, _doc)| name).collect();
                            Self::echo_completion(&tabs);
                            tx_shutdown.send(MainShutdown {}).await?;
                        }
                        MainRecv::AutocompleteCloseTab => {
                            // get the list of tabs which are running on the daemon.
                            let running_tabs = Self::await_initialized(&mut rx_tabs_state).await;

                            let mut tabs = Vec::new();
                            if let Some(running_tabs) = running_tabs {
                                for (_id, metadata) in running_tabs.tabs {
                                    tabs.push(metadata.name);
                                }
                            }

                            tabs.sort();
                            tabs.dedup();

                            Self::echo_completion(&tabs);
                            tx_shutdown.send(MainShutdown {}).await?;
                        }

                        MainRecv::GlobalShutdown => {}
                        MainRecv::SelectInteractive => {}
                        MainRecv::CloseTab(_) => {}
                    }
                }

                Ok(())
            })
        };

        Ok(MainTabCarrier {
            _main,
            _tx_selected,
            _forward_request,
            _forward_shutdown,
            // _create_tab,
            _rx_response,
        })
    }
}

impl TabBus {
    fn merge_tabs(
        running: Option<TabsState>,
        workspace: Option<Vec<WorkspaceTab>>,
    ) -> Vec<(String, String)> {
        let mut tabs = Vec::new();
        if let Some(running) = running {
            for (_id, metadata) in running.tabs {
                tabs.push((metadata.name, "".to_string()));
            }
        }

        if let Some(workspace) = workspace {
            for tab in workspace.into_iter() {
                tabs.push((tab.name, tab.doc));
            }
        }

        tabs.sort();
        tabs.dedup_by(|(n, _), (n2, _)| n == n2);

        tabs
    }

    fn echo_tabs(tabs: &Vec<(String, String)>) {
        debug!("echo tabs: {:?}", tabs);

        if tabs.len() == 0 {
            println!("No active tabs.");
            return;
        }

        let len = tabs.iter().map(|(name, _doc)| name.len()).max().unwrap();
        let target_len = len + 4;
        println!("Available tabs:");
        for (name, doc) in tabs.iter() {
            print!("    {}", name);
            if doc.len() > 0 {
                for _ in name.len()..target_len {
                    print!(" ");
                }
                println!("({})", doc);
            } else {
                println!("");
            }
        }
    }

    fn echo_completion(tabs: &Vec<String>) {
        debug!("echo completion: {:?}", tabs);

        for tab in tabs {
            println!("{}", tab);
        }
    }

    fn overlap(value: &str, target: &str) -> usize {
        let mut matches = 0;
        for (a, b) in value.chars().zip(target.chars()) {
            if a != b {
                return matches;
            }

            matches += 1;
        }

        return matches;
    }

    async fn await_initialized(rx: &mut watch::Receiver<TabsState>) -> Option<TabsState> {
        {
            let borrow = rx.borrow();
            if borrow.initialized {
                return Some(borrow.clone());
            }
        }

        let mut state = rx.recv().await;
        // TODO: 2 second timeout?

        while state.is_some() && !state.as_ref().unwrap().initialized {
            state = rx.recv().await;
        }

        state
    }

    async fn await_workspace(
        rx: &mut watch::Receiver<WorkspaceState>,
    ) -> Option<Vec<WorkspaceTab>> {
        {
            let borrow = rx.borrow();
            if let WorkspaceState::Ready(ref ready) = *borrow {
                return Some(ready.clone());
            }
        }
        let mut state = rx.recv().await;
        // TODO: 2 second timeout?

        while state.is_some() {
            if let Some(WorkspaceState::Ready(ready)) = state {
                return Some(ready);
            };

            state = rx.recv().await;
        }

        None
    }
    async fn await_created(name: String, rx: &mut watch::Receiver<TabsState>) -> TabMetadata {
        {
            let borrow = rx.borrow();
            let existing = borrow.tabs.values().find(|tab| tab.name == name);
            if let Some(metadata) = existing {
                return metadata.clone();
            }
        }

        // TODO: 2 second timeout?
        loop {
            let state = rx.recv().await;

            if !state.is_some() {
                continue;
            }

            let state = state.unwrap();

            if !state.initialized {
                continue;
            }

            for (_id, metadata) in state.tabs {
                if metadata.name == name {
                    return metadata;
                }
            }
        }
    }
}
