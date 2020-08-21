use super::{client::ClientService, tab_state::TabStateService, terminal::TerminalService};
use crate::bus::ClientBus;
use crate::{
    bus::MainBus,
    message::main::{MainRecv, MainShutdown},
    state::tab::TabStateSelect,
};
use log::{debug, error};
use tab_api::{request::Request, response::Response};
use tab_service::{dyn_bus::DynBus, Bus, Lifeline, Service};

use tab_websocket::{
    bus::WebsocketConnectionBus,
    message::connection::{WebsocketRecv, WebsocketSend},
    resource::connection::WebsocketResource,
    service::WebsocketService,
};
use tungstenite::Message as TungsteniteMessage;
pub struct MainService {
    _main: Lifeline,
    _client: ClientService,
    _websocket: WebsocketService,
    _terminal: TerminalService,
    _tab_state: TabStateService,
    _websocket_send: Lifeline,
    _websocket_recv: Lifeline,
}

impl Service for MainService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &MainBus) -> anyhow::Result<Self> {
        let client_bus = ClientBus::default();

        client_bus.take_tx::<MainShutdown, _>(bus)?;

        let websocket_bus = WebsocketConnectionBus::default();
        websocket_bus.take_resource::<WebsocketResource, _>(bus)?;

        let mut main_rx = bus.rx::<MainRecv>()?;
        // let mut tx_client = bus.tx::<ClientRecv>();
        let tx_select_tab = client_bus.tx::<TabStateSelect>()?;

        let _main = Self::try_task("main_recv", async move {
            while let Some(msg) = main_rx.recv().await {
                debug!("MainRecv: {:?}", &msg);

                match msg {
                    MainRecv::SelectTab(name) => {
                        tx_select_tab
                            .broadcast(TabStateSelect::Selected(name))
                            .map_err(|_err| anyhow::Error::msg("failed to send msg"))?;
                    }
                }
            }

            Ok(())
        });

        let _websocket = WebsocketService::spawn(&websocket_bus)?;
        let _websocket_send = {
            let mut rx = client_bus.rx::<Request>()?;
            let mut tx = websocket_bus.tx::<WebsocketSend>()?;

            Self::task("forward_request", async move {
                while let Some(req) = rx.recv().await {
                    match bincode::serialize(&req) {
                        Ok(vec) => tx
                            .send(WebsocketSend(TungsteniteMessage::Binary(vec)))
                            .await
                            .expect("failed to send websocket msg"),
                        Err(e) => error!("failed to send websocket msg: {}", e),
                    };
                }

                tx.send(WebsocketSend(TungsteniteMessage::Close(None)))
                    .await
                    .expect("failed to close websocket");
            })
        };

        let _websocket_recv = {
            let mut rx = websocket_bus.rx::<WebsocketRecv>()?;
            let mut tx = client_bus.tx::<Response>()?;

            Self::task("forward_request", async move {
                while let Some(resp) = rx.recv().await {
                    match bincode::deserialize(resp.0.into_data().as_slice()) {
                        Ok(resp) => tx.send(resp).await.expect("failed to send websocket msg"),
                        Err(e) => error!("failed to send websocket msg: {}", e),
                    };
                }
            })
        };

        let _tab_state = TabStateService::spawn(&client_bus)?;
        let _client = ClientService::spawn(&client_bus)?;
        let _terminal = TerminalService::spawn(&client_bus)?;

        Ok(Self {
            _main,
            _client,
            _websocket,
            _terminal,
            _tab_state,
            _websocket_send,
            _websocket_recv,
        })
    }
}
