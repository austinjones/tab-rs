use super::{
    main::MainShutdown,
    state::{TabState, TabStateAvailable, TerminalSizeState},
    terminal::{TerminalRecv, TerminalSend},
};
use log::debug;
use tab_api::{
    chunk::InputChunk,
    request::Request,
    response::Response,
    tab::{CreateTabMetadata, TabId, TabMetadata},
};
use tab_service::{Lifeline, Service};
use tokio::sync::{broadcast, mpsc, watch};

pub struct ClientService {
    _request_tab: Lifeline,
    _websocket: WebsocketMessageService,
    _terminal: TerminalMessageService,
}

pub struct ClientRx {
    pub websocket: mpsc::Receiver<Response>,
    pub terminal: mpsc::Receiver<TerminalSend>,

    pub tab_state: watch::Receiver<TabState>,
    pub terminal_size: watch::Receiver<TerminalSizeState>,
}

pub struct ClientTx {
    pub websocket: mpsc::Sender<Request>,
    pub terminal: mpsc::Sender<TerminalRecv>,
    pub active_tabs: watch::Sender<TabStateAvailable>,
    pub tab_metadata: broadcast::Sender<TabMetadata>,
    pub shutdown: mpsc::Sender<MainShutdown>,
}

impl Service for ClientService {
    type Rx = ClientRx;
    type Tx = ClientTx;
    type Lifeline = Self;

    fn spawn(rx: Self::Rx, mut tx: Self::Tx) -> Self {
        let _request_tab = {
            let mut rx_tab_state = rx.tab_state.clone();
            let rx_terminal_size = rx.terminal_size.clone();
            let mut tx_websocket = tx.websocket.clone();

            Self::task("request_tab", async move {
                while let Some(update) = rx_tab_state.recv().await {
                    if let TabState::Awaiting(name) = update {
                        let dimensions = rx_terminal_size.borrow().clone().0;
                        tx_websocket
                            .send(Request::CreateTab(CreateTabMetadata { name, dimensions }))
                            .await
                            .expect("send create tab");
                    }
                }
            })
        };

        let websocket_rx = WebsocketMessageRx {
            websocket: rx.websocket,
            tab_state: rx.tab_state.clone(),
            terminal_size: rx.terminal_size,
        };

        let websocket_tx = WebsocketMessageTx {
            websocket: tx.websocket.clone(),
            terminal: tx.terminal.clone(),
            active_tabs: tx.active_tabs,
            tab_metadata: tx.tab_metadata,
            shutdown: tx.shutdown,
        };
        let _websocket = WebsocketMessageService::spawn(websocket_rx, websocket_tx);

        let terminal_rx = TerminalMessageRx {
            terminal: rx.terminal,
            tab_state: rx.tab_state.clone(),
        };
        let terminal_tx = TerminalMessageTx {
            websocket: tx.websocket,
        };
        let _terminal = TerminalMessageService::spawn(terminal_rx, terminal_tx);

        ClientService {
            _request_tab,
            _websocket,
            _terminal,
        }
    }
}

struct WebsocketMessageRx {
    pub websocket: mpsc::Receiver<Response>,
    pub tab_state: watch::Receiver<TabState>,
    pub terminal_size: watch::Receiver<TerminalSizeState>,
}

struct WebsocketMessageTx {
    pub websocket: mpsc::Sender<Request>,
    pub terminal: mpsc::Sender<TerminalRecv>,
    pub active_tabs: watch::Sender<TabStateAvailable>,
    pub tab_metadata: broadcast::Sender<TabMetadata>,
    pub shutdown: mpsc::Sender<MainShutdown>,
}

struct WebsocketMessageService {
    _websocket: Lifeline,
}

impl Service for WebsocketMessageService {
    type Rx = WebsocketMessageRx;
    type Tx = WebsocketMessageTx;
    type Lifeline = Self;

    fn spawn(mut rx: Self::Rx, mut tx: Self::Tx) -> Self {
        let _websocket = Self::task("recv", async move {
            while let Some(msg) = rx.websocket.recv().await {
                match msg {
                    Response::Output(tab_id, stdout) => {
                        if rx.tab_state.borrow().is_selected(&tab_id) {
                            tx.terminal
                                .send(TerminalRecv::Stdout(stdout.data))
                                .await
                                .expect("send terminal data");
                        }
                    }
                    Response::TabUpdate(tab) => {
                        tx.tab_metadata.send(tab).expect("send tab metadata");
                    }
                    Response::TabList(tabs) => tx
                        .active_tabs
                        .broadcast(TabStateAvailable(tabs))
                        .expect("failed to update active tabs"),
                    Response::TabTerminated(id) => {
                        if rx.tab_state.borrow().is_selected(&id) {
                            tx.shutdown
                                .send(MainShutdown {})
                                .await
                                .expect("tx shutdown failed");
                        }
                    }
                    Response::Close => {}
                }
            }
        });

        Self { _websocket }
    }
}

impl WebsocketMessageService {}

struct TerminalMessageRx {
    terminal: mpsc::Receiver<TerminalSend>,
    tab_state: watch::Receiver<TabState>,
}

struct TerminalMessageTx {
    websocket: mpsc::Sender<Request>,
}

struct TerminalMessageService {
    _terminal: Lifeline,
}

impl Service for TerminalMessageService {
    type Rx = TerminalMessageRx;
    type Tx = TerminalMessageTx;
    type Lifeline = Self;

    fn spawn(mut rx: Self::Rx, mut tx: Self::Tx) -> Self {
        let _terminal = Self::task("terminal", async move {
            while let Some(msg) = rx.terminal.recv().await {
                let tab_state = rx.tab_state.borrow().clone();
                match (tab_state, msg) {
                    (TabState::Selected(id, _), TerminalSend::Stdin(data)) => {
                        let request = Request::Input(id, InputChunk { data });
                        tx.websocket
                            .send(request)
                            .await
                            .expect("failed to send websocket message")
                    }
                    _ => {}
                }
            }
        });

        Self { _terminal }
    }
}
