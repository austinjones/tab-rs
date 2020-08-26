use crate::{
    message::{
        client::TabTerminated,
        main::{MainRecv, MainShutdown},
        tabs::{TabShutdown, TabsRecv},
    },
    prelude::*,
    state::{
        tab::{TabState, TabStateAvailable, TabStateSelect},
        tabs::TabsState,
        terminal::TerminalSizeState,
    },
};
use anyhow::Context;
use std::collections::HashMap;
use tab_api::tab::{CreateTabMetadata, TabId, TabMetadata};
use tokio::{
    stream::StreamExt,
    sync::{broadcast, mpsc, watch},
};

lifeline_bus!(pub struct TabBus);

impl Message<TabBus> for Request {
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

impl Message<TabBus> for TabStateSelect {
    type Channel = watch::Sender<Self>;
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

pub struct MainTabCarrier {
    pub(super) _main: Lifeline,
    pub(super) _tx_selected: Lifeline,
    pub(super) _forward_request: Lifeline,
    pub(super) _forward_shutdown: Lifeline,

    pub(super) _create_tab: Lifeline,
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

        let _create_tab = {
            let mut rx_tab_state = self.rx::<TabState>()?;
            let rx_terminal_size = self.rx::<TerminalSizeState>()?.into_inner();
            let mut tx_request = from.tx::<Request>()?;

            Self::try_task("request_tab", async move {
                while let Some(update) = rx_tab_state.recv().await {
                    if let TabState::Awaiting(name) = update {
                        let terminal_size = rx_terminal_size.borrow().clone();
                        let dimensions = terminal_size.0;
                        tx_request
                            .send(Request::CreateTab(CreateTabMetadata { name, dimensions }))
                            .await
                            .context("tx Request::CreateTab")?;
                    }
                }

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
            let mut rx_tabs_state = self.rx::<TabsState>()?;
            let mut rx_main = from.rx::<MainRecv>()?;
            let mut tx_shutdown = from.tx::<MainShutdown>()?;
            let mut tx_select_tab = self.tx::<TabStateSelect>()?;

            Self::try_task("main_recv", async move {
                while let Some(msg) = rx_main.recv().await {
                    match msg {
                        MainRecv::SelectTab(name) => {
                            tx_select_tab
                                .send(TabStateSelect::Selected(name))
                                .await
                                .context("send TabStateSelect")?;
                        }
                        MainRecv::ListTabs => {
                            while let Some(state) = rx_tabs_state.recv().await {
                                if !state.initialized {
                                    continue;
                                }

                                Self::echo_tabs(&state.tabs);

                                tx_shutdown.send(MainShutdown {}).await?;
                            }
                        }
                        MainRecv::AutocompleteTab(complete) => {
                            while let Some(state) = rx_tabs_state.recv().await {
                                if !state.initialized {
                                    continue;
                                }

                                Self::echo_completion(&state.tabs, complete.as_str());

                                tx_shutdown.send(MainShutdown {}).await?;
                            }
                        }
                        _ => {}
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
            _create_tab,
            _rx_response,
        })
    }
}

impl TabBus {
    fn echo_tabs(tabs: &HashMap<TabId, TabMetadata>) {
        debug!("echo tabs: {:?}", tabs);

        let mut names: Vec<&str> = tabs.values().map(|v| v.name.as_str()).collect();
        names.sort();

        if names.len() == 0 {
            println!("No active tabs.");
            return;
        }

        println!("Available tabs:");
        for name in names {
            println!("\t{}", name);
        }
    }

    fn echo_completion(tabs: &HashMap<TabId, TabMetadata>, completion: &str) {
        debug!("echo completion: {:?}, {}", tabs, completion);

        let mut names: Vec<&str> = tabs
            .values()
            .map(|v| v.name.as_str())
            .filter(|name| name.starts_with(completion))
            .collect();

        names.sort_by(|a, b| {
            let a_len = Self::overlap(a, completion);
            let b_len = Self::overlap(b, completion);

            a_len.cmp(&b_len)
        });

        for name in names {
            println!("{}", name);
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
}
