use tab_api::{chunk::OutputChunk, tab::TabId};

use crate::{
    message::cli::CliSend, message::cli::CliSubscriptionRecv, message::cli::CliSubscriptionSend,
    prelude::*,
};
use anyhow::Context;

pub struct CliSubscriptionService {
    _rx: Lifeline,
}

impl Service for CliSubscriptionService {
    type Bus = CliBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let _rx = {
            let mut rx = bus.rx::<CliSubscriptionRecv>()?.log();
            let mut tx = bus.tx::<CliSubscriptionSend>()?;
            let mut tx_daemon = bus.tx::<CliSend>()?;

            Self::try_task("rx", async move {
                let mut state = SubscriptionState::None;
                while let Some(msg) = rx.recv().await {
                    match msg {
                        CliSubscriptionRecv::Subscribe(id) => {
                            if state.is_selected(id) {
                                debug!("Ignoring subscription request for {:?}", id);
                                continue;
                            }

                            info!("Subscribing to {:?}", id);

                            tx_daemon.send(CliSend::RequestScrollback(id)).await?;
                            state = SubscriptionState::AwaitingScrollback(id, Vec::new());
                        }
                        CliSubscriptionRecv::Unsubscribe(id) => {
                            if state.is_selected(id) {
                                info!("Unsubscribing from {:?}", id);
                                state = SubscriptionState::None;
                            }
                        }
                        CliSubscriptionRecv::Scrollback(scrollback) => {
                            if !state.is_selected(scrollback.id) {
                                continue;
                            }

                            if let SubscriptionState::AwaitingScrollback(id, buffer) = state {
                                let mut index = 0usize;

                                info!("Received scrollback for tab {}", id);

                                for chunk in scrollback.scrollback().await {
                                    index = Self::send_output(id, index, chunk, &mut tx).await?;
                                }

                                for chunk in buffer {
                                    index = Self::send_output(id, index, chunk, &mut tx).await?;
                                }

                                state = SubscriptionState::Selected(id, index);
                            }
                        }
                        CliSubscriptionRecv::Retask(from, to) => {
                            if state.is_selected(from) {
                                info!("Retasking subscription from {:?} to {:?}", from, to);

                                // if to is none, trigger a disconnect
                                if let None = to {
                                    state = SubscriptionState::None;
                                    tx.send(CliSubscriptionSend::Disconnect).await?;
                                    continue;
                                }

                                // otherwise, process the retask
                                let to = to.unwrap();

                                tx_daemon.send(CliSend::RequestScrollback(to)).await?;
                                tx.send(CliSubscriptionSend::Retask(to)).await?;

                                state = SubscriptionState::AwaitingScrollback(to, Vec::new());
                            }
                        }
                        CliSubscriptionRecv::Output(output) => {
                            if let SubscriptionState::AwaitingScrollback(id, ref mut buffer) = state
                            {
                                if id == output.id {
                                    let chunk = OutputChunk::clone(output.stdout.as_ref());
                                    buffer.push(chunk);
                                }
                            } else if let SubscriptionState::Selected(id, ref mut index) = state {
                                if id == output.id {
                                    let chunk = OutputChunk::clone(output.stdout.as_ref());
                                    *index = Self::send_output(id, *index, chunk, &mut tx).await?;
                                }
                            }
                        }
                        CliSubscriptionRecv::Stopped(id) => {
                            if !state.is_selected(id) {
                                continue;
                            }

                            tx.send(CliSubscriptionSend::Stopped(id)).await?;
                        }
                    }

                    debug!("subscription state: {:?}", &state);
                }
                Ok(())
            })
        };

        Ok(Self { _rx })
    }
}

impl CliSubscriptionService {
    async fn send_output(
        id: TabId,
        index: usize,
        mut chunk: OutputChunk,
        tx: &mut impl Sender<CliSubscriptionSend>,
    ) -> anyhow::Result<usize> {
        let end = chunk.end();

        if chunk.is_before(index) {
            debug!("ignoring chunk {:?} - before index: {:?}", &chunk, index);
            return Ok(index);
        }

        if chunk.index != index && chunk.contains(index) {
            debug!("truncating chunk {} at index {}", chunk.start(), index);
            chunk.truncate_before(index);
        }

        debug!(
            "tx subscription {}, idx {}..{}, len {}",
            id,
            chunk.start(),
            chunk.end(),
            chunk.data.len()
        );
        debug!("tx subscription {}, data: {}", id, chunk.to_string());

        let response = CliSubscriptionSend::Output(id, chunk);
        tx.send(response).await.context("tx_websocket closed")?;

        Ok(end)
    }
}

#[derive(Debug)]
enum SubscriptionState {
    None,
    AwaitingScrollback(TabId, Vec<OutputChunk>),
    Selected(TabId, usize),
}

impl SubscriptionState {
    pub fn is_selected(&self, tab: TabId) -> bool {
        match &self {
            SubscriptionState::None => false,
            SubscriptionState::AwaitingScrollback(id, _) => *id == tab,
            SubscriptionState::Selected(id, _) => *id == tab,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{
        message::cli::CliSend, message::cli::CliSubscriptionRecv,
        message::cli::CliSubscriptionSend, message::tab::TabOutput, message::tab::TabScrollback,
        prelude::*, service::pty::scrollback::ScrollbackBuffer, state::pty::PtyScrollback,
    };
    use lifeline::{assert_completes, assert_times_out};
    use tab_api::{chunk::OutputChunk, tab::TabId};
    use tokio::sync::Mutex;

    use super::CliSubscriptionService;

    async fn tx_subscribe(
        tx: &mut impl Sender<CliSubscriptionRecv>,
        tab: TabId,
    ) -> anyhow::Result<()> {
        tx.send(CliSubscriptionRecv::Subscribe(tab)).await?;

        Ok(())
    }

    async fn tx_empty_scrollback(
        tx: &mut impl Sender<CliSubscriptionRecv>,
        id: TabId,
    ) -> anyhow::Result<()> {
        let scrollback = TabScrollback {
            id,
            scrollback: PtyScrollback::new(Arc::new(Mutex::new(ScrollbackBuffer::new()))),
        };
        tx.send(CliSubscriptionRecv::Scrollback(scrollback)).await?;

        Ok(())
    }

    async fn tx_chunk(
        tx: &mut impl Sender<CliSubscriptionRecv>,
        id: TabId,
        index: usize,
        data: Vec<u8>,
    ) -> anyhow::Result<()> {
        let stdout = OutputChunk { index, data };

        let tab_output = TabOutput {
            id,
            stdout: Arc::new(stdout),
        };

        tx.send(CliSubscriptionRecv::Output(tab_output)).await?;

        Ok(())
    }

    #[tokio::test]
    async fn scrollback() -> anyhow::Result<()> {
        let bus = CliBus::default();
        let _service = CliSubscriptionService::spawn(&bus)?;

        let mut tx = bus.tx::<CliSubscriptionRecv>()?;
        let mut rx = bus.rx::<CliSubscriptionSend>()?;

        tx_subscribe(&mut tx, TabId(0)).await?;

        let scrollback = TabScrollback::empty(TabId(0));
        scrollback
            .push(OutputChunk {
                index: 1,
                data: vec![1, 2],
            })
            .await;

        tx.send(CliSubscriptionRecv::Scrollback(scrollback)).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(
                Some(CliSubscriptionSend::Output(
                    TabId(0),
                    OutputChunk {
                        index: 1,
                        data: vec![1, 2]
                    }
                )),
                msg
            );
        });

        Ok(())
    }

    #[tokio::test]
    async fn scrollback_ignored_unsubscribed() -> anyhow::Result<()> {
        let bus = CliBus::default();
        let _service = CliSubscriptionService::spawn(&bus)?;

        let mut tx = bus.tx::<CliSubscriptionRecv>()?;
        let mut rx = bus.rx::<CliSubscriptionSend>()?;

        let scrollback = TabScrollback::empty(TabId(0));
        scrollback
            .push(OutputChunk {
                index: 1,
                data: vec![1, 2],
            })
            .await;

        tx.send(CliSubscriptionRecv::Scrollback(scrollback)).await?;

        assert_times_out!(async {
            rx.recv().await;
        });

        Ok(())
    }

    #[tokio::test]
    async fn output() -> anyhow::Result<()> {
        let bus = CliBus::default();
        let _service = CliSubscriptionService::spawn(&bus)?;

        let mut tx = bus.tx::<CliSubscriptionRecv>()?;
        let mut rx = bus.rx::<CliSubscriptionSend>()?;

        tx_subscribe(&mut tx, TabId(0)).await?;
        tx_empty_scrollback(&mut tx, TabId(0)).await?;
        tx_chunk(&mut tx, TabId(0), 1, vec![1, 2]).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(
                Some(CliSubscriptionSend::Output(
                    TabId(0),
                    OutputChunk {
                        index: 1,
                        data: vec![1, 2]
                    }
                )),
                msg
            );
        });

        Ok(())
    }

    #[tokio::test]
    async fn output_repairs_overlap() -> anyhow::Result<()> {
        let bus = CliBus::default();
        let _service = CliSubscriptionService::spawn(&bus)?;

        let mut tx = bus.tx::<CliSubscriptionRecv>()?;
        let mut rx = bus.rx::<CliSubscriptionSend>()?;

        tx_subscribe(&mut tx, TabId(0)).await?;
        tx_empty_scrollback(&mut tx, TabId(0)).await?;
        tx_chunk(&mut tx, TabId(0), 1, vec![1, 2]).await?;
        tx_chunk(&mut tx, TabId(0), 2, vec![2, 3, 4]).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(
                Some(CliSubscriptionSend::Output(
                    TabId(0),
                    OutputChunk {
                        index: 1,
                        data: vec![1, 2]
                    }
                )),
                msg
            );

            let msg = rx.recv().await;
            assert_eq!(
                Some(CliSubscriptionSend::Output(
                    TabId(0),
                    OutputChunk {
                        index: 3,
                        data: vec![3, 4]
                    }
                )),
                msg
            );
        });

        Ok(())
    }

    #[tokio::test]
    async fn output_buffers_until_scrollback() -> anyhow::Result<()> {
        let bus = CliBus::default();
        let _service = CliSubscriptionService::spawn(&bus)?;

        let mut tx = bus.tx::<CliSubscriptionRecv>()?;
        let mut rx = bus.rx::<CliSubscriptionSend>()?;

        tx_subscribe(&mut tx, TabId(0)).await?;
        tx_chunk(&mut tx, TabId(0), 1, vec![1, 2]).await?;
        tx_empty_scrollback(&mut tx, TabId(0)).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(
                Some(CliSubscriptionSend::Output(
                    TabId(0),
                    OutputChunk {
                        index: 1,
                        data: vec![1, 2]
                    }
                )),
                msg
            );
        });

        Ok(())
    }

    #[tokio::test]
    async fn output_ignores_unsubscribed() -> anyhow::Result<()> {
        let bus = CliBus::default();
        let _service = CliSubscriptionService::spawn(&bus)?;

        let mut tx = bus.tx::<CliSubscriptionRecv>()?;
        let mut rx = bus.rx::<CliSubscriptionSend>()?;

        tx_chunk(&mut tx, TabId(0), 1, vec![1]).await?;

        assert_times_out!(async {
            rx.recv().await;
        });

        Ok(())
    }

    #[tokio::test]
    async fn retask() -> anyhow::Result<()> {
        let bus = CliBus::default();
        let _service = CliSubscriptionService::spawn(&bus)?;

        let mut tx = bus.tx::<CliSubscriptionRecv>()?;
        let mut rx = bus.rx::<CliSubscriptionSend>()?;
        let mut rx_daemon = bus.rx::<CliSend>()?;

        tx_subscribe(&mut tx, TabId(0)).await?;

        tx.send(CliSubscriptionRecv::Retask(TabId(0), Some(TabId(1))))
            .await?;

        assert_completes!(async {
            let msg = rx.recv().await;
            assert_eq!(Some(CliSubscriptionSend::Retask(TabId(1))), msg);
        });

        assert_completes!(async {
            let msg = rx_daemon.recv().await;
            assert_eq!(Some(CliSend::RequestScrollback(TabId(0))), msg);

            let msg = rx_daemon.recv().await;
            assert_eq!(Some(CliSend::RequestScrollback(TabId(1))), msg);
        });

        Ok(())
    }

    #[tokio::test]
    async fn disconnect() -> anyhow::Result<()> {
        let bus = CliBus::default();
        let _service = CliSubscriptionService::spawn(&bus)?;

        let mut tx = bus.tx::<CliSubscriptionRecv>()?;
        let mut rx = bus.rx::<CliSubscriptionSend>()?;
        let mut rx_daemon = bus.rx::<CliSend>()?;

        tx_subscribe(&mut tx, TabId(0)).await?;

        // request scrollback msg
        assert_completes!(async {
            rx_daemon.recv().await;
        });

        tx.send(CliSubscriptionRecv::Retask(TabId(0), None)).await?;

        assert_completes!(async {
            let msg = rx.recv().await;
            assert_eq!(Some(CliSubscriptionSend::Disconnect), msg);
        });

        assert_times_out!(async {
            rx_daemon.recv().await;
        });

        Ok(())
    }
}
