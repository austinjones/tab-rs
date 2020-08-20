use crate::{
    common::{self, should_terminate},
    WebSocket,
};

use futures::{executor::block_on, SinkExt, StreamExt};
use log::{debug, error, trace};

use std::fmt::Debug;
use tab_service::{
    service_bus, Bus, Lifeline, LinkTakenError, Message, Resource, ResourceError,
    ResourceTakenError, ResourceUninitializedError, Service, Storage,
};
use thiserror::Error;
use tokio::{select, sync::mpsc};
use tungstenite::{Error, Message as TungsteniteMessage};

pub struct WebsocketService {
    _runloop: Lifeline,
}

#[derive(Debug)]
pub struct WebsocketResource(pub WebSocket);

impl Drop for WebsocketResource {
    fn drop(&mut self) {
        block_on(self.0.close(None)).expect("websocket drop failed");
    }
}

service_bus!(pub WebsocketBus);

impl Storage for WebsocketResource {
    fn clone(tx: &mut Option<Self>) -> Option<Self> {
        tx.take()
    }
}

#[derive(Debug)]
pub struct WebsocketRecv(pub TungsteniteMessage);

#[derive(Debug)]
pub struct WebsocketSend(pub TungsteniteMessage);

impl Message<WebsocketBus> for WebsocketRecv {
    type Channel = mpsc::Sender<Self>;
}

impl Message<WebsocketBus> for WebsocketSend {
    type Channel = mpsc::Sender<Self>;
}

impl Resource<WebsocketBus> for WebsocketResource {}
pub struct WebsocketRx<Recv> {
    pub websocket: WebSocket,
    pub rx: mpsc::Receiver<Recv>,
}

#[derive(Error, Debug)]
pub enum WebsocketSpawnError {
    #[error("socket taken: {0}")]
    SocketTaken(ResourceTakenError),

    #[error("socket uninitialized: {0}")]
    SocketUninitialized(ResourceUninitializedError),

    #[error("websocket channel taken: {0}")]
    LinkTaken(LinkTakenError),
}

impl WebsocketSpawnError {
    pub fn socket_error(err: ResourceError) -> Self {
        match err {
            ResourceError::Uninitialized(uninit) => Self::SocketUninitialized(uninit),
            ResourceError::Taken(taken) => Self::SocketTaken(taken),
        }
    }

    pub fn link_taken(err: LinkTakenError) -> Self {
        Self::LinkTaken(err)
    }
}

impl Service for WebsocketService {
    type Bus = WebsocketBus;
    type Lifeline = Result<Self, WebsocketSpawnError>;

    fn spawn(bus: &WebsocketBus) -> Result<Self, WebsocketSpawnError> {
        // let mut websocket = parse_bincode(websocket);

        // let (mut tx_request, rx_request) = mpsc::channel::<Request>(4);
        // let (tx_response, mut rx_response) = mpsc::channel::<Response>(4);
        let websocket = bus
            .resource::<WebsocketResource>()
            .map_err(WebsocketSpawnError::socket_error)?;

        let rx = bus
            .rx::<WebsocketSend>()
            .map_err(WebsocketSpawnError::link_taken)?;

        let tx = bus
            .tx::<WebsocketRecv>()
            .map_err(WebsocketSpawnError::link_taken)?;

        let _runloop = Self::task("run", runloop(websocket, rx, tx));

        Ok(Self { _runloop })
    }
}

async fn runloop(
    mut websocket_drop: WebsocketResource,
    mut rx: mpsc::Receiver<WebsocketSend>,
    mut tx: mpsc::Sender<WebsocketRecv>,
) {
    let websocket = &mut websocket_drop.0;
    loop {
        select!(
            message = websocket.next() => {
                if let None = message {
                    debug!("terminating - websocket disconnected");
                    break;
                }

                trace!("message received: {:?}", &message);

                let message = message.unwrap();
                if let Err(e) = message {
                    match e {
                        Error::ConnectionClosed | Error::AlreadyClosed | Error::Protocol(_)=> {
                            break;
                        },
                        _ => {
                            error!("message error: {}", e);
                            continue;
                        }
                    }
                }

                let message = message.unwrap();
                if should_terminate(&message) {
                    debug!("terminating - received close");
                    break;
                }

                tx.send(WebsocketRecv(message)).await.expect("tx send failed");
            },
            message = rx.recv() => {
                if !message.is_some()  {
                    common::send_close(websocket).await;

                    debug!("terminating - channel disconnected");
                    break;
                }

                let message = message.unwrap();

                trace!("send message: {:?}", &message);
                websocket.send(message.0).await.expect("websocket send failed");
            },
        );
    }

    debug!("server loop terminated");
}

// async fn process_message<Request: DeserializeOwned>(
//     _websocket: &mut WebSocket,
//     response: tungstenite::Message,
//     target: &mut mpsc::Sender<Request>,
// ) {
//     if let TungsteniteMessage::Close(_) = response {
//         return;
//     }

//     let decoded = bincode::deserialize(response.into_data().as_slice());

//     if let Err(e) = decoded {
//         error!("failed to decode response: {}", e);
//         return;
//     }

//     if let Err(e) = target.send(decoded.unwrap()).await {
//         error!("failed to queue response: {}", e);
//     }
// }
