use crate::{message::cli::CliSubscriptionRecv, message::cli::CliSubscriptionSend, prelude::*};
use crate::{
    message::{
        cli::{CliRecv, CliSend, CliShutdown},
        listener::ListenerShutdown,
        tab::{TabInput, TabRecv, TabSend},
        tab_manager::TabManagerRecv,
    },
    state::tab::TabsState,
};
use anyhow::Context;
use std::sync::Arc;
use tab_api::{client::Request, client::Response};
use tab_websocket::{bus::WebsocketMessageBus, resource::connection::WebsocketResource};
use time::Duration;
use tokio::{
    sync::{broadcast, mpsc},
    time,
};

lifeline_bus!(pub struct CliBus);

impl Message<CliBus> for CliShutdown {
    type Channel = mpsc::Sender<Self>;
}

impl Message<CliBus> for Request {
    type Channel = broadcast::Sender<Self>;
}

impl Message<CliBus> for Response {
    type Channel = mpsc::Sender<Self>;
}

impl Message<CliBus> for CliSend {
    type Channel = mpsc::Sender<Self>;
}

impl Message<CliBus> for CliRecv {
    type Channel = mpsc::Sender<Self>;
}

impl Message<CliBus> for CliSubscriptionSend {
    type Channel = mpsc::Sender<Self>;
}

impl Message<CliBus> for CliSubscriptionRecv {
    type Channel = mpsc::Sender<Self>;
}

/// This binding needs to be mpsc, as it is carried from the listener.
/// If it is watch, receivers can see a temporary empty value
impl Message<CliBus> for TabsState {
    type Channel = mpsc::Sender<Self>;
}

impl Resource<CliBus> for WebsocketResource {}
impl WebsocketMessageBus for CliBus {
    type Send = Response;
    type Recv = Request;
}

pub struct ListenerConnectionCarrier {
    _forward: Lifeline,
    _reverse: Lifeline,
    _forward_tabs_state: Lifeline,
}

impl CarryFrom<ListenerBus> for CliBus {
    type Lifeline = anyhow::Result<ListenerConnectionCarrier>;

    fn carry_from(&self, from: &ListenerBus) -> Self::Lifeline {
        let _forward = {
            let rx_tab = from.rx::<TabSend>()?;

            let tx_conn = self.tx::<CliRecv>()?;
            let tx_subscription = self.tx::<CliSubscriptionRecv>()?;

            Self::try_task(
                "output",
                Self::run_output(rx_tab, tx_conn.clone(), tx_subscription),
            )
        };

        let _reverse = {
            let rx_conn = self.rx::<CliSend>()?;

            let tx_tab = from.tx::<TabRecv>()?.log();
            let tx_manager = from.tx::<TabManagerRecv>()?;
            let tx_shutdown = self.tx::<CliShutdown>()?;
            let tx_listener_shutdown = from.tx::<ListenerShutdown>()?;
            Self::try_task(
                "input",
                Self::run_input(
                    rx_conn,
                    tx_tab,
                    tx_manager,
                    tx_shutdown,
                    tx_listener_shutdown,
                ),
            )
        };

        let _forward_tabs_state = {
            let mut rx_tabs_state = from.rx::<TabsState>()?;
            let mut tx_tabs_state = self.tx::<TabsState>()?;
            Self::try_task("forward_tabs_state", async move {
                while let Some(msg) = rx_tabs_state.recv().await {
                    tx_tabs_state.send(msg).await.ok();
                }

                Ok(())
            })
        };

        Ok(ListenerConnectionCarrier {
            _forward,
            _reverse,
            _forward_tabs_state,
        })
    }
}

impl CliBus {
    async fn run_output(
        mut rx: impl Receiver<TabSend>,
        mut tx: impl Sender<CliRecv>,
        mut tx_subscription: impl Sender<CliSubscriptionRecv>,
    ) -> anyhow::Result<()> {
        while let Some(msg) = rx.recv().await {
            Self::handle_tabsend(msg, &mut tx, &mut tx_subscription).await?
        }

        Ok(())
    }

    async fn run_input(
        mut rx: impl Receiver<CliSend>,
        mut tx: impl Sender<TabRecv>,
        mut tx_manager: impl Sender<TabManagerRecv>,
        mut tx_shutdown: impl Sender<CliShutdown>,
        mut tx_listener_shutdown: impl Sender<ListenerShutdown>,
    ) -> anyhow::Result<()> {
        while let Some(msg) = rx.recv().await {
            match msg {
                CliSend::CreateTab(create) => {
                    debug!("received CreateTab from client: {:?}", &create);
                    tx_manager.send(TabManagerRecv::CreateTab(create)).await?;
                }
                CliSend::CloseTab(id) => {
                    tx_manager.send(TabManagerRecv::CloseTab(id)).await?;
                }
                CliSend::CloseNamedTab(name) => {
                    tx_manager.send(TabManagerRecv::CloseNamedTab(name)).await?;
                }
                CliSend::RequestScrollback(id) => {
                    debug!(
                        "ListenerConnectionCarrier forwarding scrollback request on tab {:?}",
                        id
                    );
                    tx.send(TabRecv::Scrollback(id))
                        .await
                        .context("tx TabRecv::Scrollback")?;
                }
                CliSend::Input(id, input) => {
                    let stdin = Arc::new(input);
                    let input = TabInput { id, stdin };
                    let message = TabRecv::Input(input);
                    tx.send(message).await.context("tx TabRecv closed")?;
                }
                CliSend::ResizeTab(id, dimensions) => {
                    let message = TabRecv::Resize(id, dimensions);
                    tx.send(message).await?;
                }
                CliSend::Retask(from, to) => {
                    let message = TabRecv::Retask(from, to);
                    tx.send(message).await?;
                }
                CliSend::GlobalShutdown => {
                    info!("Daemon receieved a global shutdown.");
                    tx.send(TabRecv::TerminateAll).await?;
                    tx_listener_shutdown.send(ListenerShutdown {}).await?;
                    time::delay_for(Duration::from_millis(50)).await;
                }
            }
        }

        tx_shutdown.send(CliShutdown {}).await.ok();

        Ok(())
    }

    async fn handle_tabsend(
        msg: TabSend,
        tx: &mut impl Sender<CliRecv>,
        tx_subscription: &mut impl Sender<CliSubscriptionRecv>,
    ) -> anyhow::Result<()> {
        match msg {
            TabSend::Started(tab) => tx.send(CliRecv::TabStarted(tab)).await?,
            TabSend::Stopped(id) => {
                tx_subscription
                    .send(CliSubscriptionRecv::Stopped(id))
                    .await?;
            }
            TabSend::Scrollback(scrollback) => {
                tx_subscription
                    .send(CliSubscriptionRecv::Scrollback(scrollback))
                    .await?;
            }
            TabSend::Output(stdout) => {
                tx_subscription
                    .send(CliSubscriptionRecv::Output(stdout))
                    .await?
            }
            TabSend::Retask(from, to) => {
                tx_subscription
                    .send(CliSubscriptionRecv::Retask(from, to))
                    .await?;
            }
        };

        Ok(())
    }
}

#[cfg(test)]
mod forward_tests {
    use crate::message::{
        cli::CliRecv,
        cli::CliSubscriptionRecv,
        tab::{TabOutput, TabScrollback, TabSend},
    };
    use crate::{
        prelude::*, service::pty::scrollback::ScrollbackBuffer, state::pty::PtyScrollback,
    };
    use lifeline::{assert_completes, assert_times_out};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tab_api::{
        chunk::OutputChunk,
        tab::{TabId, TabMetadata},
    };
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn started() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let listener_bus = ListenerBus::default();

        let _carrier = cli_bus.carry_from(&listener_bus)?;

        let mut tx = listener_bus.tx::<TabSend>()?;
        let mut rx = cli_bus.rx::<CliRecv>()?;

        let started = TabMetadata {
            id: TabId(0),
            name: "name".into(),
            dimensions: (1, 1),
            env: HashMap::new(),
            shell: "bash".into(),
            dir: "dir".into(),
        };

        tx.send(TabSend::Started(started.clone())).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert!(msg.is_some());
            assert_eq!(CliRecv::TabStarted(started), msg.unwrap());
        });

        Ok(())
    }

    #[tokio::test]
    async fn stopped() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let listener_bus = ListenerBus::default();

        let _carrier = cli_bus.carry_from(&listener_bus)?;

        let mut tx = listener_bus.tx::<TabSend>()?;
        let mut rx = cli_bus.rx::<CliSubscriptionRecv>()?;

        tx.send(TabSend::Stopped(TabId(0))).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert!(msg.is_some());
            assert_eq!(CliSubscriptionRecv::Stopped(TabId(0)), msg.unwrap());
        });

        Ok(())
    }

    #[tokio::test]
    async fn scrollback() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let listener_bus = ListenerBus::default();

        let _carrier = cli_bus.carry_from(&listener_bus)?;

        let mut tx = listener_bus.tx::<TabSend>()?;
        let mut rx = cli_bus.rx::<CliSubscriptionRecv>()?;

        let mut buffer = ScrollbackBuffer::new();
        buffer.push(OutputChunk {
            index: 0,
            data: vec![0, 1],
        });
        buffer.push(OutputChunk {
            index: 2,
            data: vec![1, 2],
        });
        let scrollback = PtyScrollback::new(Arc::new(Mutex::new(buffer)));
        let scrollback = TabScrollback {
            id: TabId(0),
            scrollback,
        };
        tx.send(TabSend::Scrollback(scrollback)).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert!(msg.is_some());
            if let CliSubscriptionRecv::Scrollback(scroll) = msg.unwrap() {
                let mut iter = scroll.scrollback().await;
                assert_eq!(
                    Some(OutputChunk {
                        index: 0,
                        data: vec![0, 1, 1, 2]
                    }),
                    iter.next()
                );
                assert_eq!(None, iter.next());
            } else {
                panic!("Expected CliRecv::Scrollback, found None");
            }
        });

        Ok(())
    }

    #[tokio::test]
    async fn output() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let listener_bus = ListenerBus::default();

        let _carrier = cli_bus.carry_from(&listener_bus)?;

        let mut tx = listener_bus.tx::<TabSend>()?;
        let mut rx = cli_bus.rx::<CliSubscriptionRecv>()?;

        tx.send(TabSend::Output(TabOutput {
            id: TabId(0),
            stdout: Arc::new(OutputChunk {
                index: 0,
                data: vec![0],
            }),
        }))
        .await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            if let Some(CliSubscriptionRecv::Output(output)) = msg {
                assert_eq!(TabId(0), output.id);
            } else {
                panic!("expected CliRecv::Output, found: {:?}", msg)
            }
        });

        Ok(())
    }

    #[tokio::test]
    async fn retask() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let listener_bus = ListenerBus::default();

        let _carrier = cli_bus.carry_from(&listener_bus)?;

        let mut tx = listener_bus.tx::<TabSend>()?;
        let mut rx = cli_bus.rx::<CliSubscriptionRecv>()?;

        tx.send(TabSend::Retask(TabId(0), TabId(1))).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert!(msg.is_some());
            let msg = msg.unwrap();
            if let CliSubscriptionRecv::Retask(from, to) = msg {
                assert_eq!(TabId(0), from);
                assert_eq!(TabId(1), to);
            } else {
                panic!("Expected CliSubscriptionRecv::Retask, found {:?}", msg);
            }
        });

        Ok(())
    }

    #[tokio::test]
    async fn retask_unsubscribed() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let listener_bus = ListenerBus::default();

        let _carrier = cli_bus.carry_from(&listener_bus)?;

        let mut tx = listener_bus.tx::<TabSend>()?;
        let mut rx = cli_bus.rx::<CliRecv>()?;

        tx.send(TabSend::Retask(TabId(0), TabId(1))).await?;

        assert_times_out!(async move {
            let _msg = rx.recv().await;
        });

        Ok(())
    }
}

#[cfg(test)]
mod reverse_tests {
    use crate::{
        message::{
            cli::CliSend,
            listener::ListenerShutdown,
            tab::{TabInput, TabRecv},
            tab_manager::TabManagerRecv,
        },
        prelude::*,
    };
    use lifeline::assert_completes;
    use std::collections::HashMap;
    use tab_api::{
        chunk::InputChunk,
        tab::{CreateTabMetadata, TabId},
    };

    #[tokio::test]
    async fn create_tab() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let listener_bus = ListenerBus::default();

        let _carrier = cli_bus.carry_from(&listener_bus)?;

        let mut tx = cli_bus.tx::<CliSend>()?;
        let mut rx = listener_bus.rx::<TabManagerRecv>()?;

        let create = CreateTabMetadata {
            name: "name".into(),
            shell: "bash".into(),
            env: HashMap::new(),
            dimensions: (1, 1),
            dir: "dir".into(),
        };

        tx.send(CliSend::CreateTab(create.clone())).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert!(msg.is_some());
            assert_eq!(TabManagerRecv::CreateTab(create), msg.unwrap());
        });

        Ok(())
    }

    #[tokio::test]
    async fn close_tab() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let listener_bus = ListenerBus::default();

        let _carrier = cli_bus.carry_from(&listener_bus)?;

        let mut tx = cli_bus.tx::<CliSend>()?;
        let mut rx = listener_bus.rx::<TabManagerRecv>()?;

        tx.send(CliSend::CloseTab(TabId(0))).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert!(msg.is_some());
            assert_eq!(TabManagerRecv::CloseTab(TabId(0)), msg.unwrap());
        });

        Ok(())
    }

    #[tokio::test]
    async fn close_named_tab() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let listener_bus = ListenerBus::default();

        let _carrier = cli_bus.carry_from(&listener_bus)?;

        let mut tx = cli_bus.tx::<CliSend>()?;
        let mut rx = listener_bus.rx::<TabManagerRecv>()?;

        tx.send(CliSend::CloseNamedTab("foo".into())).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert!(msg.is_some());
            assert_eq!(TabManagerRecv::CloseNamedTab("foo".into()), msg.unwrap());
        });

        Ok(())
    }

    #[tokio::test]
    async fn request_scrollback() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let listener_bus = ListenerBus::default();

        let _carrier = cli_bus.carry_from(&listener_bus)?;

        let mut tx = cli_bus.tx::<CliSend>()?;
        let mut rx = listener_bus.rx::<TabRecv>()?;

        tx.send(CliSend::RequestScrollback(TabId(0))).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert!(msg.is_some());
            assert_eq!(TabRecv::Scrollback(TabId(0)), msg.unwrap());
        });

        Ok(())
    }

    #[tokio::test]
    async fn input() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let listener_bus = ListenerBus::default();

        let _carrier = cli_bus.carry_from(&listener_bus)?;

        let mut tx = cli_bus.tx::<CliSend>()?;
        let mut rx = listener_bus.rx::<TabRecv>()?;

        tx.send(CliSend::Input(TabId(0), InputChunk { data: vec![0] }))
            .await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert!(msg.is_some());
            assert_eq!(
                TabRecv::Input(TabInput::new(TabId(0), vec![0u8])),
                msg.unwrap()
            );
        });

        Ok(())
    }

    #[tokio::test]
    async fn resize() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let listener_bus = ListenerBus::default();

        let _carrier = cli_bus.carry_from(&listener_bus)?;

        let mut tx = cli_bus.tx::<CliSend>()?;
        let mut rx = listener_bus.rx::<TabRecv>()?;

        tx.send(CliSend::ResizeTab(TabId(0), (1, 2))).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert!(msg.is_some());
            assert_eq!(TabRecv::Resize(TabId(0), (1, 2)), msg.unwrap());
        });

        Ok(())
    }

    #[tokio::test]
    async fn retask() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let listener_bus = ListenerBus::default();

        let _carrier = cli_bus.carry_from(&listener_bus)?;

        let mut tx = cli_bus.tx::<CliSend>()?;
        let mut rx = listener_bus.rx::<TabRecv>()?;

        tx.send(CliSend::Retask(TabId(0), TabId(1))).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert!(msg.is_some());
            assert_eq!(TabRecv::Retask(TabId(0), TabId(1)), msg.unwrap());
        });

        Ok(())
    }

    #[tokio::test]
    async fn global_shutdown() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let listener_bus = ListenerBus::default();

        let _carrier = cli_bus.carry_from(&listener_bus)?;

        let mut tx = cli_bus.tx::<CliSend>()?;
        let mut rx = listener_bus.rx::<ListenerShutdown>()?;

        tx.send(CliSend::GlobalShutdown).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert!(msg.is_some());
        });

        Ok(())
    }
}

#[cfg(test)]
mod tabs_state_tests {
    use crate::{prelude::*, state::tab::TabsState};
    use lifeline::assert_completes;
    use std::collections::HashMap;

    #[tokio::test]
    async fn forward_state() -> anyhow::Result<()> {
        let cli_bus = CliBus::default();
        let listener_bus = ListenerBus::default();

        let _carrier = cli_bus.carry_from(&listener_bus)?;

        let mut tx = listener_bus.tx::<TabsState>()?;
        let mut rx = cli_bus.rx::<TabsState>()?;

        tx.send(TabsState {
            tabs: HashMap::new(),
        })
        .await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert!(msg.is_some());
            assert_eq!(
                TabsState {
                    tabs: HashMap::new()
                },
                msg.unwrap()
            );
        });

        Ok(())
    }
}
