use crate::bus::WebsocketConnectionBus;
use crate::service::WebsocketService;

#[derive(Debug)]
pub struct WebsocketConnectionMessage {
    pub bus: WebsocketConnectionBus,
    pub lifeline: WebsocketService,
}
