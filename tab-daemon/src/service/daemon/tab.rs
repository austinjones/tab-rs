use crate::prelude::*;
use crate::{
    message::tab::{TabOutput, TabRecv, TabScrollback, TabSend},
    pty_process::{PtyOptions, PtyProcess, PtyReceiver, PtyRequest, PtySender},
};
use anyhow::Context;
use lifeline::Task;
use lifeline::{Bus, Lifeline, Service};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tab_api::{chunk::InputChunk, tab::TabId};
use tokio::{
    stream::StreamExt,
    sync::{broadcast, mpsc},
};

pub struct TabService {
    pub id: TabId,
    _run: Lifeline,
}

static TAB_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

impl Service for TabService {
    type Bus = TabBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let id = TAB_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
        let id = TabId(id as u16);

        let rx = bus.rx::<TabRecv>()?;
        let tx = bus.tx::<TabSend>()?;

        let _run = Self::try_task("run", async move {
            Self::run(id, rx, tx).await.context(format!("{}", id))
        });

        Ok(Self { id, _run })
    }
}

impl TabService {
    #[allow(unreachable_code)]
    async fn run(
        id: TabId,
        mut rx: broadcast::Receiver<TabRecv>,
        mut tx: mpsc::Sender<TabSend>,
    ) -> anyhow::Result<()> {
        let mut sender = None;
        let mut echoes = vec![];
        while let Some(msg) = rx.next().await {
            if msg.is_err() {
                continue;
            }

            let msg = msg.unwrap();

            match msg {
                TabRecv::Init(create) => {
                    if id != create.id {
                        continue;
                    }

                    let options = PtyOptions {
                        dimensions: create.dimensions,
                        command: "bash".to_string(),
                    };

                    let (send, recv) = PtyProcess::spawn(options).await?;
                    sender = Some(send);

                    let lifeline = Self::try_task("echo", Self::stdout(id, recv, tx.clone()));
                    echoes.push(lifeline);

                    info!("tab {} initialized, name {}", id, create.name.as_str());

                    tx.send(TabSend::Started(create)).await?;

                    Self::send_scrollback(id, sender.as_ref(), &mut tx).await?;
                }
                TabRecv::Scrollback(id) => {
                    Self::send_scrollback(id, sender.as_ref(), &mut tx).await?
                }
                TabRecv::Input(input) => {
                    if id != input.id {
                        continue;
                    }

                    if let Some(ref mut pty) = sender {
                        let message = PtyRequest::Input(InputChunk::clone(input.stdin.as_ref()));

                        pty.send(message)
                            .await
                            .map_err(|_e| anyhow::Error::msg("send PtyRequest::Input"))?;
                    }
                }
            }
        }

        Ok(())
    }

    async fn send_scrollback(
        id: TabId,
        pty: Option<&PtySender>,
        tx: &mut mpsc::Sender<TabSend>,
    ) -> anyhow::Result<()> {
        if let Some(sender) = pty {
            let scrollback = TabScrollback {
                id,
                scrollback: sender.scrollback().await,
            };

            let message = TabSend::Scrollback(scrollback);

            tx.send(message).await?;

            debug!("sent scrollback");
        } else {
            error!("scrollback requested before init");
        }

        Ok(())
    }

    async fn stdout(
        id: TabId,
        mut rx: PtyReceiver,
        mut tx: mpsc::Sender<TabSend>,
    ) -> anyhow::Result<()> {
        loop {
            let msg = rx.recv().await?;
            match msg {
                crate::pty_process::PtyResponse::Output(out) => {
                    let stdout = Arc::new(out);
                    let output = TabOutput { id, stdout };

                    tx.send(TabSend::Output(output)).await?;
                }
                crate::pty_process::PtyResponse::Terminated(_term) => {
                    tx.send(TabSend::Stopped(id)).await?;
                }
            }
        }
    }
}
