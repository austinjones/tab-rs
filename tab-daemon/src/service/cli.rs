// mod session;
use crate::message::cli::{
    CliRecv, CliSend, CliShutdown, CliSubscriptionRecv, CliSubscriptionSend,
};
use crate::prelude::*;
use crate::state::tab::TabsState;
use anyhow::Context;
use postage::{sink::Sink, stream::Stream};
use tab_api::client::InitResponse;

pub mod subscription;

/// Drives an active connection from the tab-command client, and forwards messages between the websocket and the daemon.
/// Tracks the client tab subscriptions, and filters messages received from the daemon.
pub struct CliService {
    _init: Lifeline,
    _rx_websocket: Lifeline,
    _rx_daemon: Lifeline,
    _rx_subscription: Lifeline,
}

impl Service for CliService {
    type Bus = CliBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let _init = {
            let mut tx_websocket = bus.tx::<Response>()?.log(Level::Debug);
            let mut rx_tabs_state = bus.rx::<TabsState>()?.log(Level::Debug);

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

        let _rx_websocket = {
            let mut rx = bus.rx::<Request>()?.log(Level::Debug);

            let mut tx_daemon = bus.tx::<CliSend>()?;
            let mut tx_subscription = bus.tx::<CliSubscriptionRecv>()?;
            let mut tx_shutdown = bus.tx::<CliShutdown>()?;

            Self::try_task("run", async move {
                debug!("cli connection waiting for messages");

                while let Some(msg) = rx.recv().await {
                    Self::recv_websocket(msg, &mut tx_subscription, &mut tx_daemon).await?
                }

                tx_shutdown.send(CliShutdown {}).await?;

                Ok(())
            })
        };

        let _rx_daemon = {
            let mut rx = bus.rx::<CliRecv>()?;

            let mut tx_websocket = bus.tx::<Response>()?;

            Self::try_task("run", async move {
                while let Some(msg) = rx.recv().await {
                    Self::recv_daemon(msg, &mut tx_websocket).await?
                }

                Ok(())
            })
        };

        let _rx_subscription = {
            let mut rx = bus.rx::<CliSubscriptionSend>()?;

            let mut tx = bus.tx::<Response>()?;

            Self::try_task("run", async move {
                debug!("cli connection waiting for messages");

                while let Some(msg) = rx.recv().await {
                    match msg {
                        CliSubscriptionSend::Retask(id) => {
                            tx.send(Response::Retask(id)).await?;
                        }
                        CliSubscriptionSend::Output(id, chunk) => {
                            tx.send(Response::Output(id, chunk)).await?;
                        }
                        CliSubscriptionSend::Stopped(id) => {
                            debug!("Notifying client of termination on tab {:?}", id);
                            tx.send(Response::TabTerminated(id)).await?;
                        }
                        CliSubscriptionSend::Disconnect => {
                            tx.send(Response::Disconnect).await?;
                        }
                    }
                }

                Ok(())
            })
        };

        Ok(CliService {
            _init,
            _rx_websocket,
            _rx_daemon,
            _rx_subscription,
        })
    }
}

impl CliService {
    async fn recv_websocket(
        request: Request,
        mut tx_subscription: impl Sink<Item = CliSubscriptionRecv> + Unpin,
        mut tx_daemon: impl Sink<Item = CliSend> + Unpin,
    ) -> anyhow::Result<()> {
        debug!("received Request: {:?}", &request);

        match request {
            Request::Subscribe(id) => {
                debug!("client subscribing to tab {}", id);
                tx_subscription
                    .send(CliSubscriptionRecv::Subscribe(id))
                    .await
                    .context("tx_subscription closed")?;
            }
            Request::Unsubscribe(id) => {
                debug!("client subscribing from tab {}", id);
                tx_subscription
                    .send(CliSubscriptionRecv::Unsubscribe(id))
                    .await
                    .context("tx_subscription closed")?;
            }
            Request::Input(id, stdin) => {
                debug!("rx input on tab {}, data: {}", id.0, stdin.to_string());
                let message = CliSend::Input(id, stdin);
                tx_daemon.send(message).await.context("tx_daemon closed")?;
            }
            Request::CreateTab(create) => {
                let message = CliSend::CreateTab(create);
                tx_daemon.send(message).await.context("tx_daemon closed")?;
            }
            Request::ResizeTab(id, dimensions) => {
                info!("Resizing tab {} to {:?}", id.0, dimensions);
                tx_daemon.send(CliSend::ResizeTab(id, dimensions)).await?;
            }
            Request::CloseTab(id) => {
                let message = CliSend::CloseTab(id);
                tx_daemon.send(message).await.context("tx_daemon closed")?;
            }
            Request::DisconnectTab(id) => {
                let message = CliSend::DisconnectTab(id);
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
        mut tx_websocket: impl Sink<Item = Response> + Unpin,
    ) -> anyhow::Result<()> {
        debug!("message from daemon: {:?}", &msg);
        match msg {
            CliRecv::TabStarted(metadata) => {
                tx_websocket
                    .send(Response::TabUpdate(metadata))
                    .await
                    .context("tx_websocket closed")?;
            }
            CliRecv::TabUpdated(metadata) => {
                tx_websocket
                    .send(Response::TabUpdate(metadata))
                    .await
                    .context("tx_websocket closed")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod request_tests {
    use super::CliService;
    use crate::{
        bus::CliBus, message::cli::CliSend, message::cli::CliSubscriptionRecv,
        state::tab::TabsState,
    };
    use lifeline::{assert_completes, Bus, Receiver, Sender, Service};
    use std::collections::HashMap;
    use tab_api::{
        chunk::InputChunk,
        client::{InitResponse, Request, Response},
        tab::{CreateTabMetadata, TabId, TabMetadata},
    };

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
            doc: Some("doc".into()),
            dimensions: (1, 2),
            env: HashMap::new(),
            shell: "bash".into(),
            dir: "/".into(),
            selected: 0,
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
        let mut rx = cli_bus.rx::<CliSubscriptionRecv>()?;

        tx.send(Request::Subscribe(TabId(0))).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(CliSubscriptionRecv::Subscribe(TabId(0))), msg);
        });

        Ok(())
    }

    #[tokio::test]
    async fn unsubscribe() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let _service = CliService::spawn(&cli_bus)?;

        let mut tx = cli_bus.tx::<Request>()?;
        let mut rx = cli_bus.rx::<CliSubscriptionRecv>()?;

        tx.send(Request::Unsubscribe(TabId(0))).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(CliSubscriptionRecv::Unsubscribe(TabId(0))), msg);
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

        let mut env = HashMap::new();
        env.insert("foo".into(), "bar".into());

        let tab = CreateTabMetadata {
            name: "name".into(),
            doc: Some("doc".into()),
            dimensions: (1, 2),
            shell: "shell".into(),
            dir: "/".into(),
            env,
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
    async fn disconnect_tab() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let _service = CliService::spawn(&cli_bus)?;

        let mut tx = cli_bus.tx::<Request>()?;
        let mut rx = cli_bus.rx::<CliSend>()?;

        tx.send(Request::DisconnectTab(TabId(0))).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(CliSend::DisconnectTab(TabId(0))), msg);
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
    use crate::{bus::CliBus, message::cli::CliRecv, message::cli::CliSubscriptionSend};
    use lifeline::{assert_completes, Bus, Receiver, Sender, Service};
    use std::collections::HashMap;
    use tab_api::{
        client::Response,
        tab::{TabId, TabMetadata},
    };

    #[tokio::test]
    async fn tab_started() -> anyhow::Result<()> {
        let bus = CliBus::default();
        let _service = CliService::spawn(&bus)?;

        let mut tx = bus.tx::<CliRecv>()?;
        let mut rx = bus.rx::<Response>()?;

        let metadata = TabMetadata {
            id: TabId(0),
            name: "name".into(),
            doc: Some("doc".into()),
            dimensions: (1, 2),
            env: HashMap::new(),
            shell: "shell".into(),
            dir: "/".into(),
            selected: 0,
        };

        tx.send(CliRecv::TabStarted(metadata.clone())).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(Response::TabUpdate(metadata)), msg);
        });

        Ok(())
    }

    #[tokio::test]
    async fn tab_updated() -> anyhow::Result<()> {
        let bus = CliBus::default();
        let _service = CliService::spawn(&bus)?;

        let mut tx = bus.tx::<CliRecv>()?;
        let mut rx = bus.rx::<Response>()?;

        let metadata = TabMetadata {
            id: TabId(0),
            name: "name".into(),
            doc: Some("doc".into()),
            dimensions: (1, 2),
            env: HashMap::new(),
            shell: "shell".into(),
            dir: "/".into(),
            selected: 10,
        };

        tx.send(CliRecv::TabUpdated(metadata.clone())).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(Response::TabUpdate(metadata)), msg);
        });

        Ok(())
    }
    #[tokio::test]
    async fn tab_stopped() -> anyhow::Result<()> {
        let bus = CliBus::default();
        let _service = CliService::spawn(&bus)?;

        let mut tx = bus.tx::<CliSubscriptionSend>()?;
        let mut rx = bus.rx::<Response>()?;

        tx.send(CliSubscriptionSend::Stopped(TabId(0))).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(Response::TabTerminated(TabId(0))), msg);
        });

        Ok(())
    }

    #[tokio::test]
    async fn disconnect() -> anyhow::Result<()> {
        let bus = CliBus::default();
        let _service = CliService::spawn(&bus)?;

        let mut tx = bus.tx::<CliSubscriptionSend>()?;
        let mut rx = bus.rx::<Response>()?;

        tx.send(CliSubscriptionSend::Disconnect).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(Response::Disconnect), msg);
        });

        Ok(())
    }
}
