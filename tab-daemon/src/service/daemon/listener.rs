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
use log::{debug, error};
use std::sync::Arc;
use subscription::Subscription;
use tab_api::{chunk::OutputChunk, tab::TabId};
use tab_websocket::{
    bus::{WebsocketConnectionBus, WebsocketListenerBus},
    message::{
        connection::{WebsocketRecv, WebsocketSend},
        listener::WebsocketConnectionMessage,
    },
    resource::listener::{WebsocketAuthToken, WebsocketListenerResource},
    service::{WebsocketListenerService, WebsocketService},
};
use tokio::sync::{broadcast, mpsc, oneshot};

struct ConnectionLifeline {
    pub _websocket: WebsocketService,
    pub _forward: Lifeline,
    pub _reverse: Lifeline,
}

pub struct ListenerService {
    _listener: WebsocketListenerService,
    _new_session: Lifeline,
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
        // listener_bus.take_rx::<WebsocketConnectionMessage, WebsocketListenerBus>(&websocket_bus)?;
        // listener_bus.take_channel::<TabSend, DaemonBus>(bus)?;
        // listener_bus.take_channel::<TabRecv, DaemonBus>(bus)?;
        // listener_bus.take_tx::<CreateTab, DaemonBus>(bus)?;
        // listener_bus.take_tx::<CloseTab, DaemonBus>(bus)?;
        // listener_bus.take_rx::<TabsState, DaemonBus>(bus)?;

        debug!("ListenerBus: {:#?}", &listener_bus);

        let _new_session = Self::try_task("new_session", Self::new_session(listener_bus));

        Ok(Self {
            _listener,
            _new_session,
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
            let tx_tab = bus.tx::<TabRecv>()?;
            let rx_tab = bus.rx::<TabSend>()?;

            let conn_bus = ConnectionBus::default();
            // conn_bus.take_tx::<ConnectionSend, ListenerBus>(&bus)?;
            // conn_bus.take_channel::<ConnectionRecv, _>(&bus)?;
            // conn_bus.take_tx::<WebsocketSend, WebsocketConnectionBus>(&msg.bus)?;
            // conn_bus.take_rx::<WebsocketRecv, WebsocketConnectionBus>(&msg.bus)?;
            // conn_bus.take_rx::<TabsState, ListenerBus>(&bus)?;

            let tx_conn = conn_bus.tx::<ConnectionRecv>()?;
            let rx_conn = conn_bus.rx::<ConnectionSend>()?;
            let tx_create_tab = conn_bus.rx::<CreateTab>()?;
            let tx_close_tab = conn_bus.rx::<CloseTab>()?;
            let id_subscription = conn_bus.rx::<Subscription<TabId>>()?;
            let tx_shutdown = conn_bus.tx::<ConnectionShutdown>()?;

            debug!("ConnectionBus: {:?}", &bus);

            let _forward = Self::try_task(
                format!("{}_output", &name).as_str(),
                Self::run_output(rx_tab, tx_conn, id_subscription),
            );
            let _reverse = Self::try_task(
                format!("{}_input", &name).as_str(),
                Self::run_input(
                    rx_conn,
                    tx_tab,
                    tx_create_tab.clone(),
                    tx_close_tab.clone(),
                    tx_shutdown,
                ),
            );

            let support_lifeline = ConnectionLifeline {
                _websocket: msg.lifeline,
                _forward,
                _reverse,
            };
            let run_service =
                Self::try_task(name.as_str(), Self::run_service(conn_bus, support_lifeline));

            sessions.push(run_service);
            index += 1;
        }

        Ok(())
    }

    async fn run_service(
        bus: ConnectionBus,
        support_lifeline: ConnectionLifeline,
    ) -> anyhow::Result<()> {
        let shutdown = bus.rx::<ConnectionShutdown>()?;

        // keep service alive until we get a shutdown signal
        let _service = ConnectionService::spawn(&bus)?;
        drop(bus);

        shutdown.await.context("rx ConnectionShutdown closed")?;
        drop(support_lifeline);

        Ok(())
    }

    async fn run_output(
        mut rx: broadcast::Receiver<TabSend>,
        mut tx: mpsc::Sender<ConnectionRecv>,
        id_subscription: subscription::Receiver<TabId>,
    ) -> anyhow::Result<()> {
        loop {
            let msg = rx.recv().await;
            match msg {
                Ok(msg) => Self::handle_tabsend(msg, &mut tx, &id_subscription).await?,
                Err(broadcast::RecvError::Closed) => {
                    break;
                }
                Err(broadcast::RecvError::Lagged(n)) => {
                    error!("recv TabSend skipped {} messages", n)
                }
            }
        }

        Ok(())
    }

    async fn handle_tabsend(
        msg: TabSend,
        tx: &mut mpsc::Sender<ConnectionRecv>,
        id_subscription: &subscription::Receiver<TabId>,
    ) -> anyhow::Result<()> {
        match msg {
            TabSend::Started(tab) => tx.send(ConnectionRecv::TabStarted(tab)).await?,
            TabSend::Output(stdout) => {
                if !id_subscription.contains(&stdout.id) {
                    return Ok(());
                }

                tx.send(ConnectionRecv::Output(
                    stdout.id,
                    OutputChunk::clone(stdout.stdout.as_ref()),
                ))
                .await?
            }
            TabSend::Stopped(id) => tx.send(ConnectionRecv::TabStopped(id)).await?,
            TabSend::Scrollback(scrollback) => {
                tx.send(ConnectionRecv::Scrollback(scrollback)).await?;
            }
        };

        Ok(())
    }

    async fn run_input(
        mut rx: mpsc::Receiver<ConnectionSend>,
        tx: broadcast::Sender<TabRecv>,
        mut tx_create: mpsc::Sender<CreateTab>,
        mut tx_close: mpsc::Sender<CloseTab>,
        tx_shutdown: oneshot::Sender<ConnectionShutdown>,
    ) -> anyhow::Result<()> {
        while let Some(msg) = rx.recv().await {
            match msg {
                ConnectionSend::CreateTab(create) => {
                    debug!("received CreateTab from client: {:?}", &create);
                    tx_create.send(CreateTab(create)).await?;
                }
                ConnectionSend::RequestScrollback(id) => {
                    tx.send(TabRecv::Scrollback(id))
                        .map_err(|_| anyhow::Error::msg("tx TabRecv::Scrollback"))?;
                }
                ConnectionSend::Input(id, input) => {
                    let stdin = Arc::new(input);
                    let input = TabInput { id, stdin };
                    let message = TabRecv::Input(input);
                    tx.send(message)
                        .map_err(|_| anyhow::Error::msg("tx TabRecv closed"))?;
                }
                ConnectionSend::CloseTab(id) => {
                    tx_close.send(CloseTab(id)).await?;
                }
                ConnectionSend::CloseNamedTab(name) => {
                    let message = TabRecv::Input(input);
                }
            }
        }

        tx_shutdown
            .send(ConnectionShutdown {})
            .map_err(|_| anyhow::Error::msg("tx ConnectionShutdown closed"))?;

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
