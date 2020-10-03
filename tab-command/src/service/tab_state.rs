use crate::{
    message::{client::TabTerminated, tabs::TabShutdown},
    state::tab::{SelectTab, TabState},
};
use crate::{prelude::*, state::terminal::TerminalSizeState};

use anyhow::Context;

use std::collections::HashMap;
use tab_api::tab::{TabId, TabMetadata};
use tokio::{stream::StreamExt, sync::watch};

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
        let rx_terminal_size = bus.rx::<TerminalSizeState>()?.into_inner();

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

            let mut tabs: HashMap<String, TabMetadata> = HashMap::new();

            while let Some(event) = events.next().await {
                match event {
                    Event::Select(select) => match select {
                        SelectTab::NamedTab(name) => {
                            if state.is_awaiting(name.as_str()) {
                                continue;
                            }

                            state = if let Some(metadata) = tabs.get(&name.to_string()) {
                                info!("selected tab {}", name);

                                Self::select_tab(
                                    &metadata,
                                    &rx_terminal_size,
                                    &mut tx,
                                    &mut tx_websocket,
                                )
                                .await?
                            } else {
                                info!("awaiting tab {}", name);
                                TabState::Awaiting(name.to_string())
                            };

                            tx.send(state.clone()).await?;
                        }
                        SelectTab::Tab(id) => {
                            if state.is_selected(id) {
                                continue;
                            }

                            state = TabState::AwaitingId(id);
                            tx.send(state.clone()).await?;
                        }
                    },
                    Event::Metadata(metadata) => {
                        if state.is_awaiting_id(metadata.id)
                            || state.is_awaiting(metadata.name.as_str())
                        {
                            info!("tab active {}", metadata.name.as_str());

                            state = Self::select_tab(
                                &metadata,
                                &rx_terminal_size,
                                &mut tx,
                                &mut tx_websocket,
                            )
                            .await?;
                        }

                        let name = metadata.name.clone();
                        tabs.insert(name, metadata);
                    }
                    Event::Terminated(terminated_id) => {
                        if let TabState::Selected(ref tab) = state {
                            if terminated_id == tab.id {
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
                            .filter(|(_, tab)| tab.id == terminated_id)
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

impl TabStateService {
    pub async fn select_tab(
        metadata: &TabMetadata,
        rx_terminal_size: &watch::Receiver<TerminalSizeState>,
        tx_state: &mut impl Sender<TabState>,
        tx_websocket: &mut impl Sender<Request>,
    ) -> anyhow::Result<TabState> {
        let id = metadata.id;

        tx_websocket.send(Request::Subscribe(id)).await?;

        let terminal_size = rx_terminal_size.borrow().clone();
        tx_websocket
            .send(Request::ResizeTab(id, terminal_size.0))
            .await?;

        let state = TabState::Selected(metadata.clone());

        tx_state.send(state.clone()).await?;

        Ok(state)
    }
}
