use crate::message::daemon::CloseTab;
use tab_service::{service_bus, Message};
use tokio::sync::broadcast;

service_bus!(pub TabBus);

impl Message<TabBus> for CloseTab {
    type Channel = broadcast::Sender<Self>;
}
