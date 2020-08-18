use super::terminal::{TerminalRecv, TerminalSend};
use tab_api::{request::Request, response::Response};
use tab_service::{spawn, spawn_from, spawn_from_stream, Lifeline, Service};
use tokio::sync::mpsc;

pub struct ClientService {
    websocket: WebsocketRxService,
    terminal: TerminalRxService,
}

pub struct ClientServiceRx {
    pub websocket: mpsc::Receiver<Response>,
    pub terminal: mpsc::Receiver<TerminalSend>,
}

#[derive(Clone)]
pub struct ClientServiceTx {
    pub websocket: mpsc::Sender<Request>,
    pub terminal: mpsc::Sender<TerminalRecv>,
}

impl Service for ClientService {
    type Rx = ClientServiceRx;
    type Tx = ClientServiceTx;

    fn spawn(rx: Self::Rx, mut tx: Self::Tx) -> Self {
        ClientService {
            websocket: WebsocketRxService::spawn(rx.websocket, tx.clone()),
            terminal: TerminalRxService::spawn(rx.terminal, tx),
        }
    }

    fn shutdown(self) {}
}

struct WebsocketRxService {
    websocket: Lifeline,
}

impl Service for WebsocketRxService {
    type Rx = mpsc::Receiver<Response>;
    type Tx = ClientServiceTx;

    fn spawn(rx: Self::Rx, tx: Self::Tx) -> Self {
        let websocket = spawn(async move {});

        Self { websocket }
    }

    fn shutdown(self) {}
}

struct TerminalRxService {
    terminal: Lifeline,
}

impl Service for TerminalRxService {
    type Rx = mpsc::Receiver<TerminalSend>;
    type Tx = ClientServiceTx;

    fn spawn(rx: Self::Rx, tx: Self::Tx) -> Self {
        let terminal = spawn(async move {});

        Self { terminal }
    }

    fn shutdown(self) {}
}
