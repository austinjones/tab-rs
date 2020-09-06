use crate::prelude::*;
use crate::{
    message::{client::TabTerminated, tabs::TabShutdown},
    state::tab::{SelectTab, TabState},
};

use anyhow::Context;

use std::collections::HashMap;
use tab_api::tab::{TabId, TabMetadata};
use tokio::stream::StreamExt;

/// Tracks the current tab state, and updates TabState.
pub struct TabStateService {
    _lifeline: Lifeline,
}

enum Event {
    Select(SelectTab),
    Metadata(TabMetadata),
    Terminated(TabId),
}

impl Service for TabStateService {
    type Bus = TabBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &TabBus) -> Self::Lifeline {
        let rx_select = bus.rx::<SelectTab>()?;
        let rx_tab_metadata = bus
            .rx::<TabMetadata>()?
            .into_inner()
            .filter(|r| r.is_ok())
            .map(|r| r.unwrap());
        let rx_tab_terminated = bus.rx::<TabTerminated>()?;

        let mut tx = bus.tx::<TabState>()?;
        let mut tx_websocket = bus.tx::<Request>()?;
        let mut tx_shutdown = bus.tx::<TabShutdown>()?;

        let _lifeline = Self::try_task("run", async move {
            let mut state = TabState::None;

            let mut events = {
                let tabs = rx_select.map(|elem| Event::Select(elem));
                let tab_metadatas = rx_tab_metadata.map(|elem| Event::Metadata(elem));
                let tab_terminated = rx_tab_terminated.map(|elem| Event::Terminated(elem.0));
                tabs.merge(tab_metadatas).merge(tab_terminated)
            };

            let mut tabs: HashMap<String, TabId> = HashMap::new();

            while let Some(event) = events.next().await {
                match event {
                    Event::Select(select) => match select {
                        SelectTab::NamedTab(name) => {
                            if state.is_awaiting(name.as_str()) {
                                continue;
                            }

                            state = if let Some(id) = tabs.get(&name.to_string()) {
                                info!("selected tab {}", name);

                                if let TabState::Selected(id) = state {
                                    tx_websocket.send(Request::Unsubscribe(id)).await?;
                                }

                                tx_websocket.send(Request::Subscribe(*id)).await?;
                                TabState::Selected(*id)
                            } else {
                                info!("awaiting tab {}", name);
                                TabState::Awaiting(name.to_string())
                            };

                            tx.send(state.clone()).await?;
                        }
                        SelectTab::Tab(id) => {
                            if state.is_selected(&id) {
                                continue;
                            }

                            if let TabState::Selected(id) = state {
                                tx_websocket.send(Request::Unsubscribe(id)).await?;
                            }

                            tx_websocket.send(Request::Subscribe(id)).await?;
                            state = TabState::Selected(id);

                            tx.send(state.clone()).await?;
                        }
                    },
                    Event::Metadata(metadata) => {
                        if state.is_awaiting(metadata.name.as_str()) {
                            info!("tab active {}", metadata.name.as_str());

                            state = TabState::Selected(metadata.id);
                            tx.send(state.clone()).await?;

                            tx_websocket.send(Request::Subscribe(metadata.id)).await?;
                        }

                        let id = metadata.id;
                        let name = metadata.name;
                        tabs.insert(name, id);
                    }
                    Event::Terminated(terminated_id) => {
                        if let TabState::Selected(selected_id) = state {
                            if terminated_id == selected_id {
                                state = TabState::None;
                                tx.send(state.clone()).await?;
                                tx_shutdown
                                    .send(TabShutdown {})
                                    .await
                                    .context("tx TabShutdown")?;
                            }
                        }

                        let remove: Vec<String> = tabs
                            .iter()
                            .filter(|(_, id)| **id == terminated_id)
                            .map(|(name, _)| name.clone())
                            .collect();

                        for name in remove.into_iter() {
                            tabs.remove(&name);
                        }
                    }
                }
            }

            Ok(())
        });

        Ok(Self { _lifeline })
    }
}
