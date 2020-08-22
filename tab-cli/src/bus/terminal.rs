use crate::{
    message::terminal::{TerminalRecv, TerminalSend},
    state::terminal::TerminalSizeState,
};
use simplelog::TerminalMode;
use tab_service::{service_bus, Message};
use tokio::sync::{broadcast, mpsc, watch};

service_bus!(pub TerminalBus);

impl Message<TerminalBus> for TerminalSend {
    type Channel = broadcast::Sender<Self>;
}

impl Message<TerminalBus> for TerminalRecv {
    type Channel = broadcast::Sender<Self>;
}

impl Message<TerminalBus> for TerminalSizeState {
    type Channel = watch::Sender<Self>;
}

impl Message<TerminalBus> for TerminalMode {
    type Channel = watch::Sender<Self>;
}
