use crate::prelude::*;
use crate::{
    message::{
        daemon::DaemonShutdown,
        listener::ListenerShutdown,
        tab::{TabRecv, TabSend},
        tab_manager::{TabManagerRecv, TabManagerSend},
    },
    state::{assignment::Retraction, tab::TabsState},
};
use lifeline::error::into_msg;
use tab_api::tab::TabMetadata;
use tab_websocket::{bus::WebsocketListenerBus, message::listener::WebsocketConnectionMessage};
use tokio::sync::{broadcast, mpsc, watch};

lifeline_bus!(pub struct ListenerBus);

impl Message<ListenerBus> for WebsocketConnectionMessage {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ListenerBus> for ListenerShutdown {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ListenerBus> for TabSend {
    type Channel = broadcast::Sender<Self>;
}

impl Message<ListenerBus> for TabRecv {
    type Channel = broadcast::Sender<Self>;
}

impl Message<ListenerBus> for TabManagerSend {
    type Channel = broadcast::Sender<Self>;
}

impl Message<ListenerBus> for TabManagerRecv {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ListenerBus> for Retraction<TabMetadata> {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ListenerBus> for TabsState {
    type Channel = watch::Sender<Self>;
}

pub struct ListenerDaemonCarrier {
    _forward_shutdown: Lifeline,
}

impl CarryFrom<DaemonBus> for ListenerBus {
    type Lifeline = anyhow::Result<ListenerDaemonCarrier>;

    fn carry_from(&self, from: &DaemonBus) -> Self::Lifeline {
        let _forward_shutdown = {
            let mut rx = self.rx::<ListenerShutdown>()?;
            let mut tx = from.tx::<DaemonShutdown>()?;

            Self::task("forward_shutdown", async move {
                if let Some(shutdown) = rx.recv().await {
                    tx.send(DaemonShutdown {}).await.ok();
                }
            })
        };

        Ok(ListenerDaemonCarrier { _forward_shutdown })
    }
}

pub struct ConnectionMessageCarrier {
    _forward_connection: Lifeline,
}

impl CarryFrom<WebsocketListenerBus> for ListenerBus {
    type Lifeline = anyhow::Result<ConnectionMessageCarrier>;
    fn carry_from(&self, from: &WebsocketListenerBus) -> Self::Lifeline {
        let _forward_connection = {
            let mut rx = from.rx::<WebsocketConnectionMessage>()?;
            let mut tx = self.tx::<WebsocketConnectionMessage>()?;
            Self::try_task("forward_connection", async move {
                while let Some(msg) = rx.recv().await {
                    tx.send(msg).await.map_err(into_msg)?;
                }

                Ok(())
            })
        };

        Ok(ConnectionMessageCarrier {
            _forward_connection,
        })
    }
}
