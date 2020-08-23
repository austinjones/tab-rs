use crate::{
    message::{
        client::TabTerminated,
        main::MainShutdown,
        tabs::TabsRecv,
        terminal::{TerminalRecv, TerminalSend},
    },
    state::{
        tab::{TabState, TabStateAvailable, TabStateSelect},
        terminal::TerminalSizeState,
    },
};
use tab_api::{request::Request, response::Response, tab::TabMetadata};
use tab_service::{service_bus, Message};
use tokio::sync::{broadcast, mpsc, watch};

service_bus!(pub ClientBus);

impl Message<ClientBus> for TabsRecv {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ClientBus> for Request {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ClientBus> for Response {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ClientBus> for TerminalSend {
    type Channel = broadcast::Sender<Self>;
}

impl Message<ClientBus> for TerminalRecv {
    type Channel = broadcast::Sender<Self>;
}

impl Message<ClientBus> for TabState {
    type Channel = watch::Sender<Self>;
}

impl Message<ClientBus> for TabMetadata {
    type Channel = broadcast::Sender<Self>;
}

impl Message<ClientBus> for TabTerminated {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ClientBus> for TabStateSelect {
    type Channel = watch::Sender<Self>;
}

impl Message<ClientBus> for TabStateAvailable {
    type Channel = watch::Sender<Self>;
}

impl Message<ClientBus> for TerminalSizeState {
    type Channel = watch::Sender<Self>;
}

impl Message<ClientBus> for MainShutdown {
    type Channel = mpsc::Sender<Self>;
}
