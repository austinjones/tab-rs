// mod session;
use crate::message::cli::{CliRecv, CliSend};
use crate::prelude::*;
use crate::state::tab::TabsState;
use anyhow::Context;
use lifeline::subscription;
use std::collections::HashMap;
use tab_api::{chunk::OutputChunk, client::InitResponse, tab::TabId};

use subscription::Subscription;
use time::Duration;
use tokio::{stream::StreamExt, time};

pub struct CliService {
    _run: Lifeline,
}

enum Event {
    Websocket(Request),
    Daemon(CliRecv),
}

impl Event {
    pub fn websocket(recv: Request) -> Self {
        Self::Websocket(recv)
    }

    pub fn daemon(recv: CliRecv) -> Self {
        Self::Daemon(recv)
    }
}

impl Service for CliService {
    type Bus = CliBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let rx_websocket = bus
            .rx::<Request>()?
            .into_inner()
            .filter(|r| r.is_ok())
            .map(|r| r.unwrap())
            .map(Event::websocket);
        let rx_daemon = bus.rx::<CliRecv>()?.map(Event::daemon);
        let mut rx = rx_websocket.merge(rx_daemon);

        let mut tx_websocket = bus.tx::<Response>()?;
        let mut tx_daemon = bus.tx::<CliSend>()?;
        let mut tx_subscription = bus.tx::<Subscription<TabId>>()?;
        let rx_subscription = bus.rx::<Subscription<TabId>>()?.into_inner();
        let mut rx_tabs_state = bus.rx::<TabsState>()?;

        let _run = Self::try_task("run", async move {
            let mut subscription_index: HashMap<usize, usize> = HashMap::new();

            let tabs = rx_tabs_state
                .recv()
                .await
                .ok_or_else(|| anyhow::Error::msg("rx TabsState closed"))?;

            let init = InitResponse {
                tabs: tabs.tabs.clone(),
            };
            let init = Response::Init(init);
            tx_websocket.send(init).await?;

            for tab in tabs.tabs.values() {
                debug!("notifying client of existing tab {}", &tab.name);
                let message = Response::TabUpdate(tab.clone());
                tx_websocket.send(message).await?;
            }

            while let Some(event) = rx.next().await {
                match event {
                    Event::Websocket(msg) => {
                        Self::recv_websocket(msg, &mut tx_subscription, &mut tx_daemon).await?
                    }
                    Event::Daemon(msg) => {
                        Self::recv_daemon(
                            msg,
                            &rx_subscription,
                            &mut tx_subscription,
                            &mut tx_websocket,
                            &mut tx_daemon,
                            &mut subscription_index,
                        )
                        .await?
                    }
                }
            }

            Ok(())
        });

        Ok(CliService { _run })
    }
}

impl CliService {
    async fn recv_websocket(
        request: Request,
        tx_subscription: &mut impl Sender<Subscription<TabId>>,
        tx_daemon: &mut impl Sender<CliSend>,
    ) -> anyhow::Result<()> {
        debug!("received Request: {:?}", &request);

        match request {
            Request::Subscribe(id) => {
                debug!("client subscribing to tab {}", id);
                tx_subscription
                    .send(Subscription::Subscribe(id))
                    .await
                    .context("tx_subscription closed")?;

                time::delay_for(Duration::from_millis(10)).await;

                tx_daemon.send(CliSend::RequestScrollback(id)).await?;
            }
            Request::Unsubscribe(id) => {
                tx_subscription
                    .send(Subscription::Unsubscribe(id))
                    .await
                    .context("tx_subscription closed")?;
            }
            Request::Input(id, stdin) => {
                let message = CliSend::Input(id, stdin);
                tx_daemon.send(message).await.context("tx_daemon closed")?;
            }
            Request::CreateTab(create) => {
                let message = CliSend::CreateTab(create);
                tx_daemon.send(message).await.context("tx_daemon closed")?;
            }
            Request::ResizeTab(id, dimensions) => {
                debug!("resizing tab {} to {:?}", id.0, dimensions);
                tx_daemon.send(CliSend::ResizeTab(id, dimensions)).await?;
            }
            Request::CloseTab(id) => {
                let message = CliSend::CloseTab(id);
                tx_daemon.send(message).await.context("tx_daemon closed")?;
            }
            Request::CloseNamedTab(name) => {
                let message = CliSend::CloseNamedTab(name);
                tx_daemon.send(message).await.context("tx_daemon closed")?;
            }
            Request::Retask(id, name) => {
                // we need to send this along so other attached tabs get retasked
                let message = CliSend::Retask(id, name);
                tx_daemon.send(message).await?;
            }
            Request::GlobalShutdown => {
                tx_daemon.send(CliSend::GlobalShutdown).await?;
            }
        }

        Ok(())
    }

    async fn recv_daemon(
        msg: CliRecv,
        rx_subscription: &subscription::Receiver<TabId>,
        tx_subscription: &mut impl Sender<Subscription<TabId>>,
        tx_websocket: &mut impl Sender<Response>,
        tx_daemon: &mut impl Sender<CliSend>,
        subscription_index: &mut HashMap<usize, usize>,
    ) -> anyhow::Result<()> {
        trace!("message from daemon: {:?}", &msg);
        match msg {
            CliRecv::TabStarted(metadata) => {
                tx_websocket
                    .send(Response::TabUpdate(metadata))
                    .await
                    .context("tx_websocket closed")?;
            }
            CliRecv::Scrollback(message) => {
                if let Some(_identifier) = rx_subscription.get_identifier(&message.id) {
                    info!("processing scrollback for tab {}", message.id);

                    let subscription_id = rx_subscription.get_identifier(&message.id).unwrap();

                    for chunk in message.scrollback().await {
                        let _index = chunk.index;
                        Self::send_output(
                            message.id,
                            subscription_id,
                            chunk,
                            tx_websocket,
                            subscription_index,
                        )
                        .await?;
                    }
                }
            }
            // TODO: this way of handling scrollback isn't perfect
            // if the terminal is generating output, the scrollback may arrive too late.
            // the historical channel would fix this, but it'd also destory some of the tokio::broadcast goodness w/ TabId
            CliRecv::Output(id, chunk) => {
                if let Some(identifier) = rx_subscription.get_identifier(&id) {
                    Self::send_output(id, identifier, chunk, tx_websocket, subscription_index)
                        .await?;
                }
            }
            CliRecv::TabStopped(id) => {
                info!("notifying client of stopped tab: {}", id);
                tx_websocket
                    .send(Response::TabTerminated(id))
                    .await
                    .context("tx_websocket closed")?;
            }
            CliRecv::Retask(from, to) => {
                info!("acknowledging retask from {:?} to {:?}, updating subscriptions & requesting scrollback", from, to);

                tx_websocket.send(Response::Retask(to)).await?;
                time::delay_for(Duration::from_millis(10)).await;

                tx_subscription
                    .send(Subscription::Unsubscribe(from))
                    .await?;
                tx_subscription.send(Subscription::Subscribe(to)).await?;
                time::delay_for(Duration::from_millis(10)).await;

                tx_daemon.send(CliSend::RequestScrollback(to)).await?;
            }
        }
        Ok(())
    }

    async fn send_output(
        id: TabId,
        subscription_id: usize,
        chunk: OutputChunk,
        tx_websocket: &mut impl Sender<Response>,
        subscription_index: &mut HashMap<usize, usize>,
    ) -> anyhow::Result<()> {
        let index = chunk.index;

        if let Some(sub_index) = subscription_index.get(&subscription_id) {
            if index <= *sub_index {
                return Ok(());
            }
        }

        debug!(
            "tx subscription {}, chunk {}, len {}",
            subscription_id,
            chunk.index,
            chunk.data.len()
        );

        let response = Response::Output(id, chunk);
        tx_websocket
            .send(response)
            .await
            .context("tx_websocket closed")?;

        subscription_index.insert(subscription_id, index);

        Ok(())
    }
}
