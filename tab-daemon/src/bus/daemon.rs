use crate::message::daemon::DaemonShutdown;
use crate::prelude::*;
use lifeline::Resource;
use postage::mpsc;
use tab_api::config::DaemonConfig;
use tab_websocket::resource::listener::{WebsocketAuthToken, WebsocketListenerResource};

lifeline_bus!(pub struct DaemonBus);

impl Resource<DaemonBus> for DaemonConfig {}
impl Resource<DaemonBus> for WebsocketListenerResource {}
impl Resource<DaemonBus> for WebsocketAuthToken {}

impl Message<DaemonBus> for DaemonShutdown {
    type Channel = mpsc::Sender<Self>;
}
