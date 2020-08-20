use crate::{
    common::{self, should_terminate},
    WebSocket,
};
use async_trait::async_trait;
use futures::{executor::block_on, StreamExt};
use log::{debug, error, trace};
use serde::{de::DeserializeOwned, Serialize};
use std::{fmt::Debug, marker::PhantomData};
use tab_service::{AsyncService, Lifeline, Service};
use tokio::{
    net::TcpStream,
    select,
    signal::ctrl_c,
    sync::{mpsc, watch},
};
use tungstenite::{Error, Message};

pub struct WebsocketService<Recv, Transmit> {
    _runloop: Lifeline,
    _recv: PhantomData<Recv>,
    _send: PhantomData<Transmit>,
}

struct WebsocketDrop(WebSocket);

impl Drop for WebsocketDrop {
    fn drop(&mut self) {
        block_on(self.0.close(None)).expect("websocket drop failed");
    }
}

pub struct WebsocketRx<Recv> {
    pub websocket: WebSocket,
    pub rx: mpsc::Receiver<Recv>,
}

impl<Recv, Transmit> Service for WebsocketService<Recv, Transmit>
where
    Recv: Send + Sync + Serialize + Debug + 'static,
    Transmit: Send + Sync + DeserializeOwned + Debug + 'static,
{
    type Rx = WebsocketRx<Recv>;
    type Tx = mpsc::Sender<Transmit>;
    type Lifeline = Self;

    fn spawn(rx: Self::Rx, message_tx: Self::Tx) -> Self {
        // let mut websocket = parse_bincode(websocket);

        // let (mut tx_request, rx_request) = mpsc::channel::<Request>(4);
        // let (tx_response, mut rx_response) = mpsc::channel::<Response>(4);
        let websocket = WebsocketDrop(rx.websocket);

        let _runloop = Self::task("run", runloop(websocket, rx.rx, message_tx));

        Self {
            _runloop,
            _recv: PhantomData,
            _send: PhantomData,
        }
    }
}

async fn runloop<Recv, Transmit>(
    mut websocket_drop: WebsocketDrop,
    mut rx: mpsc::Receiver<Recv>,
    mut tx: mpsc::Sender<Transmit>,
) where
    Recv: Serialize + Debug,
    Transmit: DeserializeOwned + Debug,
{
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

                process_message(websocket, message, &mut tx).await;
            },
            message = rx.recv() => {
                if !message.is_some()  {
                    common::send_close(websocket).await;

                    debug!("terminating - channel disconnected");
                    break;
                }

                let message = message.unwrap();

                trace!("send message: {:?}", &message);
                common::send_message(websocket, message).await;
            },
        );
    }

    debug!("server loop terminated");
}

async fn process_message<Request: DeserializeOwned>(
    _websocket: &mut WebSocket,
    response: tungstenite::Message,
    target: &mut mpsc::Sender<Request>,
) {
    if let Message::Close(_) = response {
        return;
    }

    let decoded = bincode::deserialize(response.into_data().as_slice());

    if let Err(e) = decoded {
        error!("failed to decode response: {}", e);
        return;
    }

    if let Err(e) = target.send(decoded.unwrap()).await {
        error!("failed to queue response: {}", e);
    }
}
