use crate::{
    message::{
        daemon::CloseTab,
        tab::{TabRecv, TabSend},
    },
    state::tab::TabsState,
};
use tab_api::tab::TabId;
use tab_service::{channels::subscription, service_bus, Message};
use tokio::sync::{broadcast, watch};

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
