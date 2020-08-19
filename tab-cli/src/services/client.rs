use super::terminal::{TerminalRecv, TerminalSend};
use futures::SinkExt;
use tab_api::{
    request::Request,
    response::Response,
    tab::{CreateTabMetadata, TabId},
};
use tab_service::{spawn, Lifeline, Service};
use tokio::sync::{mpsc, watch};

pub struct ClientService {
    _websocket: WebsocketRxService,
    _terminal: TerminalRxService,
}

pub struct ClientServiceRx {
    pub tab: String,
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
    type Return = Self;

    fn spawn(rx: Self::Rx, mut tx: Self::Tx) -> Self {
        let (rx_tab, tx_tab) = watch::channel(Some(TabId(0)));
        let _websocket = WebsocketRxService::spawn((rx.tab, rx.websocket), tx.clone());
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
    type Rx = (String, mpsc::Receiver<Response>);
    type Tx = ClientServiceTx;

    fn spawn((tab, mut rx): Self::Rx, tx: Self::Tx) -> Self {
        let _websocket = spawn(async move {
            let mut active = None;
            while let Some(msg) = rx.recv().await {
                match msg {
                    Response::Output(_, _) => {}
                    Response::TabUpdate(_) => {}
                    Response::TabList(tabs) => {
                        let found = tabs.iter().find(|metadata| metadata.name == tab);
                        if let Some(found) = found {
                            active = Some(found.id);
                        } else {
                            tx.websocket
                                .send(Request::CreateTab(CreateTabMetadata {
                                    name: tab,
                                    dimensions: (),
                                }))
                                .await;
                        }
                    }
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
