use crate::message::daemon::DaemonShutdown;
use crate::prelude::*;
use tab_api::config::DaemonConfig;
use tab_websocket::resource::listener::{WebsocketAuthToken, WebsocketListenerResource};
use tokio::sync::oneshot;

lifeline_bus!(pub struct DaemonBus);

impl Resource<DaemonBus> for DaemonConfig {}
impl Resource<DaemonBus> for WebsocketListenerResource {}
impl Resource<DaemonBus> for WebsocketAuthToken {}

impl Message<DaemonBus> for DaemonShutdown {
    type Channel = oneshot::Sender<Self>;
}
