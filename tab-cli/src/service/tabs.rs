use crate::prelude::*;
use crate::{message::tabs::TabsRecv, state::tabs::TabsState};

use std::collections::HashMap;
use tab_api::tab::TabMetadata;
pub struct TabsStateService {
    _run: Lifeline,
}

impl Service for TabsStateService {
    type Bus = TabBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> anyhow::Result<Self> {
        let mut rx = bus.rx::<TabsRecv>()?;
        let mut tx = bus.tx::<TabsState>()?;
        let mut tx_metadata = bus.tx::<TabMetadata>()?;
        let _run = Self::try_task("run", async move {
            let mut state = HashMap::new();

            while let Some(recv) = rx.recv().await {
                info!("{:?}", recv);
                match recv {
                    TabsRecv::Init(tabs) => {
                        for metadata in tabs.values() {
                            tx_metadata.send(metadata.clone()).await?;
                        }

                        state.extend(tabs.into_iter());
                    }
                    TabsRecv::Update(metadata) => {
                        state.insert(metadata.id, metadata.clone());
                        tx_metadata.send(metadata.clone()).await?;
                    }
                    TabsRecv::Terminated(id) => {
                        state.remove(&id);
                    }
                }

                tx.send(TabsState {
                    initialized: true,
                    tabs: state.clone(),
                })
                .await?;
            }

            Ok(())
        });

        Ok(Self { _run })
    }
}
