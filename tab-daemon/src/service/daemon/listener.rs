use super::tabs::TabsService;
use crate::prelude::*;
use crate::{
    message::{
        connection::{ConnectionRecv, ConnectionSend, ConnectionShutdown},
        daemon::{CloseTab, CreateTab},
        tab::{TabInput, TabRecv, TabSend},
    },
    service::connection::ConnectionService,
};
use anyhow::Context;
use dyn_bus::DynBus;
use log::debug;

use tab_websocket::{
    bus::{WebsocketCarrier, WebsocketConnectionBus, WebsocketListenerBus},
    message::{
        connection::{WebsocketRecv, WebsocketSend},
        listener::WebsocketConnectionMessage,
    },
    resource::listener::{WebsocketAuthToken, WebsocketListenerResource},
    service::{WebsocketListenerService, WebsocketService},
};

struct ConnectionLifeline {
    _websocket_carrier: WebsocketCarrier,
    _listener_carrier: ListenerConnectionCarrier,
}

pub struct ListenerService {
    _listener: WebsocketListenerService,
    _new_session: Lifeline,
    _tabs: TabsService,
    _connection_carrier: ConnectionMessageCarrier,
}

impl Service for ListenerService {
    type Bus = DaemonBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> anyhow::Result<Self> {
        let websocket_bus = WebsocketListenerBus::default();

        let listener_resource = bus.resource::<WebsocketListenerResource>()?;
        let authtoken_resource = bus.resource::<WebsocketAuthToken>()?;

        websocket_bus.store_resource(listener_resource);
        websocket_bus.store_resource(authtoken_resource);

        let _listener = WebsocketListenerService::spawn(&websocket_bus)?;

        let listener_bus = ListenerBus::default();
        let _connection_carrier = listener_bus.carry_from(&websocket_bus)?;

        let _tabs = TabsService::spawn(&listener_bus)?;

        debug!("ListenerBus: {:#?}", &listener_bus);

        let _new_session = Self::try_task("new_session", Self::new_session(listener_bus));

        Ok(Self {
            _listener,
            _new_session,
            _connection_carrier,
            _tabs,
        })
    }
}

impl ListenerService {
    async fn new_session(bus: ListenerBus) -> anyhow::Result<()> {
        // TODO: think about better ways to clean up this.
        let mut sessions = Vec::new();
        let mut index = 0usize;

        let mut rx_conn = bus.rx::<WebsocketConnectionMessage>()?;

        // let tx_create_tab = bus.tx::<CreateTab>()?;
        // let tx_close_tab = bus.tx::<CloseTab>()?;

        while let Some(msg) = rx_conn.recv().await {
            let name = format!("connection_{}", index);
            debug!("Starting {}", name);

            let conn_bus = ConnectionBus::default();
            let _listener_carrier = conn_bus.carry_from(&bus)?;
            let _websocket_carrier = msg.bus.carry_from(&conn_bus)?;

            let _connection = ConnectionLifeline {
                _listener_carrier,
                _websocket_carrier,
            };

            let run_service =
                Self::try_task(name.as_str(), Self::run_service(conn_bus, _connection));

            sessions.push(run_service);
            index += 1;
        }

        Ok(())
    }

    async fn run_service(
        bus: ConnectionBus,
        _connection: ConnectionLifeline,
    ) -> anyhow::Result<()> {
        let shutdown = bus.rx::<ConnectionShutdown>()?;

        // keep service alive until we get a shutdown signal
        let _service = ConnectionService::spawn(&bus)?;
        drop(bus);

        shutdown.await.context("rx ConnectionShutdown closed")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ListenerService;

    use async_tungstenite::tokio::connect_async;
    use http::StatusCode;
    use lifeline::{dyn_bus::DynBus, Bus, Service};
    use std::fmt::Debug;
    use tab_api::config::DaemonConfig;
    use tab_websocket::bus::WebsocketConnectionBus;
    use tab_websocket::{resource::connection::WebsocketResource, service::WebsocketService};
    use tungstenite::{handshake::client::Request, http};

    #[tokio::test]
    async fn test_listener_spawn() -> anyhow::Result<()> {
        let bus = crate::new_bus().await?;
        let _listener = ListenerService::spawn(&bus)?;

        Ok(())
    }

    #[tokio::test]
    async fn test_listener_accepts_connection() -> anyhow::Result<()> {
        let bus = crate::new_bus().await?;
        let config = bus.resource::<DaemonConfig>()?;

        let _listener = ListenerService::spawn(&bus)?;

        let websocket_bus = WebsocketConnectionBus::default();
        let connection = tab_websocket::connect_authorized(
            format!("ws://127.0.0.1:{}", config.port),
            config.auth_token,
        )
        .await?;
        websocket_bus.store_resource(WebsocketResource(connection));

        let _connection = WebsocketService::spawn(&websocket_bus)?;

        Ok(())
    }

    fn assert_status_err<T: Debug>(
        expect: http::StatusCode,
        result: Result<T, tungstenite::Error>,
    ) {
        if let Err(tungstenite::Error::Http(code)) = result {
            assert_eq!(expect, code);
        } else {
            panic!(
                "tungstenite::Error::Http({}) expected, found: {:?}",
                expect, result
            );
        }
    }

    #[tokio::test]
    async fn test_listener_rejects_unauthorized() -> anyhow::Result<()> {
        let bus = crate::new_bus().await?;
        let config = bus.resource::<DaemonConfig>()?;
        let _listener = ListenerService::spawn(&bus)?;

        let connection = tab_websocket::connect(format!("ws://127.0.0.1:{}", config.port)).await;
        assert!(connection.is_err());
        assert_status_err(StatusCode::UNAUTHORIZED, connection);

        Ok(())
    }

    #[tokio::test]
    async fn test_listener_rejects_bad_token() -> anyhow::Result<()> {
        let bus = crate::new_bus().await?;
        let config = bus.resource::<DaemonConfig>()?;
        let _listener = ListenerService::spawn(&bus)?;

        let connection = tab_websocket::connect_authorized(
            format!("ws://127.0.0.1:{}", config.port),
            "BAD TOKEN".into(),
        )
        .await;
        assert!(connection.is_err());
        assert_status_err(StatusCode::UNAUTHORIZED, connection);

        Ok(())
    }

    #[tokio::test]
    async fn test_listener_rejects_origin() -> anyhow::Result<()> {
        let bus = crate::new_bus().await?;
        let config = bus.resource::<DaemonConfig>()?;
        let _listener = ListenerService::spawn(&bus)?;

        let request = Request::builder()
            .uri(format!("ws://127.0.0.1:{}", config.port))
            .header("Authorization", config.auth_token)
            .header("Origin", "http://badwebsite.com")
            .body(())?;
        let result = connect_async(request).await;

        assert!(result.is_err());
        assert_status_err(StatusCode::FORBIDDEN, result);

        Ok(())
    }
}
