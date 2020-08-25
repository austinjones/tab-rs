use super::{tab_state::TabStateService, tabs::TabsStateService, terminal::TerminalService};
use crate::prelude::*;
use crate::{
    bus::MainBus,
    message::{
        main::{MainRecv, MainShutdown},
        tabs::TabsRecv,
        terminal::{TerminalRecv, TerminalSend},
    },
    state::{tab::TabStateSelect, tabs::TabsState, terminal::TerminalMode},
};
use lifeline::{dyn_bus::DynBus, Bus, Lifeline, Service};

use crate::prelude::*;
use lifeline::Task;

use tab_websocket::{
    bus::{WebsocketCarrier, WebsocketConnectionBus},
    message::connection::{WebsocketRecv, WebsocketSend},
    resource::connection::WebsocketResource,
    service::WebsocketService,
};
use tokio::stream::StreamExt;

pub struct MainService {
    _main: Lifeline,
    _main_tab: MainTabCarrier,
    _main_websocket: WebsocketCarrier,
    _tab_state: TabStateService,
    _tabs_state: TabsStateService,
    _terminal: TerminalService,
    _close_tab: CloseTabService,
}

impl Service for MainService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(main_bus: &MainBus) -> anyhow::Result<Self> {
        let tab_bus = TabBus::default();

        let websocket_bus = WebsocketConnectionBus::default();
        let websocket = main_bus.resource::<WebsocketResource>()?;
        websocket_bus.store_resource(websocket);
        let _main_websocket = websocket_bus.carry_from(main_bus)?;

        let _main_tab = tab_bus.carry_from(main_bus)?;

        let mut rx_main = main_bus.rx::<MainRecv>()?;
        let _main = Self::try_task("main_recv", async move {
            while let Some(msg) = rx_main.next().await {
                debug!("MainRecv: {:?}", &msg);
                // all the event types are handled by carriers
            }

            Ok(())
        });

        let _tab_state = TabStateService::spawn(&tab_bus)?;
        let _tabs_state = TabsStateService::spawn(&tab_bus)?;
        let _terminal = TerminalService::spawn(&main_bus)?;
        let _close_tab = CloseTabService::spawn(&main_bus)?;

        Ok(Self {
            _main,
            _main_tab,
            _main_websocket,
            _tab_state,
            _tabs_state,
            _terminal,
            _close_tab,
        })
    }
}

impl MainService {}

pub struct CloseTabService {
    _on_close: Lifeline,
}

impl Service for CloseTabService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;
    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut rx_main = bus.rx::<MainRecv>()?;

        let tx_request = bus.tx::<Request>()?;
        let mut tx_shutdown = bus.tx::<MainShutdown>()?;

        let _on_close = Self::try_task("on_close", async move {
            while let Some(msg) = rx_main.next().await {
                if msg.is_err() {
                    continue;
                }

                let msg = msg.unwrap();

                match msg {
                    MainRecv::CloseTab(name) => {
                        tx_request
                            .send(Request::CloseNamedTab(name))
                            .map_err(into_msg)?;
                        tx_shutdown.send(MainShutdown {}).await?;
                    }
                    _ => {}
                }
            }

            Ok(())
        });

        Ok(Self { _on_close })
    }
}
