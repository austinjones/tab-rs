use crate::prelude::*;

use super::pty::PtyService;
use dyn_bus::DynBus;
use tab_websocket::{
    bus::{WebsocketCarrier, WebsocketConnectionBus},
    resource::connection::WebsocketResource,
};

pub struct MainService {
    _pty: PtyService,
    _carrier: WebsocketCarrier,
}

impl Service for MainService {
    type Bus = PtyBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let websocket = bus.resource::<WebsocketResource>()?;
        let websocket_connection_bus = WebsocketConnectionBus::default();
        websocket_connection_bus.store_resource(websocket);

        let _carrier = websocket_connection_bus.carry_from(bus)?;

        debug!("Launching MainService");
        let _pty = PtyService::spawn(&bus)?;

        Ok(Self { _pty, _carrier })
    }
}
