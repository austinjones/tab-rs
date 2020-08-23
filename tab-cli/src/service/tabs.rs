use crate::{bus::MainBus, message::tabs::TabsRecv, state::tabs::TabsState};
use log::info;
use std::collections::HashMap;
use tab_service::{Bus, Lifeline, Service};
pub struct TabsStateService {
    _run: Lifeline,
}

impl Service for TabsStateService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> anyhow::Result<Self> {
        let mut rx = bus.rx::<TabsRecv>()?;
        let tx = bus.tx::<TabsState>()?;
        let _run = Self::try_task("run", async move {
            let mut state = HashMap::new();

            while let Some(recv) = rx.recv().await {
                info!("{:?}", recv);
                match recv {
                    TabsRecv::Init(tabs) => {
                        state.extend(tabs.into_iter());
                    }
                    TabsRecv::Update(metadata) => {
                        state.insert(metadata.id, metadata);
                    }
                    TabsRecv::Terminated(id) => {
                        state.remove(&id);
                    }
                }

                tx.broadcast(TabsState {
                    initialized: true,
                    tabs: state.clone(),
                })?;
            }

            Ok(())
        });

        Ok(Self { _run })
    }
}
