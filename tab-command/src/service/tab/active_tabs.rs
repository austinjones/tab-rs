use crate::prelude::*;
use crate::{message::tabs::TabsRecv, state::tabs::ActiveTabsState};

use std::collections::HashMap;
use tab_api::tab::TabMetadata;

/// Tracks all running tabs, and provides TabsState
pub struct ActiveTabsService {
    _run: Lifeline,
}

impl Service for ActiveTabsService {
    type Bus = TabBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> anyhow::Result<Self> {
        let mut rx = bus.rx::<TabsRecv>()?;
        let mut tx = bus.tx::<Option<ActiveTabsState>>()?;
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
                }

                tx.send(Some(ActiveTabsState {
                    tabs: state.clone(),
                }))
                .await?;
            }

            Ok(())
        });

        Ok(Self { _run })
    }
}
