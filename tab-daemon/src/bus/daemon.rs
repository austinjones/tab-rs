use crate::{
    message::{
        daemon::{CloseTab, CreateTab, DaemonShutdown},
        tab::{TabRecv, TabSend},
    },
    state::tab::TabsState,
};
use tab_api::config::DaemonConfig;
use tab_service::{service_bus, Message, Resource};
use tab_websocket::resource::listener::{WebsocketAuthToken, WebsocketListenerResource};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

service_bus!(pub DaemonBus);

impl Resource<DaemonBus> for DaemonConfig {}
impl Resource<DaemonBus> for WebsocketListenerResource {}
impl Resource<DaemonBus> for WebsocketAuthToken {}

impl Message<DaemonBus> for DaemonShutdown {
    type Channel = oneshot::Sender<Self>;
}

impl Message<DaemonBus> for CreateTab {
    type Channel = mpsc::Sender<Self>;
}

impl Message<DaemonBus> for CloseTab {
    type Channel = mpsc::Sender<Self>;
}

impl Message<DaemonBus> for TabSend {
    type Channel = broadcast::Sender<Self>;
}

impl Message<DaemonBus> for TabRecv {
    type Channel = broadcast::Sender<Self>;
}

impl Message<DaemonBus> for TabsState {
    type Channel = watch::Sender<Self>;
}
