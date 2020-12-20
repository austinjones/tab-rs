use crate::bus::WebsocketConnectionBus;
use crate::{
    bind,
    bus::WebsocketListenerBus,
    message::listener::WebsocketConnectionMessage,
    resource::{
        connection::WebsocketResource,
        listener::{WebsocketAuthToken, WebsocketListenerResource},
    },
};
use log::{debug, error};

use lifeline::prelude::*;
use lifeline::{dyn_bus::DynBus, request::Request as LifelineRequest};
use tokio::net::UnixListener;

/// An established listener service, which transmits WebsocketConnectionMessages over the listener bus.
pub struct WebsocketListenerService {
    _accept: Lifeline,
}

impl Service for WebsocketListenerService {
    type Bus = WebsocketListenerBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let listener = bus.resource::<WebsocketListenerResource>()?;
        let auth_token = bus.resource::<WebsocketAuthToken>()?;

        let tx = bus.tx::<WebsocketConnectionMessage>()?;
        let _accept = Self::try_task("accept", accept_connections(listener.0, tx, auth_token));

        Ok(Self { _accept })
    }
}

/// The main runloop for the WebsocketListenerService
async fn accept_connections(
    mut listener: UnixListener,
    mut tx: impl Sender<WebsocketConnectionMessage>,
    auth_token: WebsocketAuthToken,
) -> anyhow::Result<()> {
    loop {
        let (stream, addr) = listener.accept().await?;

        // TODO: only accept connections from loopback address
        debug!("connection opened from {:?}", addr);

        let conn_bus = WebsocketConnectionBus::default();
        let (request, recv_metadata) = LifelineRequest::send(());
        let bound = match bind(stream, auth_token.clone(), request).await {
            Ok(res) => res,
            Err(e) => {
                error!("error binding websocket: {}", e);
                continue;
            }
        };

        conn_bus.store_resource(WebsocketResource(bound));
        // let service = WebsocketService::spawn(&conn_bus)?;

        let message = WebsocketConnectionMessage {
            bus: conn_bus,
            request: recv_metadata.await?,
        };

        tx.send(message)
            .await
            .map_err(|_| anyhow::Error::msg("send WebsocketConnectionMessage"))?;
    }
}

#[cfg(test)]
use std::path::PathBuf;

#[cfg(test)]
use tempfile::{tempdir, TempDir};

#[cfg(test)]
pub(crate) fn test_socket() -> anyhow::Result<(PathBuf, TempDir)> {
    let dir = tempdir()?;

    let mut path = dir.path().to_path_buf();
    path.push("tab-sock");

    Ok((path, dir))
}

#[cfg(test)]
pub(crate) async fn serve(
    token: &str,
) -> anyhow::Result<(
    WebsocketListenerBus,
    WebsocketListenerService,
    (PathBuf, TempDir),
)> {
    let bus = WebsocketListenerBus::default();
    bus.store_resource::<WebsocketAuthToken>(token.into());

    let socket = test_socket()?;

    let listener = UnixListener::bind(socket.0.as_path())?;
    let websocket = WebsocketListenerResource(listener);
    bus.store_resource(websocket);

    let lifeline = WebsocketListenerService::spawn(&bus)?;

    Ok((bus, lifeline, socket))
}

#[cfg(test)]
mod tests {
    use super::{serve, test_socket, WebsocketListenerService};
    use crate::{
        bus::*,
        message::{
            connection::{WebsocketRecv, WebsocketSend},
            listener::WebsocketConnectionMessage,
        },
        resource::{
            connection::WebsocketResource,
            listener::{WebsocketAuthToken, WebsocketListenerResource},
        },
        service::WebsocketService,
    };
    use lifeline::{assert_completes, dyn_bus::DynBus, prelude::*};
    use std::path::Path;
    use tokio::net::UnixListener;

    async fn connect(
        socket: &Path,
        token: &str,
    ) -> anyhow::Result<(WebsocketConnectionBus, WebsocketService)> {
        let bus = WebsocketConnectionBus::default();
        bus.store_resource::<WebsocketAuthToken>(token.into());

        let connection =
            crate::connect_authorized(socket, "/".to_string(), token.to_string()).await?;
        bus.store_resource(WebsocketResource(connection));

        let lifeline = WebsocketService::spawn(&bus)?;
        Ok((bus, lifeline))
    }

    #[tokio::test]
    async fn test_listener_spawn() -> anyhow::Result<()> {
        let bus = WebsocketListenerBus::default();
        bus.store_resource(WebsocketAuthToken::unauthenticated());

        let (socket, _dir) = test_socket()?;
        let listener = UnixListener::bind(socket)?;
        let websocket = WebsocketListenerResource(listener);
        bus.store_resource(websocket);

        let _listener = WebsocketListenerService::spawn(&bus)?;

        Ok(())
    }

    #[tokio::test]
    async fn test_listener_accepts_connection() -> anyhow::Result<()> {
        let (listener_bus, _listener, (socket, _tempdir)) = serve("TOKEN").await?;

        let bus = WebsocketConnectionBus::default();
        let connection =
            crate::connect_authorized(socket.as_path(), "/".to_string(), "TOKEN".to_string())
                .await?;
        bus.store_resource(WebsocketResource(connection));

        let _sender = WebsocketService::spawn(&bus)?;

        let mut rx_conn = listener_bus
            .rx::<WebsocketConnectionMessage>()?
            .into_inner();
        let conn = rx_conn.try_recv();

        assert!(conn.is_ok());

        Ok(())
    }

    #[tokio::test]
    async fn test_send_request() -> anyhow::Result<()> {
        let (listener_bus, _serve, (socket, _tempdir)) = serve("TOKEN").await?;
        let (bus, _connect) = connect(socket.as_path(), "TOKEN").await?;

        let mut rx_conn = listener_bus
            .rx::<WebsocketConnectionMessage>()?
            .into_inner();
        let conn = rx_conn.try_recv()?;
        let _serve = WebsocketService::spawn(&conn.bus)?;

        let mut tx_request = bus.tx::<WebsocketSend>()?;
        let mut rx_request = conn.bus.rx::<WebsocketRecv>()?;

        tx_request
            .send(WebsocketSend(tungstenite::Message::Text(
                "request".to_string(),
            )))
            .await?;

        assert_completes!(async move {
            let request_recv = rx_request.recv().await.expect("rx_request recv");
            let request_recv = request_recv.0.into_text().expect("into text");
            assert_eq!("request", request_recv);
        });

        Ok(())
    }

    #[tokio::test]
    async fn test_send_response() -> anyhow::Result<()> {
        let (listener_bus, _serve, (socket, _tempdir)) = serve("TOKEN").await?;
        let (bus, _connect) = connect(socket.as_path(), "TOKEN").await?;

        let mut rx_conn = listener_bus
            .rx::<WebsocketConnectionMessage>()?
            .into_inner();
        let conn = rx_conn.try_recv()?;
        let _serve = WebsocketService::spawn(&conn.bus)?;

        let mut rx_response = bus.rx::<WebsocketRecv>()?;

        let mut tx_response = conn.bus.tx::<WebsocketSend>()?;

        tx_response
            .send(WebsocketSend(tungstenite::Message::Text(
                "response".to_string(),
            )))
            .await?;

        assert_completes!(async move {
            let response_recv = rx_response.recv().await;
            assert!(response_recv.is_some());
            let response_recv = response_recv.unwrap().0.into_text().expect("into text");
            assert_eq!("response", response_recv);
        });

        Ok(())
    }
}
