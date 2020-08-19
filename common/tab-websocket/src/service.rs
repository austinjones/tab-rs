use crate::{
    common::{self, should_terminate},
    WebSocket,
};
use async_trait::async_trait;
use futures::StreamExt;
use log::{debug, error, trace};
use serde::{de::DeserializeOwned, Serialize};
use std::{fmt::Debug, marker::PhantomData};
use tab_service::{spawn, AsyncService, Lifeline, Service};
use tokio::{net::TcpStream, select, signal::ctrl_c, sync::mpsc};
use tungstenite::{Error, Message};
pub struct WebsocketService<Recv, Transmit> {
    _runloop: Lifeline,
    _recv: PhantomData<Recv>,
    _send: PhantomData<Transmit>,
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

    fn spawn(rx: Self::Rx, message_tx: Self::Tx) -> Self {
        // let mut websocket = parse_bincode(websocket);

        // let (mut tx_request, rx_request) = mpsc::channel::<Request>(4);
        // let (tx_response, mut rx_response) = mpsc::channel::<Response>(4);

        let _runloop = spawn(async { runloop(rx.websocket, rx.rx, message_tx) });

        Self {
            _runloop,
            _recv: PhantomData,
            _send: PhantomData,
        }
    }
}

async fn runloop<Recv, Transmit>(
    mut websocket: WebSocket,
    mut rx: mpsc::Receiver<Recv>,
    mut tx: mpsc::Sender<Transmit>,
) where
    Recv: Serialize + Debug,
    Transmit: DeserializeOwned + Debug,
{
    loop {
        select!(
            request = websocket.next() => {
                if let None = request {
                    break;
                }

                trace!("request received: {:?}", &request);

                let request = request.unwrap();
                if let Err(e) = request {
                    match e {
                        Error::ConnectionClosed | Error::AlreadyClosed | Error::Protocol(_)=> {
                            break;
                        },
                        _ => {
                            error!("request error: {}", e);
                            continue;
                        }
                    }
                }

                let request = request.unwrap();
                if should_terminate(&request) {
                    break;
                }

                process_request(&mut websocket, request, &mut tx).await;
            },
            message = rx.recv() => {
                if !message.is_some()  {
                    common::send_close(&mut websocket).await;
                    break;
                }

                let message = message.unwrap();

                debug!("send message: {:?}", &message);
                common::send_message(&mut websocket, message).await;
            },
            _ = ctrl_c() => {
                // common::send_close(&mut websocket).await;
            },
        );
    }

    debug!("server loop terminated");
}

async fn process_request<Request: DeserializeOwned>(
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
