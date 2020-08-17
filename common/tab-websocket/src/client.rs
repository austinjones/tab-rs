use async_tungstenite::{
    tokio::{connect_async, TokioAdapter},
    WebSocketStream,
};
use futures::{future::ready, Future, SinkExt, StreamExt};
use serde::{de::DeserializeOwned, Serialize};

use crate::{common, WebSocket};
use log::{debug, error, info, trace};
use std::fmt::Debug;
use tokio::sync::mpsc::Sender;
use tokio::{net::TcpStream, select, signal::ctrl_c, sync::mpsc};
use tungstenite::error::Error;
use tungstenite::Message;

pub async fn spawn_client<
    Request: Serialize + Send + Sync + 'static,
    Response: DeserializeOwned + Debug + Send + Sync + 'static,
    F: Fn(&Request) -> bool + Send + Sync + 'static,
>(
    url: &str,
    is_close: F,
) -> anyhow::Result<(mpsc::Sender<Request>, mpsc::Receiver<Response>)> {
    let (mut websocket, _) = connect_async(url).await?;

    let (tx_request, mut rx_request) = mpsc::channel::<Request>(4);
    let (mut tx_response, rx_response) = mpsc::channel::<Response>(4);

    tokio::spawn(async move {
        loop {
            select!(
                response = websocket.next() => {
                    if let None = response {
                        break;
                    }

                    let response = response.unwrap();
                    if let Err(e) = response {
                        match e {
                            Error::ConnectionClosed | Error::AlreadyClosed | Error::Protocol(_)=> {
                                break;
                            },
                            _ => {
                                error!("response error: {}", e);
                                continue;
                            }
                        }
                    }

                    client_process_response(&mut websocket, response.unwrap(), &mut tx_response).await;
                },
                request = rx_request.recv() => {
                    if !request.is_some()  {
                        common::send_close(&mut websocket).await;
                        continue;
                    }

                    let request = request.unwrap();
                    if is_close(&request) {
                        common::send_close(&mut websocket).await;
                        continue;
                    }

                    common::send_message(&mut websocket, request).await;
                },
                _ = ctrl_c() => {
                    common::send_close(&mut websocket).await;
                },
            );
        }

        debug!("client loop terminated");
    });

    Ok((tx_request, rx_response))
}

async fn client_process_response<Response: DeserializeOwned>(
    websocket: &mut WebSocket,
    response: tungstenite::Message,
    target: &mut Sender<Response>,
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
