use crate::WebSocket;
use async_trait::async_trait;
use std::marker::PhantomData;
use tab_service::{AsyncService, Service};
use tokio::{net::TcpStream, sync::mpsc};

pub struct WebsocketService<Recv, Send> {
    _runloop: Lifeline,
    _recv: PhantomData<Recv>,
    _send: PhantomData<Send>,
}

pub struct WebsocketRx<Recv> {
    websocket: WebSocket,
    rx: mpsc::Receiver<Recv>,
}

#[async_trait]
impl<Recv, Send> AsyncService for WebsocketService<Recv, Send> {
    type Rx = WebsocketRx<Recv>;
    type Tx = mpsc::Sender<Send>;

    async fn spawn(rx: Self::Rx, tx: Self::Tx) -> Self {
        let mut websocket = async_tungstenite::tokio::accept_async(rx.tcp_stream).await;

        // let mut websocket = parse_bincode(websocket);

        // let (mut tx_request, rx_request) = mpsc::channel::<Request>(4);
        // let (tx_response, mut rx_response) = mpsc::channel::<Response>(4);

        let service = spawn();

        Self {}
    }

    async fn shutdown(self) {
        todo!()
    }
}

async fn runloop<Recv, Send>(
    websocket: WebSocket,
    rx: mpsc::Receiver<Recv>,
    tx: mpsc::Sender<Send>,
) {
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

                server_process_request(&mut websocket, request, &mut tx_request).await;
            },
            message = rx.recv() => {
                if !message.is_some()  {
                    common::send_close(&mut websocket).await;
                    break;
                }

                let message = message.unwrap();

                if is_close(&response) {
                    common::send_close(&mut websocket).await;
                    continue;
                }

                debug!("send message: {:?}", &response);
                common::send_message(&mut websocket, response).await;
            },
            _ = ctrl_c() => {
                common::send_close(&mut websocket).await;
            },
        );
    }

    debug!("server loop terminated");
}
