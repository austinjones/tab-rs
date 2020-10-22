use crate::{
    prelude::*,
    state::{tab::TabMetadataState, terminal::TerminalSizeState},
};
use crate::{
    state::{
        tab::{DeselectTab, SelectTab, TabState},
        tabs::ActiveTabsState,
    },
    utils::await_condition,
};

/// Tracks the current tab state, and updates TabState.
pub struct TabStateService {
    _select: Lifeline,
    _select_named: Lifeline,
    _tab_metadata: Lifeline,
    _deselect: Lifeline,
    _publish: Lifeline,
    _websocket: Lifeline,
}

impl Service for TabStateService {
    type Bus = TabBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &TabBus) -> Self::Lifeline {
        // create an internal channel to distribute state updates
        let (tx_internal, mut rx_internal) = tokio::sync::mpsc::channel(16);

        let _select = {
            let mut rx = bus.rx::<SelectTab>()?;
            let rx_state = bus.rx::<TabState>()?.into_inner();
            let mut tx = tx_internal.clone();

            Self::try_task("select", async move {
                while let Some(select) = rx.recv().await {
                    let state = rx_state.borrow().clone();
                    match select {
                        SelectTab::NamedTab(name) => {
                            if state.is_awaiting(name.as_str()) {
                                continue;
                            }

                            tx.send(TabState::Awaiting(name)).await?;
                        }
                        SelectTab::Tab(id) => {
                            if state.is_selected(&id) {
                                continue;
                            }

                            tx.send(TabState::Selected(id)).await?;
                        }
                    }
                }

                Ok(())
            })
        };

        let _select_named = {
            let mut rx = bus.rx::<TabState>()?;
            let mut rx_active = bus.rx::<Option<ActiveTabsState>>()?.into_inner();
            let mut tx = tx_internal.clone();

            Self::try_task("select_named", async move {
                while let Some(state) = rx.recv().await {
                    if let TabState::Awaiting(name) = state {
                        let tabs = await_condition(&mut rx_active, |tabs| {
                            tabs.find_name(name.as_str()).is_some()
                        })
                        .await?;

                        let id = tabs.find_name(name.as_str()).unwrap().id;
                        tx.send(TabState::Selected(id)).await?;
                    }
                }

                Ok(())
            })
        };

        let _tab_metadata = {
            let mut rx = bus.rx::<TabState>()?;
            let mut rx_tabs = bus.rx::<Option<ActiveTabsState>>()?.into_inner();
            let mut tx = bus.tx::<TabMetadataState>()?;

            Self::try_task("deselect", async move {
                while let Some(state) = rx.recv().await {
                    if let TabState::Selected(id) = state {
                        let state =
                            await_condition(&mut rx_tabs, |state| state.get(&id).is_some()).await?;
                        let tab = state.get(&id).unwrap();
                        tx.send(TabMetadataState::Selected(tab.clone())).await?;
                    } else if let TabState::None = state {
                        tx.send(TabMetadataState::None).await?;
                    }
                }

                Ok(())
            })
        };

        let _deselect = {
            let mut rx = bus.rx::<DeselectTab>()?;
            let mut tx = tx_internal.clone();

            Self::try_task("deselect", async move {
                while let Some(_deselect) = rx.recv().await {
                    tx.send(TabState::None).await?;
                }

                Ok(())
            })
        };

        let _publish = {
            let mut tx = bus.tx::<TabState>()?;
            Self::try_task("publish", async move {
                while let Some(state) = rx_internal.recv().await {
                    tx.send(state).await?;
                }

                Ok(())
            })
        };

        let _websocket = {
            let mut rx = bus.rx::<TabState>()?;
            let rx_terminal_size = bus.rx::<TerminalSizeState>()?.into_inner();

            let mut tx_websocket = bus.tx::<Request>()?;

            Self::try_task("websocket", async move {
                let mut last_state = TabState::None;
                while let Some(state) = rx.recv().await {
                    if let TabState::Selected(id) = state {
                        tx_websocket.send(Request::Subscribe(id)).await?;

                        let terminal_size = rx_terminal_size.borrow().clone();
                        tx_websocket
                            .send(Request::ResizeTab(id, terminal_size.0))
                            .await?;
                    } else if let (TabState::Selected(prev_id), &TabState::None) =
                        (last_state, &state)
                    {
                        tx_websocket.send(Request::Unsubscribe(prev_id)).await?;
                    }

                    last_state = state;
                }

                Ok(())
            })
        };

        Ok(Self {
            _select,
            _select_named,
            _tab_metadata,
            _deselect,
            _publish,
            _websocket,
        })
    }
}
