use crate::message::{
    daemon::CloseTab,
    tab::{TabRecv, TabSend},
};
use crate::prelude::*;

use lifeline::{lifeline_bus, Message};
use tokio::sync::broadcast;

lifeline_bus!(pub struct TabBus);

impl Message<TabBus> for CloseTab {
    type Channel = broadcast::Sender<Self>;
}

impl Message<TabBus> for TabSend {
    type Channel = broadcast::Sender<Self>;
}

impl Message<TabBus> for TabRecv {
    type Channel = broadcast::Sender<Self>;
}
