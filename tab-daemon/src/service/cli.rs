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

/// Drives an active connection from the tab-command client, and forwards messages between the websocket and the daemon.
/// Tracks the client tab subscriptions, and filters messages received from the daemon.
pub struct CliService {
    _init: Lifeline,
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
        let _init = {
            let mut tx_websocket = bus.tx::<Response>()?;
            let mut rx_tabs_state = bus.rx::<TabsState>()?;

            Self::try_task("init", async move {
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

                Ok(())
            })
        };

        let _run = {
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
            Self::try_task("run", async move {
                let mut subscription_index: HashMap<usize, usize> = HashMap::new();

                debug!("cli connection waiting for messages");
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
            })
        };

        Ok(CliService { _init, _run })
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
                debug!("client subscribing from tab {}", id);
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
                if let Some(subscription_id) = rx_subscription.get_identifier(&message.id) {
                    info!("processing scrollback for tab {}", message.id);

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

#[cfg(test)]
mod request_tests {
    use super::CliService;
    use crate::{bus::CliBus, message::cli::CliSend, state::tab::TabsState};
    use lifeline::{assert_completes, subscription::Subscription, Bus, Receiver, Sender, Service};
    use std::collections::HashMap;
    use tab_api::{
        chunk::InputChunk,
        client::{InitResponse, Request, Response},
        tab::{CreateTabMetadata, TabId, TabMetadata},
    };
    use tokio::time;

    #[tokio::test]
    async fn init() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();

        // create an existing tab, then spawn the connection
        let mut tx = cli_bus.tx::<TabsState>()?;
        let mut tabs = TabsState::default();
        let tab_id = TabId(0);
        let tab_metadata = TabMetadata {
            id: TabId(0),
            name: "name".into(),
            dimensions: (1, 2),
            shell: "bash".into(),
            dir: "/".into(),
        };
        tabs.tabs.insert(tab_id, tab_metadata.clone());
        tx.send(tabs).await?;

        let _service = CliService::spawn(&cli_bus)?;
        let mut rx = cli_bus.rx::<Response>()?;

        assert_completes!(async move {
            let init = rx.recv().await;

            let mut expect_tabs = InitResponse {
                tabs: HashMap::new(),
            };
            expect_tabs.tabs.insert(tab_id, tab_metadata.clone());
            assert_eq!(Some(Response::Init(expect_tabs)), init);

            let tab_update = rx.recv().await;
            assert_eq!(Some(Response::TabUpdate(tab_metadata)), tab_update);
        });

        Ok(())
    }

    #[tokio::test]
    async fn subscribe() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let _service = CliService::spawn(&cli_bus)?;

        let mut tx = cli_bus.tx::<Request>()?;
        let rx_subscription = cli_bus.rx::<Subscription<TabId>>()?.into_inner();
        let mut rx_clisend = cli_bus.rx::<CliSend>()?;

        tx.send(Request::Subscribe(TabId(0))).await?;

        assert_completes!(async move {
            let msg = rx_clisend.recv().await;
            assert!(msg.is_some());
            assert_eq!(CliSend::RequestScrollback(TabId(0)), msg.unwrap());

            assert!(rx_subscription.contains(&TabId(0)));
        });

        Ok(())
    }

    #[tokio::test]
    async fn unsubscribe() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let _service = CliService::spawn(&cli_bus)?;

        let mut tx = cli_bus.tx::<Request>()?;
        let mut tx_subscription = cli_bus.tx::<Subscription<TabId>>()?;
        let rx_subscription = cli_bus.rx::<Subscription<TabId>>()?.into_inner();

        tx_subscription
            .send(Subscription::Subscribe(TabId(0)))
            .await?;

        // setup the subscription
        assert_completes!(async {
            while !rx_subscription.contains(&TabId(0)) {
                time::delay_for(Duration::from_millis(5)).await;
            }
        });

        tx.send(Request::Unsubscribe(TabId(0))).await?;

        assert_completes!(async move {
            while rx_subscription.contains(&TabId(0)) {
                time::delay_for(Duration::from_millis(5)).await;
            }
        });

        Ok(())
    }

    #[tokio::test]
    async fn input() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let _service = CliService::spawn(&cli_bus)?;

        let mut tx = cli_bus.tx::<Request>()?;
        let mut rx = cli_bus.rx::<CliSend>()?;

        let input = InputChunk { data: vec![1u8] };
        tx.send(Request::Input(TabId(0), input.clone())).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(CliSend::Input(TabId(0), input)), msg);
        });

        Ok(())
    }

    #[tokio::test]
    async fn create_tab() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let _service = CliService::spawn(&cli_bus)?;

        let mut tx = cli_bus.tx::<Request>()?;
        let mut rx = cli_bus.rx::<CliSend>()?;

        let tab = CreateTabMetadata {
            name: "name".into(),
            dimensions: (1, 2),
            shell: "shell".into(),
            dir: "/".into(),
        };
        tx.send(Request::CreateTab(tab.clone())).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(CliSend::CreateTab(tab)), msg);
        });

        Ok(())
    }

    #[tokio::test]
    async fn resize_tab() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let _service = CliService::spawn(&cli_bus)?;

        let mut tx = cli_bus.tx::<Request>()?;
        let mut rx = cli_bus.rx::<CliSend>()?;

        tx.send(Request::ResizeTab(TabId(0), (1, 2))).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(CliSend::ResizeTab(TabId(0), (1, 2))), msg);
        });

        Ok(())
    }

    #[tokio::test]
    async fn close_tab() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let _service = CliService::spawn(&cli_bus)?;

        let mut tx = cli_bus.tx::<Request>()?;
        let mut rx = cli_bus.rx::<CliSend>()?;

        tx.send(Request::CloseTab(TabId(0))).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(CliSend::CloseTab(TabId(0))), msg);
        });

        Ok(())
    }

    #[tokio::test]
    async fn close_named_tab() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let _service = CliService::spawn(&cli_bus)?;

        let mut tx = cli_bus.tx::<Request>()?;
        let mut rx = cli_bus.rx::<CliSend>()?;

        tx.send(Request::CloseNamedTab("tab".into())).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(CliSend::CloseNamedTab("tab".into())), msg);
        });

        Ok(())
    }

    #[tokio::test]
    async fn retask() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let _service = CliService::spawn(&cli_bus)?;

        let mut tx = cli_bus.tx::<Request>()?;
        let mut rx = cli_bus.rx::<CliSend>()?;

        tx.send(Request::Retask(TabId(0), TabId(1))).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(CliSend::Retask(TabId(0), TabId(1))), msg);
        });

        Ok(())
    }

    #[tokio::test]
    async fn global_shutdown() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let _service = CliService::spawn(&cli_bus)?;

        let mut tx = cli_bus.tx::<Request>()?;
        let mut rx = cli_bus.rx::<CliSend>()?;

        tx.send(Request::GlobalShutdown).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(CliSend::GlobalShutdown), msg);
        });

        Ok(())
    }
}

#[cfg(test)]
mod recv_tests {
    use super::CliService;
    use crate::{
        bus::CliBus,
        message::{
            cli::{CliRecv, CliSend},
            tab::TabScrollback,
        },
    };
    use lifeline::{
        assert_completes, assert_times_out, subscription::Subscription, Bus, Receiver, Sender,
        Service,
    };
    use tab_api::{
        chunk::OutputChunk,
        client::Response,
        tab::{TabId, TabMetadata},
    };
    use tokio::time;

    #[tokio::test]
    async fn tab_started() -> anyhow::Result<()> {
        let bus = CliBus::default();
        let _service = CliService::spawn(&bus)?;

        let mut tx = bus.tx::<CliRecv>()?;
        let mut rx = bus.rx::<Response>()?;

        let metadata = TabMetadata {
            id: TabId(0),
            name: "name".into(),
            dimensions: (1, 2),
            shell: "shell".into(),
            dir: "/".into(),
        };

        tx.send(CliRecv::TabStarted(metadata.clone())).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(Response::TabUpdate(metadata)), msg);
        });

        Ok(())
    }

    #[tokio::test]
    async fn scrollback() -> anyhow::Result<()> {
        let bus = CliBus::default();
        let _service = CliService::spawn(&bus)?;

        let mut tx_subscription = bus.tx::<Subscription<TabId>>()?;
        let rx_subscription = bus.rx::<Subscription<TabId>>()?.into_inner();
        let mut tx = bus.tx::<CliRecv>()?;
        let mut rx = bus.rx::<Response>()?;

        tx_subscription
            .send(Subscription::Subscribe(TabId(0)))
            .await?;

        assert_completes!(async {
            while !rx_subscription.contains(&TabId(0)) {
                time::delay_for(Duration::from_millis(5)).await;
            }
        });

        let scrollback = TabScrollback::empty(TabId(0));
        scrollback
            .push(OutputChunk {
                index: 1,
                data: vec![1, 2],
            })
            .await;

        tx.send(CliRecv::Scrollback(scrollback)).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(
                Some(Response::Output(
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
        let _service = CliService::spawn(&bus)?;

        let mut tx = bus.tx::<CliRecv>()?;
        let mut rx = bus.rx::<Response>()?;

        let scrollback = TabScrollback::empty(TabId(0));
        scrollback
            .push(OutputChunk {
                index: 1,
                data: vec![1, 2],
            })
            .await;

        tx.send(CliRecv::Scrollback(scrollback)).await?;

        assert_times_out!(async {
            rx.recv().await;
        });

        Ok(())
    }

    #[tokio::test]
    async fn output() -> anyhow::Result<()> {
        let bus = CliBus::default();
        let _service = CliService::spawn(&bus)?;

        let mut tx_subscription = bus.tx::<Subscription<TabId>>()?;
        let rx_subscription = bus.rx::<Subscription<TabId>>()?.into_inner();
        let mut tx = bus.tx::<CliRecv>()?;
        let mut rx = bus.rx::<Response>()?;

        tx_subscription
            .send(Subscription::Subscribe(TabId(0)))
            .await?;

        assert_completes!(async {
            while !rx_subscription.contains(&TabId(0)) {
                time::delay_for(Duration::from_millis(5)).await;
            }
        });

        let output = OutputChunk {
            index: 1,
            data: vec![1, 2],
        };

        tx.send(CliRecv::Output(TabId(0), output)).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(
                Some(Response::Output(
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
        let _service = CliService::spawn(&bus)?;

        let mut tx = bus.tx::<CliRecv>()?;
        let mut rx = bus.rx::<Response>()?;

        let output = OutputChunk {
            index: 1,
            data: vec![1, 2],
        };

        tx.send(CliRecv::Output(TabId(0), output)).await?;

        assert_times_out!(async {
            rx.recv().await;
        });

        Ok(())
    }

    #[tokio::test]
    async fn tab_stopped() -> anyhow::Result<()> {
        let bus = CliBus::default();
        let _service = CliService::spawn(&bus)?;

        let mut tx = bus.tx::<CliRecv>()?;
        let mut rx = bus.rx::<Response>()?;

        tx.send(CliRecv::TabStopped(TabId(0))).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(Response::TabTerminated(TabId(0))), msg);
        });

        Ok(())
    }

    #[tokio::test]
    async fn retask() -> anyhow::Result<()> {
        let bus = CliBus::default();
        let _service = CliService::spawn(&bus)?;

        let mut tx_subscription = bus.tx::<Subscription<TabId>>()?;
        let rx_subscription = bus.rx::<Subscription<TabId>>()?.into_inner();
        let mut tx = bus.tx::<CliRecv>()?;
        let mut rx = bus.rx::<Response>()?;
        let mut rx_daemon = bus.rx::<CliSend>()?;

        tx_subscription
            .send(Subscription::Subscribe(TabId(0)))
            .await?;

        assert_completes!(async {
            while !rx_subscription.contains(&TabId(0)) {
                time::delay_for(Duration::from_millis(5)).await;
            }
        });

        tx.send(CliRecv::Retask(TabId(0), TabId(1))).await?;

        assert_completes!(async {
            let msg = rx.recv().await;
            assert_eq!(Some(Response::Retask(TabId(1),)), msg);
        });

        assert_completes!(async {
            while rx_subscription.contains(&TabId(0)) {
                time::delay_for(Duration::from_millis(5)).await;
            }
        });

        assert_completes!(async {
            while !rx_subscription.contains(&TabId(1)) {
                time::delay_for(Duration::from_millis(5)).await;
            }
        });

        assert_completes!(async {
            let msg = rx_daemon.recv().await;
            assert_eq!(Some(CliSend::RequestScrollback(TabId(1))), msg);
        });

        Ok(())
    }
}
