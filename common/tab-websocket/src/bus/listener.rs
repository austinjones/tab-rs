use crate::{
    message::listener::WebsocketConnectionMessage,
    resource::listener::{WebsocketAuthToken, WebsocketListenerResource},
};
use tab_service::{service_bus, Message, Resource};
use tokio::sync::mpsc;

service_bus!(pub WebsocketListenerBus);

impl Message<WebsocketListenerBus> for WebsocketConnectionMessage {
    type Channel = mpsc::Sender<Self>;
}

// impl Message<WebsocketBus> for WebsocketSend {
//     type Channel = mpsc::Sender<Self>;
// }

impl Resource<WebsocketListenerBus> for WebsocketListenerResource {}
impl Resource<WebsocketListenerBus> for WebsocketAuthToken {}
