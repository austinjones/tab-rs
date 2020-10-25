pub mod scrollback;

// mod session;

use std::time::Duration;

use crate::message::pty::{PtyRecv, PtySend, PtyShutdown};
use crate::prelude::*;

use tab_api::pty::{PtyWebsocketRequest, PtyWebsocketResponse};

use scrollback::PtyScrollbackService;
use tokio::time;

/// Drives an active tab-pty connection, forwarding events between the daemon and the websocket
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
            let mut rx_websocket = bus.rx::<PtyWebsocketResponse>()?.log();
            let mut tx_daemon = bus.tx::<PtySend>()?;
            let mut tx_shutdown = bus.tx::<PtyShutdown>()?;

            Self::try_task("websocket", async move {
                while let Some(msg) = rx_websocket.recv().await {
                    match msg {
                        PtyWebsocketResponse::Started(metadata) => {
                            info!("PTY process has started on tab {}", metadata.id);
                            tx_daemon.send(PtySend::Started(metadata)).await?;
                        }
                        PtyWebsocketResponse::Output(output) => {
                            tx_daemon.send(PtySend::Output(output)).await?;
                        }
                        PtyWebsocketResponse::Stopped => {
                            info!("PTY process has terminated");
                            tx_daemon.send(PtySend::Stopped).await?;

                            // this sleep is not visible to the user
                            time::delay_for(Duration::from_millis(500)).await;
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

            Self::try_task("daemon", async move {
                while let Some(msg) = rx_daemon.recv().await {
                    match msg {
                        PtyRecv::Init(init) => {
                            info!("PTY initialized on tab {}", init.id);
                            let message = PtyWebsocketRequest::Init(init);
                            tx_websocket.send(message).await?;
                        }
                        PtyRecv::Input(input) => {
                            let message = PtyWebsocketRequest::Input(input);
                            tx_websocket.send(message).await?;
                        }
                        PtyRecv::Resize(dimensions) => {
                            debug!("resizing pty to {:?}", &dimensions);
                            let message = PtyWebsocketRequest::Resize(dimensions);
                            tx_websocket.send(message).await?;
                        }
                        PtyRecv::Terminate => {
                            info!("PTY process terminating due to user request");
                            tx_websocket.send(PtyWebsocketRequest::Terminate).await?;
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

#[cfg(test)]
mod websocket_tests {
    use super::PtyService;
    use crate::{
        bus::PtyBus,
        message::pty::{PtySend, PtyShutdown},
    };
    use lifeline::{assert_completes, assert_times_out, Bus, Receiver, Sender, Service};
    use std::collections::HashMap;
    use tab_api::{
        chunk::OutputChunk,
        pty::PtyWebsocketResponse,
        tab::{TabId, TabMetadata},
    };

    #[tokio::test]
    async fn started() -> anyhow::Result<()> {
        let bus = PtyBus::default();
        let _service = PtyService::spawn(&bus)?;

        let mut tx = bus.tx::<PtyWebsocketResponse>()?;
        let mut rx = bus.rx::<PtySend>()?;

        let tab = TabMetadata {
            id: TabId(0),
            name: "name".into(),
            doc: Some("doc".into()),
            dimensions: (1, 2),
            env: HashMap::new(),
            shell: "shell".into(),
            dir: "/".into(),
        };
        tx.send(PtyWebsocketResponse::Started(tab.clone())).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(PtySend::Started(tab)), msg);
        });

        Ok(())
    }

    #[tokio::test]
    async fn output() -> anyhow::Result<()> {
        let bus = PtyBus::default();
        let _service = PtyService::spawn(&bus)?;

        let mut tx = bus.tx::<PtyWebsocketResponse>()?;
        let mut rx = bus.rx::<PtySend>()?;

        let output = OutputChunk {
            index: 0,
            data: vec![1, 2],
        };
        tx.send(PtyWebsocketResponse::Output(output.clone()))
            .await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(PtySend::Output(output)), msg);
        });

        Ok(())
    }

    #[tokio::test]
    async fn stopped() -> anyhow::Result<()> {
        let bus = PtyBus::default();
        let _service = PtyService::spawn(&bus)?;

        let mut tx = bus.tx::<PtyWebsocketResponse>()?;
        let mut rx = bus.rx::<PtySend>()?;
        let mut rx_shutdown = bus.rx::<PtyShutdown>()?;

        tx.send(PtyWebsocketResponse::Stopped).await?;

        assert_completes!(
            async move {
                let msg = rx.recv().await;
                assert_eq!(Some(PtySend::Stopped), msg);

                let _shutdown_msg = rx_shutdown.recv().await;
            },
            750
        );

        Ok(())
    }

    #[tokio::test]
    async fn stopped_terminates() -> anyhow::Result<()> {
        let bus = PtyBus::default();
        let _service = PtyService::spawn(&bus)?;

        let mut tx = bus.tx::<PtyWebsocketResponse>()?;
        let mut rx = bus.rx::<PtySend>()?;

        tx.send(PtyWebsocketResponse::Stopped).await?;

        assert_completes!(async {
            let msg = rx.recv().await;
            assert_eq!(Some(PtySend::Stopped), msg);
        });

        assert_times_out!(async {
            let _no_msg = rx.recv().await;
        });

        Ok(())
    }
}

#[cfg(test)]
mod daemon_tests {
    use super::PtyService;
    use crate::{bus::PtyBus, message::pty::PtyRecv};
    use lifeline::{assert_completes, Bus, Receiver, Sender, Service};
    use std::collections::HashMap;
    use tab_api::{
        chunk::InputChunk,
        pty::PtyWebsocketRequest,
        tab::{TabId, TabMetadata},
    };

    #[tokio::test]
    async fn init() -> anyhow::Result<()> {
        let bus = PtyBus::default();
        let _service = PtyService::spawn(&bus)?;

        let mut tx = bus.tx::<PtyRecv>()?;
        let mut rx = bus.rx::<PtyWebsocketRequest>()?;

        let tab = TabMetadata {
            id: TabId(0),
            name: "name".into(),
            doc: Some("doc".into()),
            dimensions: (1, 2),
            env: HashMap::new(),
            shell: "shell".into(),
            dir: "/".into(),
        };
        tx.send(PtyRecv::Init(tab.clone())).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(PtyWebsocketRequest::Init(tab)), msg);
        });

        Ok(())
    }

    #[tokio::test]
    async fn input() -> anyhow::Result<()> {
        let bus = PtyBus::default();
        let _service = PtyService::spawn(&bus)?;

        let mut tx = bus.tx::<PtyRecv>()?;
        let mut rx = bus.rx::<PtyWebsocketRequest>()?;

        let input = InputChunk { data: vec![1, 2] };
        tx.send(PtyRecv::Input(input.clone())).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(PtyWebsocketRequest::Input(input)), msg);
        });

        Ok(())
    }

    #[tokio::test]
    async fn resize() -> anyhow::Result<()> {
        let bus = PtyBus::default();
        let _service = PtyService::spawn(&bus)?;

        let mut tx = bus.tx::<PtyRecv>()?;
        let mut rx = bus.rx::<PtyWebsocketRequest>()?;

        tx.send(PtyRecv::Resize((2, 3))).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(PtyWebsocketRequest::Resize((2, 3))), msg);
        });

        Ok(())
    }

    #[tokio::test]
    async fn terminate() -> anyhow::Result<()> {
        let bus = PtyBus::default();
        let _service = PtyService::spawn(&bus)?;

        let mut tx = bus.tx::<PtyRecv>()?;
        let mut rx = bus.rx::<PtyWebsocketRequest>()?;

        tx.send(PtyRecv::Terminate).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(PtyWebsocketRequest::Terminate), msg);
        });

        Ok(())
    }
}
