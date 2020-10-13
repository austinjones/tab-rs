use self::{
    autocomplete_close_tab::MainAutocompleteCloseTabsService,
    autocomplete_tab::MainAutocompleteTabsService, close_tabs::MainCloseTabsService,
    global_shutdown::MainGlobalShutdownService, list_tabs::MainListTabsService,
    select_interactive::MainSelectInteractiveService, select_tab::MainSelectTabService,
};

use super::{
    tab::active_tabs::ActiveTabsService, tab::create_tab::CreateTabService,
    tab::select_tab::SelectTabService, tab::tab_state::TabStateService,
    tab::workspace::WorkspaceService, terminal::TerminalService,
};
use crate::bus::MainBus;
use crate::prelude::*;

use lifeline::dyn_bus::DynBus;

use tab_api::tab::TabMetadata;
use tab_websocket::{
    bus::{WebsocketCarrier, WebsocketConnectionBus},
    resource::connection::WebsocketResource,
};

mod autocomplete_close_tab;
mod autocomplete_tab;
mod close_tabs;
mod global_shutdown;
mod list_tabs;
mod select_interactive;
mod select_tab;

/// Launches the tab-command client, including websocket, tab state, and terminal services.
pub struct MainService {
    _main_autocomplete_close: MainAutocompleteCloseTabsService,
    _main_autocomplete: MainAutocompleteTabsService,
    _main_close_tabs: MainCloseTabsService,
    _main_list_tabs: MainListTabsService,
    _main_select_interactive: MainSelectInteractiveService,
    _main_select_tab: MainSelectTabService,
    _main_global_shutdown: MainGlobalShutdownService,
    _main_tab: MainTabCarrier,
    _main_websocket: WebsocketCarrier,
    _select_tab: SelectTabService,
    _workspace: WorkspaceService,
    _create_tab: CreateTabService,
    _tab_state: TabStateService,
    _tabs_state: ActiveTabsService,
    _terminal: TerminalService,
}

impl Service for MainService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(main_bus: &MainBus) -> anyhow::Result<Self> {
        let _main_autocomplete_close = MainAutocompleteCloseTabsService::spawn(main_bus)?;
        let _main_autocomplete = MainAutocompleteTabsService::spawn(main_bus)?;
        let _main_close_tabs = MainCloseTabsService::spawn(main_bus)?;
        let _main_list_tabs = MainListTabsService::spawn(main_bus)?;
        let _main_select_interactive = MainSelectInteractiveService::spawn(main_bus)?;
        let _main_select_tab = MainSelectTabService::spawn(main_bus)?;
        let _main_global_shutdown = MainGlobalShutdownService::spawn(main_bus)?;

        let tab_bus = TabBus::default();
        tab_bus.capacity::<TabMetadata>(256)?;

        let _main_tab = tab_bus.carry_from(main_bus)?;

        let websocket_bus = WebsocketConnectionBus::default();
        let websocket = main_bus.resource::<WebsocketResource>()?;
        websocket_bus.store_resource(websocket);
        let _main_websocket = websocket_bus.carry_from(main_bus)?;

        let _select_tab = SelectTabService::spawn(&tab_bus)?;
        let _tab_state = TabStateService::spawn(&tab_bus)?;
        let _workspace = WorkspaceService::spawn(&tab_bus)?;
        let _create_tab = CreateTabService::spawn(&tab_bus)?;
        let _tabs_state = ActiveTabsService::spawn(&tab_bus)?;
        let _terminal = TerminalService::spawn(&main_bus)?;

        Ok(Self {
            _main_autocomplete_close,
            _main_autocomplete,
            _main_close_tabs,
            _main_list_tabs,
            _main_select_interactive,
            _main_select_tab,
            _main_global_shutdown,
            _main_tab,
            _main_websocket,
            _select_tab,
            _workspace,
            _create_tab,
            _tab_state,
            _tabs_state,
            _terminal,
        })
    }
}

impl MainService {}
