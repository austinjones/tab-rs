use super::{
    client::ClientService, tab_state::TabStateService, tabs::TabsStateService,
    terminal::TerminalService,
};
use crate::bus::ClientBus;
use crate::{
    bus::MainBus,
    message::{
        main::{MainRecv, MainShutdown},
        tabs::TabsRecv,
        terminal::{TerminalRecv, TerminalSend},
    },
    state::{tab::TabStateSelect, tabs::TabsState, terminal::TerminalMode},
};
use log::{debug, error};
use tab_api::{
    request::Request,
    response::Response,
    tab::{TabId, TabMetadata},
};
use tab_service::{dyn_bus::DynBus, Bus, Lifeline, Service};

use std::collections::HashMap;
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
    _tabs_state: TabsStateService,
    _websocket_send: Lifeline,
    _websocket_recv: Lifeline,
}

impl Service for MainService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &MainBus) -> anyhow::Result<Self> {
        let websocket_bus = WebsocketConnectionBus::default();
        websocket_bus.take_resource::<WebsocketResource, _>(bus)?;

        let tx_terminal_mode = bus.tx::<TerminalMode>()?;
        let mut main_rx = bus.rx::<MainRecv>()?;
        let mut tx_shutdown = bus.tx::<MainShutdown>()?;
        let mut rx_tabs_state = bus.rx::<TabsState>()?;

        let client_bus = ClientBus::default();
        client_bus.take_tx::<MainShutdown, MainBus>(bus)?;

        client_bus.take_channel::<TerminalSend, MainBus>(bus)?;
        client_bus.take_tx::<TerminalRecv, MainBus>(bus)?;

        client_bus.take_tx::<TabsRecv, MainBus>(bus)?;

        let tx_select_tab = client_bus.tx::<TabStateSelect>()?;
        let mut tx_websocket = client_bus.tx::<Request>()?;

        let _tab_state = TabStateService::spawn(&client_bus)?;
        let _tabs_state = TabsStateService::spawn(&bus)?;
        let _main = Self::try_task("main_recv", async move {
            while let Some(msg) = main_rx.recv().await {
                debug!("MainRecv: {:?}", &msg);

                match msg {
                    MainRecv::SelectTab(name) => {
                        tx_terminal_mode.broadcast(TerminalMode::Echo)?;

                        tx_select_tab
                            .broadcast(TabStateSelect::Selected(name))
                            .map_err(|_err| anyhow::Error::msg("send TabStateSelect"))?;
                    }
                    MainRecv::SelectInteractive => {
                        tx_terminal_mode.broadcast(TerminalMode::Crossterm)?;
                    }
                    MainRecv::ListTabs => {
                        while let Some(state) = rx_tabs_state.recv().await {
                            if !state.initialized {
                                continue;
                            }

                            Self::echo_tabs(&state.tabs);

                            tx_shutdown.send(MainShutdown {}).await?;
                        }
                    }
                    MainRecv::AutocompleteTab(complete) => {
                        while let Some(state) = rx_tabs_state.recv().await {
                            if !state.initialized {
                                continue;
                            }

                            Self::echo_completion(&state.tabs, complete.as_str());

                            tx_shutdown.send(MainShutdown {}).await?;
                        }
                    }
                    MainRecv::CloseTab(name) => {
                        while let Some(state) = rx_tabs_state.recv().await {
                            if !state.initialized {
                                continue;
                            }

                            let found = state.tabs.values().find(|tab| &tab.name == &name);
                            match found {
                                Some(tab) => {
                                    tx_websocket.send(Request::CloseTab(tab.id)).await?;
                                    println!("Tab '{}' closed.", name);
                                }
                                None => {
                                    println!("No tab found with name: '{}'", name);
                                }
                            }

                            break;
                        }

                        tx_shutdown.send(MainShutdown {}).await?;
                    }
                }
            }

            Ok(())
        });

        let _websocket = WebsocketService::spawn(&websocket_bus)?;
        let _websocket_send = {
            let mut rx = client_bus.rx::<Request>()?;
            let mut tx = websocket_bus.tx::<WebsocketSend>()?;

            Self::try_task("forward_request", async move {
                while let Some(req) = rx.recv().await {
                    match bincode::serialize(&req) {
                        Ok(vec) => {
                            tx.send(WebsocketSend(TungsteniteMessage::Binary(vec)))
                                .await?
                        }
                        Err(e) => error!("failed to send websocket msg: {}", e),
                    };
                }

                tx.send(WebsocketSend(TungsteniteMessage::Close(None)))
                    .await?;

                Ok(())
            })
        };

        let _websocket_recv = {
            let mut rx = websocket_bus.rx::<WebsocketRecv>()?;
            let mut tx = client_bus.tx::<Response>()?;

            Self::try_task("forward_request", async move {
                while let Some(resp) = rx.recv().await {
                    match bincode::deserialize(resp.0.into_data().as_slice()) {
                        Ok(resp) => tx.send(resp).await?,
                        Err(e) => error!("failed to send websocket msg: {}", e),
                    };
                }

                Ok(())
            })
        };

        let _client = ClientService::spawn(&client_bus)?;
        let _terminal = TerminalService::spawn(&bus)?;

        Ok(Self {
            _main,
            _client,
            _websocket,
            _terminal,
            _tab_state,
            _tabs_state,
            _websocket_send,
            _websocket_recv,
        })
    }
}

impl MainService {
    pub fn echo_tabs(tabs: &HashMap<TabId, TabMetadata>) {
        debug!("echo tabs: {:?}", tabs);

        let mut names: Vec<&str> = tabs.values().map(|v| v.name.as_str()).collect();
        names.sort();

        if names.len() == 0 {
            println!("No active tabs.");
            return;
        }

        println!("Available tabs:");
        for name in names {
            println!("\t{}", name);
        }
    }

    pub fn echo_completion(tabs: &HashMap<TabId, TabMetadata>, completion: &str) {
        debug!("echo completion: {:?}, {}", tabs, completion);

        let mut names: Vec<&str> = tabs
            .values()
            .map(|v| v.name.as_str())
            .filter(|name| name.starts_with(completion))
            .collect();

        names.sort_by(|a, b| {
            let a_len = Self::overlap(a, completion);
            let b_len = Self::overlap(b, completion);

            a_len.cmp(&b_len)
        });

        for name in names {
            println!("{}", name);
        }
    }

    fn overlap(value: &str, target: &str) -> usize {
        let mut matches = 0;
        for (a, b) in value.chars().zip(target.chars()) {
            if a != b {
                return matches;
            }

            matches += 1;
        }

        return matches;
    }
}
