use std::time::Duration;

use anyhow::Context;
use postage::watch;
use tab_api::{client::RetaskTarget, tab::TabId};
use tokio::time;

use crate::{
    message::tabs::CreateTabRequest, message::tabs::TabShutdown, prelude::*,
    state::tab::SelectOrRetaskTab, state::tab::SelectTab, state::tabs::ActiveTabsState,
    utils::await_condition,
};

pub struct SelectTabService {
    _run: Lifeline,
}

impl Service for SelectTabService {
    type Bus = TabBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut rx = bus.rx::<SelectOrRetaskTab>()?;
        let mut rx_tabs_state = bus.rx::<Option<ActiveTabsState>>()?;

        let mut tx_create = bus.tx::<CreateTabRequest>()?;
        let mut tx_select = bus.tx::<SelectTab>()?;
        let mut tx_websocket = bus.tx::<Request>()?;
        let mut tx_shutdown = bus.tx::<TabShutdown>()?;

        let _run = Self::try_task("run", async move {
            while let Some(msg) = rx.recv().await {
                Self::select_named(
                    msg.name,
                    msg.env_tab,
                    &mut rx_tabs_state,
                    &mut tx_websocket,
                    &mut tx_create,
                    &mut tx_select,
                    &mut tx_shutdown,
                )
                .await?;
            }

            Ok(())
        });

        Ok(Self { _run })
    }
}

impl SelectTabService {
    async fn select_named(
        name: String,
        env_id: Option<TabId>,
        rx_tabs_state: &mut watch::Receiver<Option<ActiveTabsState>>,
        mut tx_websocket: impl Sink<Item = Request> + Unpin,
        mut tx_create: impl Sink<Item = CreateTabRequest> + Unpin,
        mut tx_select: impl Sink<Item = SelectTab> + Unpin,
        mut tx_shutdown: impl Sink<Item = TabShutdown> + Unpin,
    ) -> anyhow::Result<()> {
        if let Some(id) = env_id {
            info!("retasking tab {} with new selection {}.", id.0, &name);

            tx_create
                .send(CreateTabRequest::Named(name.clone()))
                .await?;

            debug!("retask - waiting for creation on tab {}", id.0);

            let state =
                await_condition(rx_tabs_state, |state| state.contains_name(name.as_str())).await?;
            let metadata = state.find_name(name.as_str()).unwrap();

            if metadata.id == id {
                debug!("retask - client already selected on tab {}", id.0);
                tx_shutdown.send(TabShutdown {}).await?;
                return Ok(());
            }

            debug!("retask - sending retask to tab {}", id);
            let request = Request::Retask(id, RetaskTarget::Tab(metadata.id));
            tx_websocket.send(request).await?;

            // if we quit too early, the carrier is cancelled and our message doesn't get through.
            // this sleep is not visible to the user, as the outer terminal session will emit new stdout
            time::sleep(Duration::from_millis(250)).await;

            tx_shutdown.send(TabShutdown {}).await?;
            return Ok(());
        }

        tx_create
            .send(CreateTabRequest::Named(name.clone()))
            .await?;

        tx_select
            .send(SelectTab::NamedTab(name))
            .await
            .context("send TabStateSelect")?;

        Ok(())
    }
}
