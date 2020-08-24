use crate::prelude::*;
use crate::{
    message::{
        connection::{ConnectionRecv, ConnectionSend, ConnectionShutdown},
        daemon::{CloseTab, CreateTab},
        tab::{TabInput, TabRecv, TabSend},
    },
    state::tab::TabsState,
};

use std::sync::Arc;
use subscription::Subscription;
use tab_api::{chunk::OutputChunk, request::Request, response::Response, tab::TabId};
use tab_websocket::{
    bus::{WebsocketConnectionBus, WebsocketMessageBus},
    message::connection::{WebsocketRecv, WebsocketSend},
    resource::connection::WebsocketResource,
    service::WebsocketService,
};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

lifeline_bus!(pub struct ConnectionBus);

impl Message<ConnectionBus> for ConnectionShutdown {
    type Channel = oneshot::Sender<Self>;
}

impl Message<ConnectionBus> for Request {
    type Channel = broadcast::Sender<Self>;
}

impl Message<ConnectionBus> for Response {
    type Channel = broadcast::Sender<Self>;
}

impl Message<ConnectionBus> for ConnectionSend {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ConnectionBus> for ConnectionRecv {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ConnectionBus> for subscription::Subscription<TabId> {
    type Channel = subscription::Sender<TabId>;
}

impl Message<ConnectionBus> for TabsState {
    type Channel = mpsc::Sender<Self>;
}

// impl Message<ConnectionBus> for TabsState {
//     type Channel = watch::Sender<Self>;
// }

impl Resource<ConnectionBus> for WebsocketResource {}
impl WebsocketMessageBus for ConnectionBus {
    type Send = Response;
    type Recv = Request;
}

pub struct ListenerConnectionCarrier {
    _forward: Lifeline,
    _reverse: Lifeline,
    _forward_tabs_state: Lifeline,
}

impl FromCarrier<ListenerBus> for ConnectionBus {
    type Lifeline = anyhow::Result<ListenerConnectionCarrier>;

    fn carry_from(&self, from: &ListenerBus) -> Self::Lifeline {
        let tx_tab = from.tx::<TabRecv>()?;
        let rx_tab = from.rx::<TabSend>()?;

        let tx_conn = self.tx::<ConnectionRecv>()?;
        let rx_conn = self.rx::<ConnectionSend>()?;
        let tx_create_tab = from.tx::<CreateTab>()?;
        let tx_close_tab = from.tx::<CloseTab>()?;
        let id_subscription = self.rx::<Subscription<TabId>>()?;
        let tx_shutdown = self.tx::<ConnectionShutdown>()?;
        let rx_tabs_state = from.rx::<TabsState>()?;

        let _forward = Self::try_task("output", Self::run_output(rx_tab, tx_conn, id_subscription));
        let _reverse = Self::try_task(
            "input",
            Self::run_input(
                rx_conn,
                tx_tab,
                tx_create_tab.clone(),
                tx_close_tab.clone(),
                rx_tabs_state,
                tx_shutdown,
            ),
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

impl ConnectionBus {
    async fn run_output(
        mut rx: broadcast::Receiver<TabSend>,
        mut tx: mpsc::Sender<ConnectionRecv>,
        id_subscription: subscription::Receiver<TabId>,
    ) -> anyhow::Result<()> {
        loop {
            let msg = rx.recv().await;
            debug!("got run_output: {:?}", &msg);
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
        mut rx: mpsc::Receiver<ConnectionSend>,
        tx: broadcast::Sender<TabRecv>,
        mut tx_create: mpsc::Sender<CreateTab>,
        mut tx_close: mpsc::Sender<CloseTab>,
        mut rx_tabs_state: watch::Receiver<TabsState>,
        tx_shutdown: oneshot::Sender<ConnectionShutdown>,
    ) -> anyhow::Result<()> {
        while let Some(msg) = rx.recv().await {
            match msg {
                ConnectionSend::CreateTab(create) => {
                    debug!("received CreateTab from client: {:?}", &create);
                    tx_create.send(CreateTab(create)).await?;
                }
                ConnectionSend::RequestScrollback(id) => {
                    tx.send(TabRecv::Scrollback(id))
                        .map_err(|_| anyhow::Error::msg("tx TabRecv::Scrollback"))?;
                }
                ConnectionSend::Input(id, input) => {
                    let stdin = Arc::new(input);
                    let input = TabInput { id, stdin };
                    let message = TabRecv::Input(input);
                    tx.send(message)
                        .map_err(|_| anyhow::Error::msg("tx TabRecv closed"))?;
                }
                ConnectionSend::CloseTab(id) => {}
                ConnectionSend::CloseNamedTab(name) => {
                    let mut close_ids = Vec::new();

                    for tab in rx_tabs_state.borrow().tabs.values() {
                        if tab.name == name {
                            close_ids.push(tab.id);
                        }
                    }

                    for id in close_ids {
                        tx_close.send(CloseTab(id)).await?;
                    }
                }
            }
        }

        tx_shutdown
            .send(ConnectionShutdown {})
            .map_err(|_| anyhow::Error::msg("tx ConnectionShutdown closed"))?;

        Ok(())
    }

    async fn handle_tabsend(
        msg: TabSend,
        tx: &mut mpsc::Sender<ConnectionRecv>,
        id_subscription: &subscription::Receiver<TabId>,
    ) -> anyhow::Result<()> {
        debug!("got tabsend: {:?}", &msg);
        match msg {
            TabSend::Started(tab) => tx.send(ConnectionRecv::TabStarted(tab)).await?,
            TabSend::Output(stdout) => {
                if !id_subscription.contains(&stdout.id) {
                    return Ok(());
                }

                tx.send(ConnectionRecv::Output(
                    stdout.id,
                    OutputChunk::clone(stdout.stdout.as_ref()),
                ))
                .await?
            }
            TabSend::Stopped(id) => tx.send(ConnectionRecv::TabStopped(id)).await?,
            TabSend::Scrollback(scrollback) => {
                tx.send(ConnectionRecv::Scrollback(scrollback)).await?;
            }
        };

        Ok(())
    }
}
