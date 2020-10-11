use super::{
    create_tab::CreateTabService, fuzzy::FuzzyFinderService, tab_state::TabStateService,
    tabs::TabsStateService, terminal::TerminalService, workspace::WorkspaceService,
};
use crate::prelude::*;
use crate::{
    bus::MainBus,
    message::main::{MainRecv, MainShutdown},
};

use lifeline::dyn_bus::DynBus;

use tab_api::tab::TabMetadata;
use tab_websocket::{
    bus::{WebsocketCarrier, WebsocketConnectionBus},
    resource::connection::WebsocketResource,
};

/// Launches the tab-command client, including websocket, tab state, and terminal services.
pub struct MainService {
    _main: Lifeline,
    _main_tab: MainTabCarrier,
    _main_websocket: WebsocketCarrier,
    _workspace: WorkspaceService,
    _create_tab: CreateTabService,
    _tab_state: TabStateService,
    _tabs_state: TabsStateService,
    _main_fuzzy_carrier: MainFuzzyCarrier,
    _tab_fuzzy_carrier: TabFuzzyCarrier,
    _fuzzy_finder: FuzzyFinderService,
    _terminal: TerminalService,
}

impl Service for MainService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(main_bus: &MainBus) -> anyhow::Result<Self> {
        let tab_bus = TabBus::default();
        tab_bus.capacity::<TabMetadata>(256)?;

        let _main_tab = tab_bus.carry_from(main_bus)?;

        let websocket_bus = WebsocketConnectionBus::default();
        let websocket = main_bus.resource::<WebsocketResource>()?;
        websocket_bus.store_resource(websocket);
        let _main_websocket = websocket_bus.carry_from(main_bus)?;

        let mut rx_main = main_bus.rx::<MainRecv>()?;

        let mut tx_websocket = main_bus.tx::<Request>()?;
        let mut tx_shutdown = main_bus.tx::<MainShutdown>()?;
        let _main = Self::try_task("main_recv", async move {
            while let Some(msg) = rx_main.recv().await {
                debug!("MainRecv: {:?}", &msg);
                // all the event types are handled by carriers
                if let MainRecv::GlobalShutdown = msg {
                    tx_websocket.send(Request::GlobalShutdown).await?;
                    tx_shutdown.send(MainShutdown {}).await?;
                }
            }

            Ok(())
        });

        let _tab_state = TabStateService::spawn(&tab_bus)?;
        let _workspace = WorkspaceService::spawn(&tab_bus)?;
        let _create_tab = CreateTabService::spawn(&tab_bus)?;
        let _tabs_state = TabsStateService::spawn(&tab_bus)?;
        let _terminal = TerminalService::spawn(&main_bus)?;

        let fuzzy_bus = FuzzyBus::default();
        let _main_fuzzy_carrier = fuzzy_bus.carry_from(main_bus)?;
        let _tab_fuzzy_carrier = fuzzy_bus.carry_from(&tab_bus)?;
        let _fuzzy_finder = FuzzyFinderService::spawn(&fuzzy_bus)?;

        Ok(Self {
            _main,
            _main_tab,
            _main_websocket,
            _main_fuzzy_carrier,
            _tab_fuzzy_carrier,
            _fuzzy_finder,
            _workspace,
            _create_tab,
            _tab_state,
            _tabs_state,
            _terminal,
        })
    }
}

impl MainService {}
