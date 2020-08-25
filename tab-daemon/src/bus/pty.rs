use crate::prelude::*;
use crate::{
    message::{
        pty::{PtyRecv, PtySend, PtyShutdown},
        tab::{TabOutput, TabRecv, TabScrollback, TabSend},
    },
    state::{
        pty::{PtyScrollback, PtyState},
        tab::TabsState,
    },
};

use std::sync::Arc;

use tab_api::{
    pty::{PtyWebsocketRequest, PtyWebsocketResponse},
    tab::{TabId, TabMetadata},
};
use tab_websocket::{bus::WebsocketMessageBus, resource::connection::WebsocketResource};
use tokio::{
    stream::StreamExt,
    sync::{broadcast, mpsc, watch},
};

lifeline_bus!(pub struct PtyBus);

impl Message<PtyBus> for PtyShutdown {
    type Channel = mpsc::Sender<Self>;
}

impl Message<PtyBus> for PtyWebsocketRequest {
    type Channel = broadcast::Sender<Self>;
}

impl Message<PtyBus> for PtyWebsocketResponse {
    type Channel = broadcast::Sender<Self>;
}

impl Message<PtyBus> for PtyRecv {
    type Channel = broadcast::Sender<Self>;
}

impl Message<PtyBus> for PtySend {
    type Channel = broadcast::Sender<Self>;
}

impl Message<PtyBus> for PtyState {
    type Channel = watch::Sender<Self>;
}

impl Message<PtyBus> for PtyScrollback {
    type Channel = mpsc::Sender<Self>;
}

impl Message<PtyBus> for TabMetadata {
    type Channel = mpsc::Sender<Self>;
}

impl Resource<PtyBus> for WebsocketResource {}
impl WebsocketMessageBus for PtyBus {
    type Send = PtyWebsocketRequest;
    type Recv = PtyWebsocketResponse;
}

pub struct ListenerPtyCarrier {
    _to_pty: Lifeline,
    _to_listener: Lifeline,
}

impl FromCarrier<ListenerBus> for PtyBus {
    type Lifeline = anyhow::Result<ListenerPtyCarrier>;

    fn carry_from(&self, from: &ListenerBus) -> Self::Lifeline {
        // converts TabRecv into PtyRecv
        // forwards input and output chunks
        // receives startup and shutdown signals

        let _to_pty = {
            let rx_id = self.rx::<PtyState>()?;
            let mut rx_tab = from.rx::<TabRecv>()?;

            let tx_pty = self.tx::<PtyRecv>()?;
            let tx_pty_state = self.tx::<PtyState>()?;

            Self::try_task("to_pty", async move {
                while let Some(msg) = rx_tab.next().await {
                    if let Err(_e) = msg {
                        continue;
                    }

                    match msg.unwrap() {
                        TabRecv::Assign(offer) => {
                            if rx_id.borrow().is_assigned() {
                                continue;
                            }

                            if let Some(assignment) = offer.take() {
                                tx_pty_state
                                    .broadcast(PtyState::Assigned(assignment.id))
                                    .map_err(into_msg)?;
                                tx_pty.send(PtyRecv::Init(assignment)).map_err(into_msg)?;
                            }
                        }
                        TabRecv::Scrollback(id) => {
                            if !rx_id.borrow().has_assigned(id) {
                                continue;
                            }

                            tx_pty.send(PtyRecv::Scrollback).map_err(into_msg)?;
                        }
                        TabRecv::Input(input) => {
                            if !rx_id.borrow().has_assigned(input.id) {
                                continue;
                            }

                            tx_pty.send(PtyRecv::Input(input)).map_err(into_msg)?;
                        }
                        TabRecv::Terminate(id) => {
                            if !rx_id.borrow().has_assigned(id) {
                                continue;
                            }

                            tx_pty.send(PtyRecv::Terminate).map_err(into_msg)?;
                        }
                    }
                }

                Ok(())
            })
        };

        let _to_listener = {
            let rx_id = self.rx::<PtyState>()?;
            let mut rx_pty = self.rx::<PtySend>()?;

            let tx_tab = from.tx::<TabSend>()?;

            Self::try_task("to_listener", async move {
                while let Some(msg) = rx_pty.next().await {
                    if let Err(_e) = msg {
                        continue;
                    }

                    match msg.unwrap() {
                        PtySend::Started(metadata) => {
                            let message = TabSend::Started(metadata);
                            tx_tab.send(message).map_err(into_msg)?;
                        }
                        PtySend::Output(chunk) => {
                            let id = rx_id.borrow().unwrap();

                            let output = TabOutput {
                                id,
                                stdout: Arc::new(chunk),
                            };

                            let send = TabSend::Output(output);
                            tx_tab.send(send).map_err(into_msg)?;
                        }
                        PtySend::Scrollback(scrollback) => {
                            let id = rx_id.borrow().unwrap();
                            let scrollback = TabScrollback { id, scrollback };
                            let message = TabSend::Scrollback(scrollback);
                            tx_tab.send(message).map_err(into_msg)?;
                        }
                        PtySend::Stopped => {
                            let id = rx_id.borrow().unwrap();
                            tx_tab.send(TabSend::Stopped(id)).map_err(into_msg)?;
                        }
                    }
                }

                Ok(())
            })
        };

        Ok(ListenerPtyCarrier {
            _to_pty,
            _to_listener,
        })
    }
}
