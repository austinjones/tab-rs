use crate::{
    message::connection::{WebsocketRecv, WebsocketSend},
    resource::connection::WebsocketResource,
};
use tab_service::{service_bus, Message, Resource};
use tokio::sync::mpsc;

service_bus!(pub WebsocketConnectionBus);

impl Message<WebsocketConnectionBus> for WebsocketRecv {
    type Channel = mpsc::Sender<Self>;
}

impl Message<WebsocketConnectionBus> for WebsocketSend {
    type Channel = mpsc::Sender<Self>;
}

impl Resource<WebsocketConnectionBus> for WebsocketResource {}
