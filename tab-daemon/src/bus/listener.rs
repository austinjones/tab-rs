use crate::prelude::*;
use crate::{
    message::{
        daemon::DaemonShutdown,
        listener::ListenerShutdown,
        tab::{TabRecv, TabSend},
        tab_assignment::{AssignTab, TabAssignmentRetraction},
        tab_manager::{TabManagerRecv, TabManagerSend},
    },
    state::tab::TabsState,
};
use lifeline::error::into_msg;

use tab_websocket::{bus::WebsocketListenerBus, message::listener::WebsocketConnectionMessage};
use tokio::sync::{broadcast, mpsc, watch};

lifeline_bus!(pub struct ListenerBus);

impl Message<ListenerBus> for WebsocketConnectionMessage {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ListenerBus> for ListenerShutdown {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ListenerBus> for TabSend {
    type Channel = broadcast::Sender<Self>;
}

impl Message<ListenerBus> for TabRecv {
    type Channel = broadcast::Sender<Self>;
}

impl Message<ListenerBus> for TabManagerSend {
    type Channel = broadcast::Sender<Self>;
}

impl Message<ListenerBus> for TabManagerRecv {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ListenerBus> for AssignTab {
    type Channel = mpsc::Sender<Self>;
}

impl Message<ListenerBus> for TabAssignmentRetraction {
    type Channel = broadcast::Sender<Self>;
}

impl Message<ListenerBus> for TabsState {
    type Channel = watch::Sender<Self>;
}

pub struct ListenerDaemonCarrier {
    _forward_shutdown: Lifeline,
}

impl CarryFrom<DaemonBus> for ListenerBus {
    type Lifeline = anyhow::Result<ListenerDaemonCarrier>;

    fn carry_from(&self, from: &DaemonBus) -> Self::Lifeline {
        let _forward_shutdown = {
            let mut rx = self.rx::<ListenerShutdown>()?;
            let mut tx = from.tx::<DaemonShutdown>()?;

            Self::task("forward_shutdown", async move {
                if let Some(_shutdown) = rx.recv().await {
                    debug!("listener shutdown");
                    tx.send(DaemonShutdown {}).await.ok();
                }
            })
        };

        Ok(ListenerDaemonCarrier { _forward_shutdown })
    }
}

pub struct ConnectionMessageCarrier {
    _forward_connection: Lifeline,
}

impl CarryFrom<WebsocketListenerBus> for ListenerBus {
    type Lifeline = anyhow::Result<ConnectionMessageCarrier>;
    fn carry_from(&self, from: &WebsocketListenerBus) -> Self::Lifeline {
        let _forward_connection = {
            let mut rx = from.rx::<WebsocketConnectionMessage>()?;
            let mut tx = self.tx::<WebsocketConnectionMessage>()?;
            Self::try_task("forward_connection", async move {
                while let Some(msg) = rx.recv().await {
                    tx.send(msg).await.map_err(into_msg)?;
                }

                Ok(())
            })
        };

        Ok(ConnectionMessageCarrier {
            _forward_connection,
        })
    }
}

#[cfg(test)]
mod carrier_tests {
    use crate::{
        message::{daemon::DaemonShutdown, listener::ListenerShutdown},
        prelude::*,
    };
    use lifeline::assert_completes;

    #[tokio::test]
    async fn forward_shutdown() -> anyhow::Result<()> {
        let daemon_bus = DaemonBus::default();
        let listener_bus = ListenerBus::default();

        let _carrier = listener_bus.carry_from(&daemon_bus)?;

        let mut tx = listener_bus.tx::<ListenerShutdown>()?;
        let mut rx = daemon_bus.rx::<DaemonShutdown>()?;

        tx.send(ListenerShutdown {}).await?;

        assert_completes!(async move {
            rx.recv().await;
        });

        Ok(())
    }
}

#[cfg(test)]
mod connection_tests {
    use crate::prelude::*;
    use http::{Method, Uri};
    use lifeline::assert_completes;
    use tab_websocket::{
        bus::{WebsocketConnectionBus, WebsocketListenerBus},
        message::listener::{RequestMetadata, WebsocketConnectionMessage},
    };

    #[tokio::test]
    async fn connection() -> anyhow::Result<()> {
        let conn_bus = WebsocketListenerBus::default();
        let listener_bus = ListenerBus::default();

        let _carrier = conn_bus.carry_into(&listener_bus);

        let mut tx = conn_bus.tx::<WebsocketConnectionMessage>()?.into_inner();
        let mut rx = listener_bus.rx::<WebsocketConnectionMessage>()?;

        assert_completes!(async move {
            tx.send(WebsocketConnectionMessage {
                bus: WebsocketConnectionBus::default(),
                request: RequestMetadata {
                    method: Method::GET,
                    uri: "/path".parse::<Uri>().expect("uri parse"),
                },
            })
            .await
            .expect("failed to send message");

            rx.recv().await;
        });

        Ok(())
    }
}
