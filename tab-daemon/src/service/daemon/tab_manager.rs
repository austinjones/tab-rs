use crate::prelude::*;
use crate::{
    message::{
        tab::TabRecv,
        tab_manager::{TabManagerRecv, TabManagerSend},
    },
    state::tab::{TabAssignment, TabsState},
};
use lifeline::Task;
use lifeline::{Bus, Lifeline, Service};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};
use tab_api::tab::{TabId, TabMetadata};
use tokio::sync::{broadcast, mpsc, watch};

pub struct TabManagerService {
    _recv: Lifeline,
}

static TAB_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

impl Service for TabManagerService {
    type Bus = ListenerBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut rx = bus.rx::<TabManagerRecv>()?;
        let mut tx = bus.tx::<TabManagerSend>()?;
        let mut tx_tabs = bus.tx::<TabRecv>()?;

        let mut tx_tabs_state = bus.tx::<TabsState>()?;
        let mut tabs: HashMap<TabId, TabMetadata> = HashMap::new();

        let _recv = Self::try_task("recv", async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    TabManagerRecv::CreateTab(create) => {
                        let id = TAB_ID_COUNTER.fetch_add(1, Ordering::SeqCst) as u16;
                        let tab_id = TabId(id);
                        let tab_metadata = TabMetadata::create(tab_id, create);

                        let assignment = TabAssignment::new(tab_metadata.clone());
                        let message = TabRecv::Assign(assignment);
                        tx_tabs.send(message).map_err(into_msg)?;

                        tabs.insert(tab_id, tab_metadata);
                        tx_tabs_state.broadcast(TabsState::new(&tabs))?;
                    }
                    TabManagerRecv::CloseNamedTab(name) => {
                        let close_tab = tabs.values().find(|t| t.name == name);
                        if let Some(tab) = close_tab {
                            Self::close_tab(
                                tab.id,
                                &mut tabs,
                                &mut tx,
                                &mut tx_tabs,
                                &mut tx_tabs_state,
                            )?;
                        }
                    }
                    TabManagerRecv::CloseTab(close) => {
                        Self::close_tab(
                            close,
                            &mut tabs,
                            &mut tx,
                            &mut tx_tabs,
                            &mut tx_tabs_state,
                        )?;
                    }
                }
            }
            Ok(())
        });

        Ok(Self { _recv })
    }
}

impl TabManagerService {
    fn close_tab(
        id: TabId,
        tabs: &mut HashMap<TabId, TabMetadata>,
        tx: &mut mpsc::Sender<TabManagerSend>,
        tx_close: &mut broadcast::Sender<TabRecv>,
        tx_tabs_state: &mut watch::Sender<TabsState>,
    ) -> anyhow::Result<()> {
        tabs.remove(&id);

        tx.send(TabManagerSend::TabTerminated(id));
        tx_close.send(TabRecv::Terminate(id));
        tx_tabs_state.broadcast(TabsState::new(&tabs))?;

        Ok(())
    }
}
