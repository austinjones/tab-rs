use crate::{
    message::listener::WebsocketConnectionMessage,
    resource::listener::{WebsocketAuthToken, WebsocketListenerResource},
};
use lifeline::{lifeline_bus, Message, Resource};
use tokio::sync::mpsc;

lifeline_bus!(pub struct WebsocketListenerBus);

impl Message<WebsocketListenerBus> for WebsocketConnectionMessage {
    type Channel = mpsc::Sender<Self>;
}

impl Resource<WebsocketListenerBus> for WebsocketListenerResource {}
impl Resource<WebsocketListenerBus> for WebsocketAuthToken {}
