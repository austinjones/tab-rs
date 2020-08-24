use crate::{
    message::{
        main::{MainRecv, MainShutdown},
        tabs::TabsRecv,
        terminal::{TerminalRecv, TerminalSend},
    },
    state::{tab::TabState, tabs::TabsState, terminal::TerminalMode},
};
use lifeline::{lifeline_bus, Message, Resource};

use tab_api::{request::Request, response::Response};
use tab_websocket::{
    bus::{WebsocketConnectionBus, WebsocketMessageBus},
    message::connection::{WebsocketRecv, WebsocketSend},
    resource::connection::WebsocketResource,
    service::WebsocketService,
};
use tokio::sync::{broadcast, mpsc, watch};

lifeline_bus!(
    pub struct MainBus
);

impl Message<MainBus> for MainShutdown {
    type Channel = mpsc::Sender<Self>;
}

impl Message<MainBus> for MainRecv {
    type Channel = broadcast::Sender<Self>;
}

impl Message<MainBus> for TabState {
    type Channel = watch::Sender<Self>;
}

impl Message<MainBus> for Request {
    type Channel = broadcast::Sender<Self>;
}

impl Message<MainBus> for Response {
    type Channel = broadcast::Sender<Self>;
}

impl Resource<MainBus> for WebsocketResource {}

impl WebsocketMessageBus for MainBus {
    type Send = Request;
    type Recv = Response;
}
