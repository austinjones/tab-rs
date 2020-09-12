use crate::prelude::*;
use crate::{
    message::{
        pty::{PtyRecv, PtySend, PtyShutdown},
        tab::{TabOutput, TabRecv, TabScrollback, TabSend},
        tab_manager::TabManagerRecv,
    },
    state::pty::{PtyScrollback, PtyState},
};

use std::sync::Arc;

use lifeline::Resource;
use tab_api::{
    chunk::InputChunk,
    pty::{PtyWebsocketRequest, PtyWebsocketResponse},
    tab::TabMetadata,
};
use tab_websocket::{bus::WebsocketMessageBus, resource::connection::WebsocketResource};
use tokio::sync::{broadcast, mpsc, watch};

lifeline_bus!(pub struct PtyBus);

impl Message<PtyBus> for PtyShutdown {
    type Channel = mpsc::Sender<Self>;
}

impl Message<PtyBus> for PtyWebsocketRequest {
    type Channel = mpsc::Sender<Self>;
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

impl CarryFrom<ListenerBus> for PtyBus {
    type Lifeline = anyhow::Result<ListenerPtyCarrier>;

    fn carry_from(&self, from: &ListenerBus) -> Self::Lifeline {
        // converts TabRecv into PtyRecv
        // forwards input and output chunks
        // receives startup and shutdown signals

        let _to_pty = {
            let rx_id = self.rx::<PtyState>()?.into_inner();
            // FIXME I think the bug is here.
            // the channel is being taken from the
            let mut rx_tab = from.rx::<TabRecv>()?;

            let mut tx_pty = self.tx::<PtyRecv>()?;
            let mut tx_pty_state = self.tx::<PtyState>()?;

            Self::try_task("to_pty", async move {
                while let Some(msg) = rx_tab.recv().await {
                    match msg {
                        TabRecv::Assign(offer) => {
                            if rx_id.borrow().is_assigned() {
                                continue;
                            }

                            if let Some(assignment) = offer.accept() {
                                info!("New PTY connected on tab {}", assignment.id);

                                tx_pty_state.send(PtyState::Assigned(assignment.id)).await?;
                                tx_pty.send(PtyRecv::Init(assignment)).await?;
                            }
                        }
                        TabRecv::Scrollback(id) => {
                            if !rx_id.borrow().has_assigned(id) {
                                continue;
                            }

                            tx_pty.send(PtyRecv::Scrollback).await?;
                        }
                        TabRecv::Input(input) => {
                            if !rx_id.borrow().has_assigned(input.id) {
                                continue;
                            }

                            let input: InputChunk = (*input.stdin).clone();
                            tx_pty.send(PtyRecv::Input(input)).await?;
                        }
                        TabRecv::Terminate(id) => {
                            if !rx_id.borrow().has_assigned(id) {
                                continue;
                            }

                            tx_pty.send(PtyRecv::Terminate).await?;
                        }
                        TabRecv::Resize(id, dimensions) => {
                            if !rx_id.borrow().has_assigned(id) {
                                continue;
                            }

                            tx_pty.send(PtyRecv::Resize(dimensions)).await?;
                        }
                        TabRecv::Retask(_, _) => {}
                        TabRecv::TerminateAll => {
                            tx_pty.send(PtyRecv::Terminate).await?;
                        }
                    }
                }

                Ok(())
            })
        };

        let _to_listener = {
            let rx_id = self.rx::<PtyState>()?.into_inner();
            let mut rx_pty = self.rx::<PtySend>()?;

            let mut tx_tab = from.tx::<TabSend>()?;
            let mut tx_tab_manager = from.tx::<TabManagerRecv>()?;

            Self::try_task("to_listener", async move {
                while let Some(msg) = rx_pty.recv().await {
                    match msg {
                        PtySend::Started(metadata) => {
                            let message = TabSend::Started(metadata);
                            tx_tab.send(message).await?;
                        }
                        PtySend::Output(chunk) => {
                            let id = rx_id.borrow().unwrap();

                            let output = TabOutput {
                                id,
                                stdout: Arc::new(chunk),
                            };

                            let send = TabSend::Output(output);
                            tx_tab.send(send).await?;
                        }
                        PtySend::Scrollback(scrollback) => {
                            let id = rx_id.borrow().unwrap();
                            let scrollback = TabScrollback { id, scrollback };
                            let message = TabSend::Scrollback(scrollback);
                            tx_tab.send(message).await?;
                        }
                        PtySend::Stopped => {
                            let id = rx_id.borrow().unwrap();
                            // todo - this should be a notification, not an action
                            // serious bugs were going on because this was missing, though.
                            tx_tab_manager.send(TabManagerRecv::CloseTab(id)).await?;
                            tx_tab.send(TabSend::Stopped(id)).await?;
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

#[cfg(test)]
mod to_pty_tests {}

#[cfg(test)]
mod to_listener_tests {}
