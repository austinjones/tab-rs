use crate::message::main::{MainRecv, MainShutdown};
use tab_service::{service_bus, Message, Resource};
use tab_websocket::service::WebsocketResource;
use tokio::sync::{mpsc, oneshot};

service_bus!(pub MainBus);

impl Message<MainBus> for MainShutdown {
    type Channel = oneshot::Sender<Self>;
}

impl Message<MainBus> for MainRecv {
    type Channel = mpsc::Sender<Self>;
}

impl Resource<MainBus> for WebsocketResource {}
