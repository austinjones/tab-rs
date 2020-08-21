use crate::message::connection::{ConnectionRecv, ConnectionSend};
use tab_service::{service_bus, Message};
use tab_websocket::message::listener::WebsocketConnectionMessage;
use tokio::sync::{broadcast, mpsc};

service_bus!(pub ListenerBus);

impl Message<ListenerBus> for WebsocketConnectionMessage {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ListenerBus> for ConnectionSend {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ListenerBus> for ConnectionRecv {
    type Channel = broadcast::Sender<Self>;
}
