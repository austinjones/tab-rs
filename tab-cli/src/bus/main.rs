use crate::{
    message::{
        main::{MainRecv, MainShutdown},
        terminal::{TerminalRecv, TerminalSend},
    },
    state::terminal::TerminalMode,
};
use tab_service::{service_bus, Message, Resource};
use tab_websocket::resource::connection::WebsocketResource;
use tokio::sync::{broadcast, mpsc, watch};

service_bus!(pub MainBus);

impl Message<MainBus> for MainShutdown {
    type Channel = mpsc::Sender<Self>;
}

impl Message<MainBus> for MainRecv {
    type Channel = mpsc::Sender<Self>;
}

impl Message<MainBus> for TerminalMode {
    type Channel = watch::Sender<Self>;
}

impl Message<MainBus> for TerminalSend {
    type Channel = broadcast::Sender<Self>;
}

impl Message<MainBus> for TerminalRecv {
    type Channel = broadcast::Sender<Self>;
}

impl Resource<MainBus> for WebsocketResource {}
