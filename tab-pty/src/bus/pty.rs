use crate::{
    message::pty::PtyShutdown,
    prelude::*,
    pty_process::{PtyRequest, PtyResponse},
};
use tab_api::{
    config::DaemonConfig,
    pty::{PtyWebsocketRequest, PtyWebsocketResponse},
};
use tab_websocket::{bus::WebsocketMessageBus, resource::connection::WebsocketResource};
use tokio::sync::{broadcast, mpsc};

lifeline_bus!(pub struct PtyBus);

impl Message<PtyBus> for PtyWebsocketRequest {
    type Channel = broadcast::Sender<Self>;
}

impl Message<PtyBus> for PtyWebsocketResponse {
    type Channel = broadcast::Sender<Self>;
}

impl Message<PtyBus> for PtyRequest {
    type Channel = mpsc::Sender<Self>;
}

impl Message<PtyBus> for PtyResponse {
    type Channel = mpsc::Sender<Self>;
}

impl Message<PtyBus> for PtyShutdown {
    type Channel = mpsc::Sender<Self>;
}

impl Resource<PtyBus> for DaemonConfig {}
impl Resource<PtyBus> for WebsocketResource {}
impl WebsocketMessageBus for PtyBus {
    type Send = PtyWebsocketResponse;
    type Recv = PtyWebsocketRequest;
}
