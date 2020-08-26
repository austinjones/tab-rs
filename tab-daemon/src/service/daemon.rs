use crate::prelude::*;
use listener::ListenerService;

use lifeline::prelude::*;

mod listener;
mod tab_manager;

pub struct DaemonService {
    _listener: ListenerService,
}

impl Service for DaemonService {
    type Bus = DaemonBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> anyhow::Result<Self> {
        let _listener = ListenerService::spawn(bus)?;
        Ok(DaemonService { _listener })
    }
}
