use crate::message::{
    connection::{ConnectionRecv, ConnectionSend},
    daemon::DaemonShutdown,
};
use tab_api::config::DaemonConfig;
use tab_service::{service_bus, Message, Resource};
use tab_websocket::{
    resource::listener::WebsocketListenerResource,
};
use tokio::sync::{broadcast, mpsc, oneshot};

service_bus!(pub DaemonBus);

impl Resource<DaemonBus> for DaemonConfig {}
impl Resource<DaemonBus> for WebsocketListenerResource {}

impl Message<DaemonBus> for DaemonShutdown {
    type Channel = oneshot::Sender<Self>;
}

impl Message<DaemonBus> for ConnectionSend {
    type Channel = mpsc::Sender<Self>;
}

impl Message<DaemonBus> for ConnectionRecv {
    type Channel = broadcast::Sender<Self>;
}
