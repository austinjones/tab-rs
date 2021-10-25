use crate::{
    bus::WebsocketConnectionBus,
    message::connection::{WebsocketRecv, WebsocketSend},
    resource::connection::WebsocketResource,
};
use lifeline::prelude::*;
use log::{debug, info, Level};
use postage::{sink::Sink, stream::Stream};
use tokio::select;

use crate::common::{self, should_terminate};

use futures::{SinkExt, StreamExt};
use log::{error, trace};

use anyhow::Context;
use std::fmt::Debug;

use lifeline::error::{
    ResourceTakenError, ResourceUninitializedError, TakeChannelError, TakeResourceError,
};
use thiserror::Error;
use tungstenite::Error;

/// A service which clients & servers use to drive the websocket connection.
/// Handles connection status & close frames, as well as logging & protocol errors.
#[derive(Debug)]
pub struct WebsocketService {
    _runloop: Lifeline,
}

impl Service for WebsocketService {
    type Bus = WebsocketConnectionBus;
    type Lifeline = Result<Self, WebsocketSpawnError>;

    fn spawn(bus: &WebsocketConnectionBus) -> Result<Self, WebsocketSpawnError> {
        // let mut websocket = parse_bincode(websocket);

        // let (mut tx_request, rx_request) = mpsc::channel::<Request>(4);
        // let (tx_response, mut rx_response) = mpsc::channel::<Response>(4);
        let websocket = bus
            .resource::<WebsocketResource>()
            .map_err(WebsocketSpawnError::socket_error)?;

        let rx = bus
            .rx::<WebsocketSend>()
            .map_err(WebsocketSpawnError::bus_failure)?
            .log(Level::Debug);

        let tx = bus
            .tx::<WebsocketRecv>()
            .map_err(WebsocketSpawnError::bus_failure)?
            .log(Level::Debug);

        let _runloop = Self::try_task("run", runloop(websocket, rx, tx));

        Ok(Self { _runloop })
    }
}

async fn runloop(
    mut websocket_drop: WebsocketResource,
    mut rx: impl Stream<Item = WebsocketSend> + Unpin,
    mut tx: impl Sink<Item = WebsocketRecv> + Unpin,
) -> anyhow::Result<()> {
    let websocket = &mut websocket_drop.0;
    info!("starting runloop");
    loop {
        select!(
            message = websocket.next() => {
                if message.is_none() {
                    info!("terminating - websocket disconnected");
                    break;
                }

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

                tx.send(WebsocketRecv(message)).await.context("send WebsocketRecv")?;
            },
            message = rx.recv() => {
                if !message.is_some()  {
                    common::send_close(websocket).await;

                    debug!("terminating - channel disconnected");
                    break;
                }

                let message = message.unwrap();

                trace!("send message: {:?}", &message);
                websocket.send(message.0).await.context("wire send Tungstenite::Message")?;
            },
        );
    }

    debug!("server loop terminated");
    Ok(())
}
#[derive(Error, Debug)]
pub enum WebsocketSpawnError {
    #[error("resource taken: {0}")]
    SocketTaken(ResourceTakenError),

    #[error("socket uninitialized: {0}")]
    SocketUninitialized(ResourceUninitializedError),

    #[error("websocket channel taken: {0}")]
    BusFailure(TakeChannelError),
}

impl WebsocketSpawnError {
    pub fn socket_error(err: TakeResourceError) -> Self {
        match err {
            TakeResourceError::Uninitialized(uninit) => Self::SocketUninitialized(uninit),
            TakeResourceError::Taken(taken) => Self::SocketTaken(taken),
        }
    }

    pub fn bus_failure(err: TakeChannelError) -> Self {
        Self::BusFailure(err)
    }
}

#[cfg(test)]
mod test {
    use super::WebsocketService;
    use crate::bus::WebsocketConnectionBus;
    use crate::{
        connect_authorized,
        message::{
            connection::{WebsocketRecv, WebsocketSend},
            listener::WebsocketConnectionMessage,
        },
        resource::{connection::WebsocketResource, listener::WebsocketAuthToken},
        service::listener,
    };
    use lifeline::prelude::*;
    use lifeline::{assert_completes, dyn_bus::DynBus};
    use postage::{sink::Sink, stream::Stream};
    use tungstenite::Message;

    #[tokio::test]
    async fn connect_authenticated() -> anyhow::Result<()> {
        let (listener_bus, _lifeline, addr) = listener::serve("TOKEN").await?;

        let url = format!("ws://{}", addr);
        let connect = connect_authorized(url, "TOKEN".to_string()).await?;

        let bus = WebsocketConnectionBus::default();
        bus.store_resource::<WebsocketAuthToken>("TOKEN".into());
        bus.store_resource::<WebsocketResource>(WebsocketResource(connect));

        let mut tx_request = bus.tx::<WebsocketSend>()?;
        let mut rx_conn = listener_bus.rx::<WebsocketConnectionMessage>()?;

        let _service = WebsocketService::spawn(&bus)?;

        tx_request
            .send(WebsocketSend(Message::Text("request".to_string())))
            .await?;

        assert_completes!(async move {
            let conn = rx_conn.recv().await;
            assert!(conn.is_some());
            let conn = conn.unwrap();
            let conn_bus = conn.bus;
            let _service = WebsocketService::spawn(&conn_bus);

            let mut rx_request = conn_bus
                .rx::<WebsocketRecv>()
                .expect("conn_bus rx WebsocketRecv");
            let request_recv = rx_request.recv().await.expect("rx_request recv");
            let request_recv = request_recv.0.into_text().expect("into text");
            assert_eq!("request", request_recv);
        });

        Ok(())
    }
}
