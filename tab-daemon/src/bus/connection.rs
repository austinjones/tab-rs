use crate::message::connection::{ConnectionRecv, ConnectionSend, ConnectionShutdown};
use tab_service::{service_bus, Message};
use tab_websocket::message::connection::{WebsocketRecv, WebsocketSend};
use tokio::sync::{broadcast, mpsc, oneshot};

service_bus!(pub ConnectionBus);

impl Message<ConnectionBus> for WebsocketSend {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ConnectionBus> for WebsocketRecv {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ConnectionBus> for ConnectionShutdown {
    type Channel = oneshot::Sender<Self>;
}

impl Message<ConnectionBus> for ConnectionSend {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ConnectionBus> for ConnectionRecv {
    type Channel = broadcast::Sender<Self>;
}
