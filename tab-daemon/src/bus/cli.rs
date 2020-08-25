use crate::prelude::*;
use crate::{
    message::{
        cli::{CliRecv, CliSend, CliShutdown},
        tab::{TabInput, TabRecv, TabSend},
        tab_manager::TabManagerRecv,
    },
    state::tab::TabsState,
};

use std::sync::Arc;
use subscription::Subscription;
use tab_api::{chunk::OutputChunk, client::Request, client::Response, tab::TabId};
use tab_websocket::{bus::WebsocketMessageBus, resource::connection::WebsocketResource};
use tokio::sync::{broadcast, mpsc, oneshot};

lifeline_bus!(pub struct CliBus);

impl Message<CliBus> for CliShutdown {
    type Channel = oneshot::Sender<Self>;
}

impl Message<CliBus> for Request {
    type Channel = broadcast::Sender<Self>;
}

impl Message<CliBus> for Response {
    type Channel = broadcast::Sender<Self>;
}

impl Message<CliBus> for CliSend {
    type Channel = mpsc::Sender<Self>;
}

impl Message<CliBus> for CliRecv {
    type Channel = mpsc::Sender<Self>;
}

impl Message<CliBus> for subscription::Subscription<TabId> {
    type Channel = subscription::Sender<TabId>;
}

impl Message<CliBus> for TabsState {
    type Channel = mpsc::Sender<Self>;
}

impl Resource<CliBus> for WebsocketResource {}
impl WebsocketMessageBus for CliBus {
    type Send = Response;
    type Recv = Request;
}

pub struct ListenerConnectionCarrier {
    _forward: Lifeline,
    _reverse: Lifeline,
    _forward_tabs_state: Lifeline,
}

impl FromCarrier<ListenerBus> for CliBus {
    type Lifeline = anyhow::Result<ListenerConnectionCarrier>;

    fn carry_from(&self, from: &ListenerBus) -> Self::Lifeline {
        let tx_tab = from.tx::<TabRecv>()?;
        let rx_tab = from.rx::<TabSend>()?;

        let tx_conn = self.tx::<CliRecv>()?;
        let rx_conn = self.rx::<CliSend>()?;
        let tx_manager = from.tx::<TabManagerRecv>()?;
        let id_subscription = self.rx::<Subscription<TabId>>()?;
        let tx_shutdown = self.tx::<CliShutdown>()?;

        let _forward = Self::try_task("output", Self::run_output(rx_tab, tx_conn, id_subscription));
        let _reverse = Self::try_task(
            "input",
            Self::run_input(rx_conn, tx_tab, tx_manager, tx_shutdown),
        );

        let _forward_tabs_state = {
            let mut rx_tabs_state = from.rx::<TabsState>()?;
            let mut tx_tabs_state = self.tx::<TabsState>()?;
            Self::try_task("forward_tabs_state", async move {
                while let Some(msg) = rx_tabs_state.recv().await {
                    tx_tabs_state.send(msg).await.map_err(into_msg)?;
                }

                Ok(())
            })
        };

        Ok(ListenerConnectionCarrier {
            _forward,
            _reverse,
            _forward_tabs_state,
        })
    }
}

impl CliBus {
    async fn run_output(
        mut rx: broadcast::Receiver<TabSend>,
        mut tx: mpsc::Sender<CliRecv>,
        id_subscription: subscription::Receiver<TabId>,
    ) -> anyhow::Result<()> {
        loop {
            let msg = rx.recv().await;
            match msg {
                Ok(msg) => Self::handle_tabsend(msg, &mut tx, &id_subscription).await?,
                Err(broadcast::RecvError::Closed) => {
                    break;
                }
                Err(broadcast::RecvError::Lagged(n)) => {
                    error!("recv TabSend skipped {} messages", n)
                }
            }
        }

        Ok(())
    }

    async fn run_input(
        mut rx: mpsc::Receiver<CliSend>,
        tx: broadcast::Sender<TabRecv>,
        mut tx_manager: mpsc::Sender<TabManagerRecv>,
        tx_shutdown: oneshot::Sender<CliShutdown>,
    ) -> anyhow::Result<()> {
        while let Some(msg) = rx.recv().await {
            match msg {
                CliSend::CreateTab(create) => {
                    debug!("received CreateTab from client: {:?}", &create);
                    tx_manager.send(TabManagerRecv::CreateTab(create)).await?;
                }
                CliSend::CloseTab(id) => {
                    tx_manager.send(TabManagerRecv::CloseTab(id)).await?;
                }
                CliSend::CloseNamedTab(name) => {
                    tx_manager.send(TabManagerRecv::CloseNamedTab(name)).await?;
                }
                CliSend::RequestScrollback(id) => {
                    tx.send(TabRecv::Scrollback(id))
                        .map_err(|_| anyhow::Error::msg("tx TabRecv::Scrollback"))?;
                }
                CliSend::Input(id, input) => {
                    let stdin = Arc::new(input);
                    let input = TabInput { id, stdin };
                    let message = TabRecv::Input(input);
                    tx.send(message)
                        .map_err(|_| anyhow::Error::msg("tx TabRecv closed"))?;
                }
            }
        }

        tx_shutdown
            .send(CliShutdown {})
            .map_err(|_| anyhow::Error::msg("tx ConnectionShutdown closed"))?;

        Ok(())
    }

    async fn handle_tabsend(
        msg: TabSend,
        tx: &mut mpsc::Sender<CliRecv>,
        id_subscription: &subscription::Receiver<TabId>,
    ) -> anyhow::Result<()> {
        match msg {
            TabSend::Started(tab) => tx.send(CliRecv::TabStarted(tab)).await?,
            TabSend::Output(stdout) => {
                if !id_subscription.contains(&stdout.id) {
                    return Ok(());
                }

                tx.send(CliRecv::Output(
                    stdout.id,
                    OutputChunk::clone(stdout.stdout.as_ref()),
                ))
                .await?
            }
            TabSend::Stopped(id) => tx.send(CliRecv::TabStopped(id)).await?,
            TabSend::Scrollback(scrollback) => {
                tx.send(CliRecv::Scrollback(scrollback)).await?;
            }
        };

        Ok(())
    }
}
