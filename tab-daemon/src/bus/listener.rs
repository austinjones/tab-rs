use crate::prelude::*;
use crate::{
    message::{
        daemon::{CloseTab, CreateTab},
        tab::{TabRecv, TabSend},
    },
    state::tab::TabsState,
};
use tab_websocket::message::listener::WebsocketConnectionMessage;
use tokio::sync::{broadcast, mpsc, watch};

lifeline_bus!(pub struct ListenerBus);

impl Message<ListenerBus> for WebsocketConnectionMessage {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ListenerBus> for TabSend {
    type Channel = broadcast::Sender<Self>;
}

impl Message<ListenerBus> for TabRecv {
    type Channel = broadcast::Sender<Self>;
}

impl Message<ListenerBus> for TabsState {
    type Channel = mpsc::Sender<Self>;
}

// impl Message<ListenerBus> for CreateTab {
//     type Channel = mpsc::Sender<Self>;
// }

// impl Message<ListenerBus> for CloseTab {
//     type Channel = mpsc::Sender<Self>;
// }

// impl Message<ListenerBus> for TabsState {
//     type Channel = watch::Sender<Self>;
// }

pub struct DaemonListenerCarrier {}

impl FromCarrier<DaemonBus> for ListenerBus {
    type Lifeline = anyhow::Result<DaemonListenerCarrier>;

    fn carry_from(&self, from: &DaemonBus) -> Self::Lifeline {
        todo!()
    }
}
