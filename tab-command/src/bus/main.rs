use crate::{
    message::main::{MainRecv, MainShutdown},
    message::tabs::TabRecv,
    message::terminal::TerminalRecv,
    message::terminal::TerminalSend,
    state::tabs::ActiveTabsState,
    state::workspace::WorkspaceState,
};
use crate::{prelude::*, state::tab::TabState};
use lifeline::prelude::*;

use tab_websocket::{bus::WebsocketMessageBus, resource::connection::WebsocketResource};
use tokio::sync::{broadcast, mpsc, watch};

lifeline_bus!(
    pub struct MainBus
);

impl Message<MainBus> for MainShutdown {
    type Channel = mpsc::Sender<Self>;
}

impl Message<MainBus> for MainRecv {
    type Channel = broadcast::Sender<Self>;
}

// Terminal
impl Message<MainBus> for TerminalRecv {
    type Channel = mpsc::Sender<Self>;
}

impl Message<MainBus> for TerminalSend {
    type Channel = broadcast::Sender<Self>;
}

// impl Message<MainBus> for TerminalOutput {
//     type Channel = mpsc::Sender<Self>;
// }

// Tabs / Tab State
impl Message<MainBus> for TabRecv {
    type Channel = mpsc::Sender<Self>;
}

impl Message<MainBus> for TabState {
    type Channel = watch::Sender<Self>;
}

impl Message<MainBus> for Option<ActiveTabsState> {
    type Channel = watch::Sender<Self>;
}

impl Message<MainBus> for Option<WorkspaceState> {
    type Channel = watch::Sender<Self>;
}

// Websocket Messages
impl Message<MainBus> for Request {
    type Channel = mpsc::Sender<Self>;
}

impl Message<MainBus> for Response {
    type Channel = broadcast::Sender<Self>;
}

impl Resource<MainBus> for WebsocketResource {}

impl WebsocketMessageBus for MainBus {
    type Send = Request;
    type Recv = Response;
}
