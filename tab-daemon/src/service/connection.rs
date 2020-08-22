// mod session;

use crate::state::{connection::ConnectionState, tab::TabsState};
use crate::{
    bus::ConnectionBus,
    message::connection::{ConnectionRecv, ConnectionSend},
};
use anyhow::Context;
use log::{debug, info};
use std::collections::HashMap;
use subscription::{Subscription, SubscriptionState};
use tab_api::{chunk::OutputChunk, request::Request, response::Response, tab::TabId};
use tab_service::{channels::subscription, Bus, Lifeline, Service};
use tab_websocket::message::connection::{WebsocketRecv, WebsocketSend};
use tokio::{stream::StreamExt, sync::mpsc};
use tungstenite::Message as TungsteniteMessage;
pub struct ConnectionService {
    _run: Lifeline,
}

enum Event {
    Websocket(WebsocketRecv),
    Daemon(ConnectionRecv),
}

impl Event {
    pub fn websocket(recv: WebsocketRecv) -> Self {
        Self::Websocket(recv)
    }

    pub fn daemon(recv: ConnectionRecv) -> Self {
        Self::Daemon(recv)
    }
}

impl Service for ConnectionService {
    type Bus = ConnectionBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let _tx = bus.tx::<ConnectionSend>()?;

        let rx_websocket = bus.rx::<WebsocketRecv>()?.map(Event::websocket);
        let rx_daemon = bus.rx::<ConnectionRecv>()?.map(Event::daemon);

        let mut rx = rx_websocket.merge(rx_daemon);

        let mut tx_websocket = bus.tx::<WebsocketSend>()?;
        let mut tx_daemon = bus.tx::<ConnectionSend>()?;
        let mut tx_subscription = bus.tx::<Subscription<TabId>>()?;
        let rx_subscription = bus.rx::<Subscription<TabId>>()?;
        let mut rx_tabs_state = bus.rx::<TabsState>()?;

        let _run = Self::try_task("run", async move {
            let mut state = ConnectionState::default();
            let mut subscription_index: HashMap<usize, usize> = HashMap::new();

            let tabs = rx_tabs_state
                .recv()
                .await
                .ok_or_else(|| anyhow::Error::msg("rx TabsState closed"))?;

            for tab in tabs.tabs.values() {
                debug!("notifying client of existing tab {}", &tab.name);
                let message = Response::TabUpdate(tab.clone());
                let message = Self::serialize(message)?;
                tx_websocket.send(message).await?;
            }

            loop {
                info!("connection bus loop");
                while let Some(event) = rx.next().await {
                    match event {
                        Event::Websocket(msg) => {
                            Self::recv_websocket(
                                msg,
                                &mut state,
                                &mut tx_subscription,
                                &mut tx_websocket,
                                &mut tx_daemon,
                            )
                            .await?
                        }
                        Event::Daemon(msg) => {
                            Self::recv_daemon(
                                msg,
                                &mut subscription_index,
                                &mut tx_websocket,
                                &rx_subscription,
                            )
                            .await?
                        }
                    }
                }
            }

            Ok(())
        });

        Ok(ConnectionService { _run })
    }
}

impl ConnectionService {
    fn deserialize(recv: WebsocketRecv) -> anyhow::Result<Request> {
        let bytes = recv.0.into_data();
        let request = bincode::deserialize(bytes.as_slice())?;
        Ok(request)
    }

    fn serialize(send: Response) -> anyhow::Result<WebsocketSend> {
        debug!("sending response: {:?}", &send);

        let encoded = bincode::serialize(&send)?;
        let send = WebsocketSend(TungsteniteMessage::Binary(encoded));
        Ok(send)
    }

    async fn recv_websocket(
        msg: WebsocketRecv,
        state: &mut ConnectionState,
        tx_subscription: &mut subscription::Sender<TabId>,
        tx_websocket: &mut mpsc::Sender<WebsocketSend>,
        tx_daemon: &mut mpsc::Sender<ConnectionSend>,
    ) -> anyhow::Result<()> {
        let request = Self::deserialize(msg)?;

        if let Request::Auth(auth) = request {
            // TODO: validate auth
            state.auth = true;
            return Ok(());
        }

        if !state.auth {
            let send = Self::serialize(Response::Unauthorized)?;
            tx_websocket
                .send(send)
                .await
                .context("tx_websocket closed")?;
        }

        debug!("received Request: {:?}", &request);

        match request {
            Request::Auth(_) => unreachable!(),
            Request::Subscribe(id) => {
                tx_subscription
                    .send(Subscription::Subscribe(id))
                    .await
                    .context("tx_subscription closed")?;

                tx_daemon
                    .send(ConnectionSend::RequestScrollback(id))
                    .await?;
            }
            Request::Unsubscribe(id) => {
                tx_subscription
                    .send(Subscription::Unsubscribe(id))
                    .await
                    .context("tx_subscription closed")?;
            }
            Request::Input(id, stdin) => {
                let message = ConnectionSend::Input(id, stdin);
                tx_daemon.send(message).await.context("tx_daemon closed")?;
            }
            Request::CreateTab(create) => {
                let message = ConnectionSend::CreateTab(create);
                tx_daemon.send(message).await.context("tx_daemon closed")?;
            }
            Request::CloseTab(id) => {
                let message = ConnectionSend::CloseTab(id);
                tx_daemon.send(message).await.context("tx_daemon closed")?;
            }
        }

        Ok(())
    }

    async fn recv_daemon(
        msg: ConnectionRecv,
        subscription_index: &mut HashMap<usize, usize>,
        tx_websocket: &mut mpsc::Sender<WebsocketSend>,
        rx_subscription: &subscription::Receiver<TabId>,
    ) -> anyhow::Result<()> {
        match msg {
            ConnectionRecv::TabStarted(metadata) => {
                let response = Response::TabUpdate(metadata);
                let message = Self::serialize(response)?;
                tx_websocket
                    .send(message)
                    .await
                    .context("tx_websocket closed")?;
            }
            ConnectionRecv::Scrollback(message) => {
                if !rx_subscription.contains(&message.id) {
                    return Ok(());
                }

                let subscription_id = rx_subscription.get_identifier(&message.id).unwrap();

                for chunk in message.scrollback().await {
                    let index = chunk.index;
                    Self::send_output(message.id, chunk, tx_websocket).await?;
                    subscription_index.insert(subscription_id, index);
                }
            }
            // TODO: this way of handling scrollback isn't perfect
            // if the terminal is generating output, the scrollback may arrive too late.
            // the historical channel would fix this, but it'd also destory some of the tokio::broadcast goodness w/ TabId
            ConnectionRecv::Output(id, chunk) => {
                if !rx_subscription.contains(&id) {
                    return Ok(());
                }

                let subscription_id = rx_subscription.get_identifier(&id).unwrap();
                let index = chunk.index;

                if let Some(sub_index) = subscription_index.get(&subscription_id) {
                    if index <= *sub_index {
                        return Ok(());
                    }
                }

                Self::send_output(id, chunk, tx_websocket).await?;
            }
            ConnectionRecv::TabStopped(id) => {
                let response = Response::TabTerminated(id);
                let message = Self::serialize(response)?;
                tx_websocket
                    .send(message)
                    .await
                    .context("tx_websocket closed")?;
            }
        }
        Ok(())
    }

    async fn send_output(
        id: TabId,
        chunk: OutputChunk,
        tx_websocket: &mut mpsc::Sender<WebsocketSend>,
    ) -> anyhow::Result<()> {
        let response = Response::Output(id, chunk);
        let message = Self::serialize(response)?;
        tx_websocket
            .send(message)
            .await
            .context("tx_websocket closed")?;

        Ok(())
    }
}
