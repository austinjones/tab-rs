use crate::{
    message::connection::{WebsocketRecv, WebsocketSend},
    resource::{connection::WebsocketResource, listener::WebsocketAuthToken},
};
use lifeline::{lifeline_bus, Message, Resource};
use tokio::sync::mpsc;

lifeline_bus!(pub struct WebsocketConnectionBus);

impl Message<WebsocketConnectionBus> for WebsocketRecv {
    type Channel = mpsc::Sender<Self>;
}

impl Message<WebsocketConnectionBus> for WebsocketSend {
    type Channel = mpsc::Sender<Self>;
}

impl Resource<WebsocketConnectionBus> for WebsocketResource {}
impl Resource<WebsocketConnectionBus> for WebsocketAuthToken {}
