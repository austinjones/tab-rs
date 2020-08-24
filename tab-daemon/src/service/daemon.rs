// mod runtime;

use crate::prelude::*;
use listener::ListenerService;

use lifeline::Service;

mod listener;
mod tab;
mod tabs;

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
