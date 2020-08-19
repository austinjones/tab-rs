use super::{
    client::{ClientService, ClientServiceRx, ClientServiceTx},
    terminal::{TerminalRecv, TerminalSend, TerminalService},
};
use tab_api::{request::Request, response::Response};
use tab_service::{channel_tokio_mpsc, service_bus, spawn, Bus, Lifeline, Service};
use tab_websocket::{
    service::{WebsocketRx, WebsocketService},
    WebSocket,
};
use tokio::sync::mpsc;

pub enum MainShutdown {}
pub enum MainRecv {
    SelectTab,
}

service_bus!(pub MainBus);

channel_tokio_mpsc!(impl Channel<MainBus, 16> for MainShutdown);
channel_tokio_mpsc!(impl Channel<MainBus, 16> for MainRecv);

service_bus!(InternalBus);
channel_tokio_mpsc!(impl Channel<InternalBus, 16> for TerminalSend);
channel_tokio_mpsc!(impl Channel<InternalBus, 16> for TerminalRecv);

channel_tokio_mpsc!(impl Channel<InternalBus, 16> for Request);
channel_tokio_mpsc!(impl Channel<InternalBus, 16> for Response);

pub struct MainService {
    _main: Lifeline,
    _client: ClientService,
    _terminal: TerminalService,
}

pub struct MainRx {
    pub tab: String,
    pub websocket: WebSocket,
    pub rx: mpsc::Receiver<MainRecv>,
}

impl Service for MainService {
    type Rx = MainRx;
    type Tx = mpsc::Sender<MainShutdown>;
    type Return = Self;

    fn spawn(rx: Self::Rx, tx: Self::Tx) -> Self {
        let mut main_rx = rx.rx;
        let _main = spawn(async move { while let Some(msg) = main_rx.recv().await {} });

        let bus = InternalBus::default();

        let _terminal = TerminalService::spawn(
            bus.take_rx::<TerminalRecv>().unwrap(),
            bus.tx::<TerminalSend>(),
        );

        let websocket_rx = WebsocketRx {
            websocket: rx.websocket,
            rx: bus.take_rx::<Request>().unwrap(),
        };

        let _websocket = WebsocketService::spawn(websocket_rx, bus.tx::<Request>());

        let rx = ClientServiceRx {
            tab: rx.tab,
            terminal: bus.take_rx::<TerminalSend>().unwrap(),
            websocket: bus.take_rx::<Response>().unwrap(),
        };

        let tx = ClientServiceTx {
            terminal: bus.tx::<TerminalRecv>(),
            websocket: bus.tx::<Request>(),
        };

        let _client = ClientService::spawn(rx, tx);

        Self {
            _main,
            _client,
            _terminal,
        }
    }
}
