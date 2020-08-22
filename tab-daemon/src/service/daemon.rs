// mod runtime;

use crate::bus::DaemonBus;
use listener::ListenerService;

use tab_service::Service;
use tabs::TabsService;

mod listener;
mod tab;
mod tabs;

pub struct DaemonService {
    _listener: ListenerService,
    _tabs: TabsService,
}

impl Service for DaemonService {
    type Bus = DaemonBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> anyhow::Result<Self> {
        let _listener = ListenerService::spawn(bus)?;
        let _tabs = TabsService::spawn(bus)?;
        Ok(DaemonService { _listener, _tabs })
    }
}
