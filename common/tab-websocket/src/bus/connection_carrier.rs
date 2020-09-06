use super::connection::WebsocketConnectionBus;
use crate::{
    message::connection::{WebsocketRecv, WebsocketSend},
    service::WebsocketService,
};
use lifeline::{dyn_bus::DynBus, prelude::*};
use log::*;
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    stream::StreamExt,
    sync::{broadcast, mpsc},
};

/// Carries requests & responses between the websocket, and the attached bus (which must implement WebsocketMessageBus).
pub struct WebsocketCarrier {
    _websocket: WebsocketService,
    _websocket_send: Lifeline,
    _websocket_recv: Lifeline,
}

/// Defines a Send and Receive message type, and constrains the Message implementation on the bus.
/// Allows the WebsocketConnectionBus to carry messages onto the bus.
pub trait WebsocketMessageBus: Sized {
    type Send: Message<Self, Channel = mpsc::Sender<Self::Send>>
        + Clone
        + Send
        + Sync
        + Serialize
        + 'static;

    type Recv: Message<Self, Channel = broadcast::Sender<Self::Recv>>
        + Clone
        + DeserializeOwned
        + Send
        + Sync
        + 'static;
}

// TODO: why is dynbus required here?? super confusing
impl<B: DynBus> CarryFrom<B> for WebsocketConnectionBus
where
    B: WebsocketMessageBus,
{
    type Lifeline = anyhow::Result<WebsocketCarrier>;

    fn carry_from(&self, bus: &B) -> Self::Lifeline {
        use tungstenite::Message as TungsteniteMessage;

        let _websocket = WebsocketService::spawn(&self)?;
        self.capacity::<WebsocketSend>(512)?;
        self.capacity::<WebsocketRecv>(512)?;

        let _websocket_send = {
            let mut rx = bus.rx::<B::Send>()?;
            let mut tx = self.tx::<WebsocketSend>()?;

            Self::try_task("forward_send", async move {
                while let Some(msg) = rx.recv().await {
                    trace!("send message: {:?}", &msg);
                    match bincode::serialize(&msg) {
                        Ok(vec) => {
                            let send = tx
                                .send(WebsocketSend(TungsteniteMessage::Binary(vec)))
                                .await;

                            if let Err(_e) = send {
                                debug!("sender disconnected - aborting carry.");
                                break;
                            }
                        }
                        Err(e) => error!("failed to send websocket msg: {}", e),
                    };
                }

                tx.send(WebsocketSend(TungsteniteMessage::Close(None)))
                    .await
                    .ok();

                Ok(())
            })
        };

        let _websocket_recv = {
            let mut rx = self.rx::<WebsocketRecv>()?;
            let mut tx = bus.tx::<B::Recv>()?;

            Self::try_task("forward_recv", async move {
                while let Some(msg) = rx.next().await {
                    let data = msg.0.into_data();
                    match bincode::deserialize(data.as_slice()) {
                        Ok(message) => {
                            trace!("recv message: {:?}", &message);
                            tx.send(message).await?;
                        }
                        Err(e) => error!("failed to recv websocket msg: {}", e),
                    };
                }

                Ok(())
            })
        };

        Ok(WebsocketCarrier {
            _websocket,
            _websocket_send,
            _websocket_recv,
        })
    }
}
