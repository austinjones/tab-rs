use crate::prelude::*;
use crate::{
    message::{
        tab::{TabRecv, TabSend},
        tab_manager::{TabManagerRecv, TabManagerSend},
    },
    state::{assignment::Retraction, tab::TabsState},
};
use tab_api::tab::TabMetadata;
use tab_websocket::{bus::WebsocketListenerBus, message::listener::WebsocketConnectionMessage};
use tokio::sync::{broadcast, mpsc, watch};

lifeline_bus!(pub struct ListenerBus);

impl Message<ListenerBus> for WebsocketConnectionMessage {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ListenerBus> for TabSend {
    type Channel = broadcast::Sender<Self>;
}

impl Message<ListenerBus> for TabRecv {
    type Channel = broadcast::Sender<Self>;
}

impl Message<ListenerBus> for TabManagerSend {
    type Channel = mpsc::Sender<Self>;
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

pub struct ConnectionMessageCarrier {
    _forward_connection: Lifeline,
}

impl FromCarrier<WebsocketListenerBus> for ListenerBus {
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
