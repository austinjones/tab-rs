use crate::prelude::*;
use crate::{
    message::pty::PtyShutdown,
    pty_process::{PtyOptions, PtyProcess, PtyReceiver, PtyRequest},
};

use error::into_msg;
use lifeline::Task;
use lifeline::{Bus, Lifeline, Service};

use tab_api::{
    pty::{PtyWebsocketRequest, PtyWebsocketResponse},
    tab::TabId,
};
use tokio::{
    stream::StreamExt,
    sync::{broadcast, mpsc},
};

pub struct PtyService {
    _run: Lifeline,
}

impl Service for PtyService {
    type Bus = PtyBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let _run = {
            let rx = bus.rx::<PtyWebsocketRequest>()?;
            let tx = bus.tx::<PtyWebsocketResponse>()?;
            let tx_shutdown = bus.tx::<PtyShutdown>()?;
            Self::try_task("run", Self::run(rx, tx, tx_shutdown))
        };

        Ok(Self { _run })
    }
}

impl PtyService {
    async fn run(
        mut rx: broadcast::Receiver<PtyWebsocketRequest>,
        tx: broadcast::Sender<PtyWebsocketResponse>,
        mut tx_shutdown: mpsc::Sender<PtyShutdown>,
    ) -> anyhow::Result<()> {
        let mut sender = None;
        let mut _echo = None;
        while let Some(msg) = rx.next().await {
            if let Err(_e) = msg {
                continue;
            }

            let msg = msg.unwrap();

            match msg {
                PtyWebsocketRequest::Init(create) => {
                    debug!("initializing on tab {}", create.id);
                    let name = create.name.clone();

                    let options = PtyOptions {
                        dimensions: create.dimensions,
                        command: "bash".to_string(),
                    };

                    let (send, recv) = PtyProcess::spawn(options).await?;

                    _echo = Some(Self::try_task(
                        "echo",
                        Self::output(create.id, recv, tx.clone()),
                    ));
                    sender = Some(send);

                    info!("tab initialized, name {}", name);
                    tx.send(PtyWebsocketResponse::Started(create))
                        .map_err(into_msg)?;
                }
                PtyWebsocketRequest::Input(input) => {
                    if let Some(ref mut pty) = sender {
                        let message = PtyRequest::Input(input);

                        pty.send(message)
                            .await
                            .map_err(|_e| anyhow::Error::msg("send PtyRequest::Input"))?;
                    }
                }
                PtyWebsocketRequest::Terminate => {
                    tx_shutdown.send(PtyShutdown {}).await;
                }
            }
        }

        Ok(())
    }

    async fn output(
        _id: TabId,
        mut rx: PtyReceiver,
        tx: broadcast::Sender<PtyWebsocketResponse>,
    ) -> anyhow::Result<()> {
        loop {
            let msg = rx.recv().await?;
            match msg {
                crate::pty_process::PtyResponse::Output(out) => {
                    tx.send(PtyWebsocketResponse::Output(out))
                        .map_err(into_msg)?;
                }
                crate::pty_process::PtyResponse::Terminated(_term) => {
                    tx.send(PtyWebsocketResponse::Stopped).map_err(into_msg)?;
                }
            }
        }
    }
}
