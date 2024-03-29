use super::{
    retask::RetaskService, tab_assignment::TabAssignmentService, tab_manager::TabManagerService,
};
use crate::{
    message::{
        cli::CliShutdown,
        listener::ListenerShutdown,
        pty::{PtyRecv, PtySend, PtyShutdown},
        tab::{TabRecv, TabSend},
    },
    service::{cli::CliService, pty::PtyService},
};
use crate::{prelude::*, service::cli::subscription::CliSubscriptionService};

use lifeline::dyn_bus::DynBus;
use tab_api::pty::{PtyWebsocketRequest, PtyWebsocketResponse};
use tab_websocket::{
    bus::{WebsocketCarrier, WebsocketListenerBus},
    message::listener::WebsocketConnectionMessage,
    resource::listener::{WebsocketAuthToken, WebsocketListenerResource},
    service::WebsocketListenerService,
};

struct CliLifeline {
    _websocket_carrier: WebsocketCarrier,
    _listener_carrier: ListenerConnectionCarrier,
}

struct PtyLifeline {
    _websocket_carrier: WebsocketCarrier,
    _listener_carrier: ListenerPtyCarrier,
}

pub struct ListenerService {
    _listener: WebsocketListenerService,
    _new_session: Lifeline,
    _tabs: TabManagerService,
    _tab_assignments: TabAssignmentService,
    _retask: RetaskService,
    _connection_carrier: ConnectionMessageCarrier,
    _daemon_carrier: ListenerDaemonCarrier,
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
        listener_bus.capacity::<TabSend>(128)?;
        listener_bus.capacity::<TabRecv>(128)?;

        let _daemon_carrier = listener_bus.carry_from(bus)?;
        let _connection_carrier = listener_bus.carry_from(&websocket_bus)?;

        let _tab_assignments = TabAssignmentService::spawn(&listener_bus)?;
        let _tabs = TabManagerService::spawn(&listener_bus)?;
        let _retask = RetaskService::spawn(&listener_bus)?;

        let _new_session = Self::try_task("new_session", Self::new_session(listener_bus));

        Ok(Self {
            _listener,
            _new_session,
            _tabs,
            _tab_assignments,
            _retask,
            _connection_carrier,
            _daemon_carrier,
        })
    }
}

impl ListenerService {
    async fn new_session(bus: ListenerBus) -> anyhow::Result<()> {
        // TODO: think about better ways to clean up this.
        let mut sessions = Vec::new();
        let mut index = 0usize;

        let mut rx_conn = bus.rx::<WebsocketConnectionMessage>()?;

        let mut tx_terminate_tabs = bus.tx::<TabRecv>()?;
        let mut tx_shutdown = bus.tx::<ListenerShutdown>()?;

        while let Some(msg) = rx_conn.recv().await {
            let name = format!("connection_{}", index);
            debug!(
                "opening connection {}, from HTTP {} {}",
                name, msg.request.method, msg.request.uri
            );

            let lifeline = match msg.request.uri.to_string().as_str() {
                "/cli" => {
                    let cli_bus = CliBus::default();
                    cli_bus.capacity::<Request>(128)?;
                    cli_bus.capacity::<Response>(256)?;

                    let _listener_carrier = cli_bus.carry_from(&bus)?;
                    let _websocket_carrier = cli_bus.carry_into(&msg.bus)?;

                    let _connection = CliLifeline {
                        _websocket_carrier,
                        _listener_carrier,
                    };

                    Self::try_task(
                        (name + "/cli").as_str(),
                        Self::run_cli(cli_bus, _connection),
                    )
                }
                "/pty" => {
                    let pty_bus = PtyBus::default();
                    pty_bus.capacity::<PtySend>(128)?;
                    pty_bus.capacity::<PtyRecv>(128)?;
                    pty_bus.capacity::<PtyWebsocketRequest>(128)?;
                    pty_bus.capacity::<PtyWebsocketResponse>(128)?;

                    let _listener_carrier = pty_bus.carry_from(&bus)?;
                    let _websocket_carrier = pty_bus.carry_into(&msg.bus)?;

                    let _pty_lifeline = PtyLifeline {
                        _websocket_carrier,
                        _listener_carrier,
                    };
                    Self::try_task(
                        (name + "/pty").as_str(),
                        Self::run_pty(pty_bus, _pty_lifeline),
                    )
                }
                "/shutdown" => {
                    tx_terminate_tabs.send(TabRecv::TerminateAll).await?;
                    tx_shutdown.send(ListenerShutdown {}).await?;
                    break;
                }
                _ => {
                    error!("unknown endpoint: {}", msg.request.uri);
                    continue;
                }
            };

            sessions.push(lifeline);
            index += 1;
        }

        Ok(())
    }

    async fn run_cli(bus: CliBus, _connection: CliLifeline) -> anyhow::Result<()> {
        let mut shutdown = bus.rx::<CliShutdown>()?;

        // keep service alive until we get a shutdown signal
        let _service = CliService::spawn(&bus)?;
        let _subscription = CliSubscriptionService::spawn(&bus)?;
        drop(bus);

        shutdown.recv().await;

        Ok(())
    }

    async fn run_pty(bus: PtyBus, _connection: PtyLifeline) -> anyhow::Result<()> {
        let mut shutdown = bus.rx::<PtyShutdown>()?;

        // keep service alive until we get a shutdown signal
        let _service = PtyService::spawn(&bus)?;
        drop(bus);

        shutdown.recv().await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ListenerService;

    use async_tungstenite::tokio::connect_async;
    use http::StatusCode;
    use lifeline::{dyn_bus::DynBus, prelude::*};
    use std::fmt::Debug;
    use tab_api::config::DaemonConfig;
    use tab_websocket::bus::WebsocketConnectionBus;
    use tab_websocket::{resource::connection::WebsocketResource, service::WebsocketService};
    use tungstenite::{handshake::client::Request, http};

    #[tokio::test]
    async fn test_listener_spawn() -> anyhow::Result<()> {
        let bus = crate::new_bus("0.0.1").await?;
        let _listener = ListenerService::spawn(&bus)?;

        Ok(())
    }

    #[tokio::test]
    async fn test_listener_accepts_connection() -> anyhow::Result<()> {
        let bus = crate::new_bus("0.0.1").await?;
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
            assert_eq!(expect, code.status());
        } else {
            panic!(
                "tungstenite::Error::Http({}) expected, found: {:?}",
                expect, result
            );
        }
    }

    #[tokio::test]
    async fn test_listener_rejects_unauthorized() -> anyhow::Result<()> {
        let bus = crate::new_bus("0.0.1").await?;
        let config = bus.resource::<DaemonConfig>()?;
        let _listener = ListenerService::spawn(&bus)?;

        let connection = tab_websocket::connect(format!("ws://127.0.0.1:{}", config.port)).await;
        assert!(connection.is_err());
        assert_status_err(StatusCode::UNAUTHORIZED, connection);

        Ok(())
    }

    #[tokio::test]
    async fn test_listener_rejects_bad_token() -> anyhow::Result<()> {
        let bus = crate::new_bus("0.0.1").await?;
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
        let bus = crate::new_bus("0.0.1").await?;
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
