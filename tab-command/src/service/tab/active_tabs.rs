use crate::prelude::*;
use crate::{message::tabs::TabsRecv, state::tabs::ActiveTabsState};

use std::collections::HashMap;

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

        let _run = Self::try_task("run", async move {
            let mut state = HashMap::new();

            while let Some(recv) = rx.recv().await {
                info!("{:?}", recv);
                match recv {
                    TabsRecv::Init(tabs) => {
                        state.extend(tabs.into_iter());
                    }
                    TabsRecv::Update(metadata) => {
                        state.insert(metadata.id, metadata.clone());
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
