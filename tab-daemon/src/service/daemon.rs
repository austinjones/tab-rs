// mod runtime;

use crate::bus::DaemonBus;
use listener::WebsocketService;

use tab_service::{Service};

mod listener;
mod tab;
mod tabs;

pub struct DaemonService {
    _listener: WebsocketService,
}

impl Service for DaemonService {
    type Bus = DaemonBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> anyhow::Result<Self> {
        let _listener = WebsocketService::spawn(bus)?;
        Ok(DaemonService { _listener })
    }
}
