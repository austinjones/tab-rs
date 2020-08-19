use super::terminal::{TerminalRecv, TerminalSend};
use tab_api::{request::Request, response::Response, tab::TabId};
use tab_service::{spawn, Lifeline, Service};
use tokio::sync::{mpsc, watch};

pub struct ClientService {
    _websocket: WebsocketRxService,
    _terminal: TerminalRxService,
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
        let (rx_tab, tx_tab) = watch::channel(Some(TabId(0)));
        let _websocket = WebsocketRxService::spawn(rx.websocket, tx.clone());
        let _terminal = TerminalRxService::spawn(rx.terminal, tx);

        ClientService {
            _websocket,
            _terminal,
        }
    }
}

struct WebsocketRxService {
    _websocket: Lifeline,
}

impl Service for WebsocketRxService {
    type Rx = mpsc::Receiver<Response>;
    type Tx = ClientServiceTx;

    fn spawn(mut rx: Self::Rx, tx: Self::Tx) -> Self {
        let _websocket = spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    Response::Output(_, _) => {}
                    Response::TabUpdate(_) => {}
                    Response::TabList(_) => {}
                    Response::TabTerminated(_) => {}
                    Response::Close => {}
                }
            }
        });

        Self { _websocket }
    }
}

struct TerminalRxService {
    _terminal: Lifeline,
}

impl Service for TerminalRxService {
    type Rx = mpsc::Receiver<TerminalSend>;
    type Tx = ClientServiceTx;

    fn spawn(mut rx: Self::Rx, tx: Self::Tx) -> Self {
        let _terminal = spawn(async move { while let Some(msg) = rx.recv().await {} });

        Self { _terminal }
    }
}
