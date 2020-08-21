use crate::bus::ConnectionBus;
use crate::bus::DaemonBus;
use crate::bus::ListenerBus;
use crate::{
    message::connection::{ConnectionRecv, ConnectionSend, ConnectionShutdown},
    service::connection::ConnectionService,
};
use tab_service::{dyn_bus::DynBus, Bus, Lifeline, Service};
use tab_websocket::{
    bus::WebsocketListenerBus,
    message::{
        connection::{WebsocketRecv, WebsocketSend},
        listener::WebsocketConnectionMessage,
    },
    resource::listener::WebsocketListenerResource,
    service::WebsocketListenerService,
};

pub struct WebsocketService {
    _listener: WebsocketListenerService,
    _new_session: Lifeline,
}

impl Service for WebsocketService {
    type Bus = DaemonBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> anyhow::Result<Self> {
        let websocket_bus = WebsocketListenerBus::default();
        websocket_bus.take_resource::<WebsocketListenerResource, _>(bus)?;

        let _listener = WebsocketListenerService::spawn(&websocket_bus)?;

        let listener_bus = ListenerBus::default();
        listener_bus.take_rx::<WebsocketConnectionMessage, _>(&websocket_bus)?;
        listener_bus.take_tx::<ConnectionSend, _>(bus)?;
        listener_bus.take_channel::<ConnectionRecv, _>(bus)?;

        let _new_session = Self::task("new_session", Self::new_session(listener_bus));

        Ok(Self {
            _listener,
            _new_session,
        })
    }
}

impl WebsocketService {
    async fn new_session(bus: ListenerBus) -> anyhow::Result<()> {
        // TODO: think about better ways to clean up this.
        let mut sessions = Vec::new();
        let mut index = 0usize;

        let mut rx_conn = bus.rx::<WebsocketConnectionMessage>()?;

        while let Some(msg) = rx_conn.recv().await {
            let name = format!("connection_{}", index);

            let bus = ConnectionBus::default();
            bus.take_tx::<ConnectionSend, _>(&bus)?;
            bus.take_channel::<ConnectionRecv, _>(&bus)?;
            bus.take_rx::<WebsocketRecv, _>(&msg.bus)?;
            bus.take_tx::<WebsocketSend, _>(&msg.bus)?;

            let lifeline = Self::try_task(name.as_str(), Self::run_service(bus));

            sessions.push((lifeline, msg.lifeline));
            index += 1;
        }

        Ok(())
    }

    async fn run_service(bus: ConnectionBus) -> anyhow::Result<()> {
        let shutdown = bus.rx::<ConnectionShutdown>()?;

        // keep service alive until we get a shutdown signal
        let _service = ConnectionService::spawn(&bus)?;

        shutdown.await.expect("shutdown recv failed");

        Ok(())
    }
}
