use crate::{
    message::connection::{ConnectionRecv, ConnectionSend, ConnectionShutdown},
    state::tab::TabsState,
};
use tab_api::tab::TabId;
use tab_service::{channels::subscription, service_bus, Message};
use tab_websocket::message::connection::{WebsocketRecv, WebsocketSend};
use tokio::sync::{mpsc, oneshot, watch};

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
    type Channel = mpsc::Sender<Self>;
}

impl Message<ConnectionBus> for subscription::Subscription<TabId> {
    type Channel = subscription::Sender<TabId>;
}

impl Message<ConnectionBus> for TabsState {
    type Channel = watch::Sender<Self>;
}
