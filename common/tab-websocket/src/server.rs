use async_tungstenite::{
    tokio::{connect_async, TokioAdapter},
    WebSocketStream,
};
use futures::{future::ready, Future, SinkExt, StreamExt};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    common::{self, send_close},
    WebSocket,
};
use log::{debug, error, info, trace};
use std::fmt::Debug;
use tokio::sync::mpsc::Sender;
use tokio::{net::TcpStream, select, signal::ctrl_c, sync::mpsc};
use tungstenite::error::Error;
use tungstenite::Message;

pub async fn spawn_server<
    Request: DeserializeOwned + Send + Sync + 'static,
    Response: Serialize + Debug + Send + Sync + 'static,
    F: Fn(&Response) -> bool + Send + Sync + 'static,
>(
    stream: TcpStream,
    is_close: F,
) -> anyhow::Result<(mpsc::Receiver<Request>, mpsc::Sender<Response>)> {
    let addr = stream.peer_addr()?;
    let mut websocket = async_tungstenite::tokio::accept_async(stream).await?;

    // let mut websocket = parse_bincode(websocket);

    let (mut tx_request, rx_request) = mpsc::channel::<Request>(4);
    let (tx_response, mut rx_response) = mpsc::channel::<Response>(4);

    tokio::spawn(async move {
        loop {
            select!(
                request = websocket.next() => {
                    if let None = request {
                        break;
                    }

                    let response = request.unwrap();
                    if let Err(e) = response {
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

                    server_process_request(&mut websocket, response.unwrap(), &mut tx_request).await;
                },
                response = rx_response.recv() => {
                    if !response.is_some()  {
                        common::send_close(&mut websocket).await;
                        break;
                    }

                    let response = response.unwrap();
                    if is_close(&response) {
                        common::send_close(&mut websocket).await;
                        continue;
                    }

                    common::send_message(&mut websocket, response).await;
                },
                _ = ctrl_c() => {
                    common::send_close(&mut websocket).await;
                },
            );
        }

        debug!("server loop terminated");
    });

    Ok((rx_request, tx_response))
}

async fn server_process_request<Request: DeserializeOwned>(
    websocket: &mut WebSocket,
    response: tungstenite::Message,
    target: &mut Sender<Request>,
) {
    if let Message::Close(frame) = response {
        // respond_to_close(websocket, frame).await;
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
