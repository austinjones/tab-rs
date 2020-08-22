use crate::{
    message::{
        daemon::{CloseTab, CreateTab},
        tab::{TabRecv, TabSend},
    },
    state::tab::TabsState,
};
use tab_service::{service_bus, Message};
use tab_websocket::message::listener::WebsocketConnectionMessage;
use tokio::sync::{broadcast, mpsc, watch};

service_bus!(pub ListenerBus);

impl Message<ListenerBus> for WebsocketConnectionMessage {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ListenerBus> for TabSend {
    type Channel = broadcast::Sender<Self>;
}

impl Message<ListenerBus> for TabRecv {
    type Channel = broadcast::Sender<Self>;
}

impl Message<ListenerBus> for CreateTab {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ListenerBus> for CloseTab {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ListenerBus> for TabsState {
    type Channel = watch::Sender<Self>;
}
