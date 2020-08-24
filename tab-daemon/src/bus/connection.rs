use crate::prelude::*;
use crate::{
    message::connection::{ConnectionRecv, ConnectionSend, ConnectionShutdown},
    state::tab::TabsState,
};
use tab_api::{request::Request, response::Response, tab::TabId};
use tab_websocket::{
    bus::{WebsocketConnectionBus, WebsocketMessageBus},
    message::connection::{WebsocketRecv, WebsocketSend},
    resource::connection::WebsocketResource,
    service::WebsocketService,
};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

lifeline_bus!(pub struct ConnectionBus);

// impl Message<ConnectionBus> for WebsocketSend {
//     type Channel = mpsc::Sender<Self>;
// }

// impl Message<ConnectionBus> for WebsocketRecv {
//     type Channel = mpsc::Sender<Self>;
// }

impl Message<ConnectionBus> for ConnectionShutdown {
    type Channel = oneshot::Sender<Self>;
}

impl Message<ConnectionBus> for Request {
    type Channel = broadcast::Sender<Self>;
}

impl Message<ConnectionBus> for Response {
    type Channel = broadcast::Sender<Self>;
}

impl Message<ConnectionBus> for ConnectionSend {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ConnectionBus> for ConnectionRecv {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ConnectionBus> for subscription::Subscription<TabId> {
    type Channel = subscription::Sender<TabId>;
}

impl Message<ConnectionBus> for TabsState {
    type Channel = mpsc::Sender<Self>;
}

impl Resource<ConnectionBus> for WebsocketResource {}
// impl Message<ConnectionBus> for TabsState {
//     type Channel = watch::Sender<Self>;
// }

impl WebsocketMessageBus for ConnectionBus {
    type Send = Response;
    type Recv = Request;
}

// pub struct ListenerConnectionCarrier {}

// impl FromCarrier<ListenerBus> for ConnectionBus {
//     type Lifeline = anyhow::Result<DaemonListenerCarrier>;

//     fn carry_from(&self, from: &ListenerBus) -> Self::Lifeline {
//         todo!()
//     }
// }
