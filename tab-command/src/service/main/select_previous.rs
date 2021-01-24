use postage::watch;
use tab_api::tab::TabId;

use crate::{
    message::main::{MainRecv, MainShutdown},
    prelude::*,
    utils::await_state,
};
use crate::{message::tabs::TabRecv, state::tabs::ActiveTabsState};

use super::env_tab_id;

pub struct MainSelectPreviousTabService {
    _run: Lifeline,
}

impl Service for MainSelectPreviousTabService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut rx = bus.rx::<MainRecv>()?;
        let mut rx_active = bus.rx::<Option<ActiveTabsState>>()?;

        let mut tx_tab = bus.tx::<TabRecv>()?;
        let mut tx_shutdown = bus.tx::<MainShutdown>()?;

        let _run = Self::try_task("run", async move {
            while let Some(recv) = rx.recv().await {
                if let MainRecv::SelectPreviousTab = recv {
                    Self::select_previous(&mut rx_active, &mut tx_tab, &mut tx_shutdown).await?;
                }
            }

            Ok(())
        });

        Ok(Self { _run })
    }
}

impl MainSelectPreviousTabService {
    async fn select_previous(
        mut rx_active_tabs: &mut watch::Receiver<Option<ActiveTabsState>>,
        mut tx_tab: impl Sink<Item = TabRecv> + Unpin,
        mut tx_shutdown: impl Sink<Item = MainShutdown> + Unpin,
    ) -> anyhow::Result<()> {
        info!("MainRecv::SelectPreviousTab running");
        let env_tab = env_tab_id();
        let state = await_state(&mut rx_active_tabs).await?;

        if let Some(name) = Self::target(env_tab, state) {
            let message = TabRecv::SelectNamedTab { name, env_tab };
            tx_tab.send(message).await?;
        } else {
            println!("No tabs have been selected in the current session.");
            tx_shutdown.send(MainShutdown(1)).await?;
        }

        Ok(())
    }

    fn target(selected: Option<TabId>, active: ActiveTabsState) -> Option<String> {
        active
            .tabs
            .iter()
            .fold(None, |prev, (id, metadata)| {
                // ignore the currently selected tab
                if selected == Some(*id) {
                    return prev;
                }

                // otherwise, accept the tab if we have no match
                if prev.is_none() {
                    return Some(metadata);
                }

                // otherwise, if the metadata is newer than prev, accept it
                if prev.unwrap().selected < metadata.selected {
                    return Some(metadata);
                }

                // otherwise, return prev
                prev
            })
            .map(|metadata| metadata.name.clone())
    }
}
