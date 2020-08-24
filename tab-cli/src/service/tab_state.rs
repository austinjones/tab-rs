use crate::prelude::*;
use crate::{
    message::{client::TabTerminated, tabs::TabShutdown},
    state::tab::{TabState, TabStateSelect},
};

use anyhow::Context;
use lifeline::Task;
use lifeline::{Bus, Lifeline, Service};
use log::{debug, info};
use std::collections::HashMap;
use tab_api::{
    request::Request,
    tab::{TabId, TabMetadata},
};
use tokio::stream::StreamExt;
pub struct TabStateService {
    _lifeline: Lifeline,
}

enum Event {
    Select(TabStateSelect),
    Metadata(TabMetadata),
    Terminated(TabId),
}

impl Service for TabStateService {
    type Bus = TabBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &TabBus) -> Self::Lifeline {
        let rx_tab = bus.rx::<TabStateSelect>()?;
        let rx_tab_metadata = bus.rx::<TabMetadata>()?;
        let rx_tab_terminated = bus.rx::<TabTerminated>()?;

        let tx = bus.tx::<TabState>()?;
        let mut tx_websocket = bus.tx::<Request>()?;
        let mut tx_shutdown = bus.tx::<TabShutdown>()?;

        let _lifeline = Self::try_task("run", async move {
            let mut state = TabState::None;

            let mut events = {
                let tabs = rx_tab.map(|elem| Event::Select(elem));
                let tab_metadatas = rx_tab_metadata.map(|elem| Event::Metadata(elem.unwrap()));
                let tab_terminated = rx_tab_terminated.map(|elem| Event::Terminated(elem.0));
                tabs.merge(tab_metadatas).merge(tab_terminated)
            };

            let mut tabs: HashMap<String, TabId> = HashMap::new();

            while let Some(event) = events.next().await {
                match event {
                    Event::Select(select) => match select {
                        TabStateSelect::None => {}
                        TabStateSelect::Selected(name) => {
                            let name = name.as_str();

                            if state.is_selected_name(name) || state.is_awaiting(name) {
                                continue;
                            }

                            if let TabState::Selected(id, _meta) = state {
                                tx_websocket.send(Request::Unsubscribe(id)).await?;
                            }

                            state = if let Some(id) = tabs.get(&name.to_string()) {
                                info!("selected tab {}", name);
                                tx_websocket.send(Request::Subscribe(*id)).await?;
                                TabState::Selected(*id, name.to_string())
                            } else {
                                info!("awaiting tab {}", name);
                                TabState::Awaiting(name.to_string())
                            };

                            tx.broadcast(state.clone())?;
                        }
                    },
                    Event::Metadata(metadata) => {
                        debug!("got tab metadata: {:?}", &metadata);
                        if state.is_awaiting(metadata.name.as_str()) {
                            info!("tab active {}", metadata.name.as_str());

                            state = TabState::Selected(metadata.id, metadata.name.clone());
                            tx.broadcast(state.clone())?;

                            tx_websocket.send(Request::Subscribe(metadata.id)).await?;
                        }

                        let id = metadata.id;
                        let name = metadata.name;
                        tabs.insert(name, id);
                    }
                    Event::Terminated(terminated_id) => {
                        if let TabState::Selected(selected_id, ref _name) = state {
                            if terminated_id == selected_id {
                                state = TabState::None;
                                tx.broadcast(state.clone())?;
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
