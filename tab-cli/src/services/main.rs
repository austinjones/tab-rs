use super::{
    client::{ClientRx, ClientService, ClientTx},
    state::{TabState, TabStateAvailable, TerminalSizeState},
    tab_state::{TabStateRx, TabStateSelect, TabStateService},
    terminal::{TerminalRecv, TerminalSend, TerminalService, TerminalTx},
};
use crate::services::state::StateBus;
use log::{debug, info};
use tab_api::{request::Request, response::Response, tab::TabMetadata};
use tab_service::{service_bus, Bus, Lifeline, Message, Service};
use tab_websocket::{
    service::{WebsocketRx, WebsocketService},
    WebSocket,
};
use tokio::sync::{mpsc, watch};

#[derive(Debug)]
pub struct MainShutdown {}

#[derive(Debug)]
pub enum MainRecv {
    SelectTab(String),
}

service_bus!(pub MainBus);

impl Message<MainBus> for MainShutdown {
    type Channel = mpsc::Sender<Self>;
}

impl Message<MainBus> for MainRecv {
    type Channel = mpsc::Sender<Self>;
}

service_bus!(InternalBus);
impl Message<InternalBus> for TerminalSend {
    type Channel = mpsc::Sender<Self>;
}

impl Message<InternalBus> for TerminalRecv {
    type Channel = mpsc::Sender<Self>;
}

impl Message<InternalBus> for Request {
    type Channel = mpsc::Sender<Self>;
}

impl Message<InternalBus> for Response {
    type Channel = mpsc::Sender<Self>;
}

pub struct MainService {
    _main: Lifeline,
    _client: ClientService,
    _websocket: WebsocketService<Request, Response>,
    _terminal: TerminalService,
    _tab_state: Lifeline,
}

pub struct MainRx {
    pub websocket: WebSocket,
    pub rx: mpsc::Receiver<MainRecv>,
}

impl Service for MainService {
    type Rx = MainRx;
    type Tx = mpsc::Sender<MainShutdown>;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(rx: Self::Rx, tx: Self::Tx) -> anyhow::Result<Self> {
        let bus = InternalBus::default();
        let state = StateBus::default();

        let mut main_rx = rx.rx;
        // let mut tx_client = bus.tx::<ClientRecv>();
        let tx_select_tab = state.tx::<TabStateSelect>()?;

        let _main = Self::task("main_recv", async move {
            while let Some(msg) = main_rx.recv().await {
                debug!("MainRecv: {:?}", &msg);

                match msg {
                    MainRecv::SelectTab(name) => tx_select_tab
                        .broadcast(TabStateSelect::Selected(name))
                        .expect("failed to send msg"),
                }
            }
        });

        let tab_state_rx = TabStateRx {
            tab: state.rx::<TabStateSelect>()?,
            tab_metadata: state.rx::<TabMetadata>()?,
        };
        let _tab_state = TabStateService::spawn(tab_state_rx, state.tx::<TabState>()?);
        let terminal_tx = TerminalTx {
            size: state.tx::<TerminalSizeState>()?,
            tx: bus.tx::<TerminalSend>()?,
        };
        let _terminal = TerminalService::spawn(bus.rx::<TerminalRecv>()?, terminal_tx);

        let websocket_rx = WebsocketRx {
            websocket: rx.websocket,
            rx: bus.rx::<Request>()?,
        };

        let _websocket = WebsocketService::spawn(websocket_rx, bus.tx::<Response>()?);

        let rx = ClientRx {
            terminal: bus.rx::<TerminalSend>()?,
            websocket: bus.rx::<Response>()?,
            tab_state: state.rx::<TabState>()?,
            terminal_size: state.rx::<TerminalSizeState>()?,
        };

        let tx = ClientTx {
            terminal: bus.tx::<TerminalRecv>()?,
            websocket: bus.tx::<Request>()?,
            active_tabs: state.tx::<TabStateAvailable>()?,
            tab_metadata: state.tx::<TabMetadata>()?,
            shutdown: tx,
        };

        let _client = ClientService::spawn(rx, tx);

        Ok(Self {
            _main,
            _client,
            _websocket,
            _terminal,
            _tab_state,
        })
    }
}
