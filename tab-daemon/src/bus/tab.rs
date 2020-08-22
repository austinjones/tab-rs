use crate::message::{
    daemon::CloseTab,
    tab::{TabRecv, TabSend},
};

use tab_service::{service_bus, Message};
use tokio::sync::broadcast;

service_bus!(pub TabBus);

impl Message<TabBus> for CloseTab {
    type Channel = broadcast::Sender<Self>;
}

impl Message<TabBus> for TabSend {
    type Channel = broadcast::Sender<Self>;
}

impl Message<TabBus> for TabRecv {
    type Channel = broadcast::Sender<Self>;
}
