pub mod scrollback;

// mod session;

use crate::message::pty::{PtyRecv, PtySend, PtyShutdown};
use crate::prelude::*;

use tab_api::chunk::InputChunk;
use tab_api::pty::{PtyWebsocketRequest, PtyWebsocketResponse};

use tokio::stream::StreamExt;

use scrollback::PtyScrollbackService;

pub struct PtyService {
    _websocket: Lifeline,
    _daemon: Lifeline,
    _scrollback: PtyScrollbackService,
}

impl Service for PtyService {
    type Bus = PtyBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        // rx/tx from websocket
        // keep track of current tab
        // notify the tab manager of status

        let _websocket = {
            let mut rx_websocket = bus
                .rx::<PtyWebsocketResponse>()?
                .into_inner()
                .filter(|e| e.is_ok())
                .map(|e| e.unwrap());
            let mut tx_daemon = bus.tx::<PtySend>()?;
            let mut tx_shutdown = bus.tx::<PtyShutdown>()?;

            Self::try_task("websocket", async move {
                while let Some(msg) = rx_websocket.next().await {
                    match msg {
                        PtyWebsocketResponse::Started(metadata) => {
                            tx_daemon.send(PtySend::Started(metadata)).await?;
                        }
                        PtyWebsocketResponse::Output(output) => {
                            tx_daemon.send(PtySend::Output(output)).await?;
                        }
                        PtyWebsocketResponse::Stopped => {
                            tx_daemon.send(PtySend::Stopped).await?;
                            tx_shutdown.send(PtyShutdown {}).await?;
                            break;
                        }
                    }
                }

                Ok(())
            })
        };

        let _daemon = {
            let mut rx_daemon = bus.rx::<PtyRecv>()?;
            let mut tx_websocket = bus.tx::<PtyWebsocketRequest>()?;
            let mut tx_shutdown = bus.tx::<PtyShutdown>()?;

            Self::try_task("daemon", async move {
                while let Some(msg) = rx_daemon.recv().await {
                    match msg {
                        PtyRecv::Init(init) => {
                            let message = PtyWebsocketRequest::Init(init);
                            tx_websocket.send(message).await?;
                        }
                        PtyRecv::Input(input) => {
                            let input: InputChunk = (*input.stdin).clone();
                            let message = PtyWebsocketRequest::Input(input);
                            tx_websocket.send(message).await?;
                        }
                        PtyRecv::Resize(dimensions) => {
                            debug!("resizing pty to {:?}", &dimensions);
                            let message = PtyWebsocketRequest::Resize(dimensions);
                            tx_websocket.send(message).await?;
                        }
                        PtyRecv::Terminate => {
                            tx_websocket.send(PtyWebsocketRequest::Terminate).await?;

                            tx_shutdown.send(PtyShutdown {}).await?;
                            break;
                        }
                        PtyRecv::Scrollback => {}
                    }
                }

                Ok(())
            })
        };

        let _scrollback = PtyScrollbackService::spawn(bus)?;

        Ok(PtyService {
            _websocket,
            _daemon,
            _scrollback,
        })
    }
}
