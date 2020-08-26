use crate::prelude::*;
use crate::{
    message::pty::PtyShutdown,
    pty_process::{PtyOptions, PtyProcess, PtyReceiver, PtyRequest},
};

use std::collections::HashMap;
use tab_api::{
    pty::{PtyWebsocketRequest, PtyWebsocketResponse},
    tab::TabId,
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
        mut rx: impl Receiver<PtyWebsocketRequest>,
        mut tx: impl Sender<PtyWebsocketResponse> + Clone + Send + 'static,
        mut tx_shutdown: impl Sender<PtyShutdown> + Clone + Send + 'static,
    ) -> anyhow::Result<()> {
        let mut sender = None;
        let mut _echo = None;
        while let Some(msg) = rx.recv().await {
            match msg {
                PtyWebsocketRequest::Init(create) => {
                    debug!("initializing on tab {}", create.id);
                    let name = create.name.clone();

                    let mut env = HashMap::new();
                    env.insert("SHELL".to_string(), create.shell.clone());
                    env.insert("TAB".to_string(), create.name.clone());
                    env.insert("TAB_ID".to_string(), create.id.0.to_string());

                    let options = PtyOptions {
                        dimensions: create.dimensions,
                        command: create.shell.clone(),
                        env,
                    };

                    let (send, recv) = PtyProcess::spawn(options).await?;

                    _echo = Some(Self::try_task(
                        "echo",
                        Self::output(create.id, recv, tx.clone(), tx_shutdown.clone()),
                    ));
                    sender = Some(send);

                    info!("tab initialized, name {}", name);
                    tx.send(PtyWebsocketResponse::Started(create)).await?;
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
                    tx_shutdown.send(PtyShutdown {}).await?;
                }
                PtyWebsocketRequest::Resize(dimensions) => {
                    debug!("received resize request: {:?}", dimensions);
                    if let Some(ref mut sender) = sender {
                        sender.send(PtyRequest::Resize(dimensions)).await?;
                    }
                }
            }
        }

        Ok(())
    }

    async fn output(
        _id: TabId,
        mut rx: PtyReceiver,
        mut tx: impl Sender<PtyWebsocketResponse>,
        mut tx_shutdown: impl Sender<PtyShutdown>,
    ) -> anyhow::Result<()> {
        loop {
            let msg = rx.recv().await?;
            match msg {
                crate::pty_process::PtyResponse::Output(out) => {
                    tx.send(PtyWebsocketResponse::Output(out)).await?;
                }
                crate::pty_process::PtyResponse::Terminated(_term) => {
                    tx.send(PtyWebsocketResponse::Stopped).await?;
                    tx_shutdown.send(PtyShutdown {}).await?;
                }
            }
        }
    }
}
