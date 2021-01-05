use crate::{message::pty::MainShutdown, prelude::*};
use postage::{broadcast, mpsc};
use tab_api::{
    config::DaemonConfig,
    pty::{PtyWebsocketRequest, PtyWebsocketResponse},
};
use tab_websocket::{bus::WebsocketMessageBus, resource::connection::WebsocketResource};

lifeline_bus!(pub struct MainBus);

impl Message<MainBus> for PtyWebsocketRequest {
    type Channel = broadcast::Sender<Self>;
}

impl Message<MainBus> for PtyWebsocketResponse {
    type Channel = mpsc::Sender<Self>;
}

impl Message<MainBus> for MainShutdown {
    type Channel = mpsc::Sender<Self>;
}

impl Resource<MainBus> for DaemonConfig {}
impl Resource<MainBus> for WebsocketResource {}
impl WebsocketMessageBus for MainBus {
    type Send = PtyWebsocketResponse;
    type Recv = PtyWebsocketRequest;
}
