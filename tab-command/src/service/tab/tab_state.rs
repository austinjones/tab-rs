use crate::state::tab::{DeselectTab, SelectTab, TabState};
use crate::{prelude::*, state::terminal::TerminalSizeState};

use std::collections::HashMap;
use tab_api::tab::{TabId, TabMetadata};
use tokio::{stream::StreamExt, sync::watch};

/// Tracks the current tab state, and updates TabState.
pub struct TabStateService {
    _lifeline: Lifeline,
}

enum Event {
    Select(SelectTab),
    Deselect(DeselectTab),
    Metadata(TabMetadata),
}

impl Service for TabStateService {
    type Bus = TabBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &TabBus) -> Self::Lifeline {
        let rx_select = bus.rx::<SelectTab>()?;
        let rx_deselect = bus.rx::<DeselectTab>()?;
        let rx_tab_metadata = bus
            .rx::<TabMetadata>()?
            .into_inner()
            .filter(|r| r.is_ok())
            .map(|r| r.unwrap());
        let rx_terminal_size = bus.rx::<TerminalSizeState>()?.into_inner();

        let mut tx = bus.tx::<TabState>()?;
        let mut tx_websocket = bus.tx::<Request>()?;

        let _lifeline = Self::try_task("run", async move {
            let mut state = TabState::None;

            let mut events = {
                let tabs = rx_select.map(|elem| Event::Select(elem));
                let tab_metadatas = rx_tab_metadata.map(|elem| Event::Metadata(elem));
                let deselect = rx_deselect.map(|elem| Event::Deselect(elem));
                tabs.merge(tab_metadatas).merge(deselect)
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

                                Self::select_tab(*id, &rx_terminal_size, &mut tx, &mut tx_websocket)
                                    .await?
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
                            state =
                                Self::select_tab(id, &rx_terminal_size, &mut tx, &mut tx_websocket)
                                    .await?;
                        }
                    },
                    Event::Metadata(metadata) => {
                        if state.is_awaiting(metadata.name.as_str()) {
                            info!("tab active {}", metadata.name.as_str());

                            state = Self::select_tab(
                                metadata.id,
                                &rx_terminal_size,
                                &mut tx,
                                &mut tx_websocket,
                            )
                            .await?;
                        }

                        let id = metadata.id;
                        let name = metadata.name;
                        tabs.insert(name, id);
                    }
                    Event::Deselect(_deselect) => {
                        if let TabState::Selected(id) = state {
                            tx_websocket.send(Request::Unsubscribe(id)).await?;
                        }

                        state = TabState::None;
                        tx.send(state.clone()).await?;
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
        id: TabId,
        rx_terminal_size: &watch::Receiver<TerminalSizeState>,
        tx_state: &mut impl Sender<TabState>,
        tx_websocket: &mut impl Sender<Request>,
    ) -> anyhow::Result<TabState> {
        tx_websocket.send(Request::Subscribe(id)).await?;

        let terminal_size = rx_terminal_size.borrow().clone();
        tx_websocket
            .send(Request::ResizeTab(id, terminal_size.0))
            .await?;

        let state = TabState::Selected(id);

        tx_state.send(state.clone()).await?;

        Ok(state)
    }
}
