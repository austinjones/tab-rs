use super::connection::WebsocketConnectionBus;
use crate::{
    message::connection::{WebsocketRecv, WebsocketSend},
    resource::connection::WebsocketResource,
    service::WebsocketService,
};
use dyn_bus::DynBus;
use lifeline::error::into_msg;
use lifeline::*;
use log::*;
use serde::{de::DeserializeOwned, Serialize};
use tokio::stream::StreamExt;
use tokio::sync::broadcast;

pub struct WebsocketCarrier {
    _websocket: WebsocketService,
    _websocket_send: Lifeline,
    _websocket_recv: Lifeline,
}

pub trait WebsocketMessageBus: Sized {
    type Send: Message<Self, Channel = broadcast::Sender<Self::Send>>
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
/// If you get a weird error here, make sure your bus carries the WebsocketResource.
/// Rust is bad at resolving this apparently
impl<B: DynBus> FromCarrier<B> for WebsocketConnectionBus
where
    B: WebsocketMessageBus,
    B: Stores<WebsocketResource>,
    WebsocketResource: Resource<B>,
{
    type Lifeline = anyhow::Result<WebsocketCarrier>;

    fn carry_from(&self, bus: &B) -> Self::Lifeline {
        use tungstenite::Message as TungsteniteMessage;

        let websocket = bus.resource::<WebsocketResource>()?;
        self.store_resource(websocket);

        let _websocket = WebsocketService::spawn(&self)?;

        let _websocket_send = {
            let mut rx = bus.rx::<B::Send>()?;
            let mut tx = self.tx::<WebsocketSend>()?;

            Self::try_task("forward_request", async move {
                while let Some(result) = rx.next().await {
                    if let Ok(msg) = result {
                        match bincode::serialize(&msg) {
                            Ok(vec) => {
                                tx.send(WebsocketSend(TungsteniteMessage::Binary(vec)))
                                    .await?
                            }
                            Err(e) => error!("failed to send websocket msg: {}", e),
                        };
                    }
                }

                tx.send(WebsocketSend(TungsteniteMessage::Close(None)))
                    .await?;

                Ok(())
            })
        };

        let _websocket_recv = {
            let mut rx = self.rx::<WebsocketRecv>()?;
            let mut tx = bus.tx::<B::Recv>()?;

            Self::try_task("forward_request", async move {
                while let Some(msg) = rx.next().await {
                    let data = msg.0.into_data();
                    match bincode::deserialize(data.as_slice()) {
                        Ok(message) => {
                            tx.send(message).map_err(into_msg)?;
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
