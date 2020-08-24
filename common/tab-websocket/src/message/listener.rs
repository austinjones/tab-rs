use crate::bus::WebsocketConnectionBus;

#[derive(Debug)]
pub struct WebsocketConnectionMessage {
    pub bus: WebsocketConnectionBus,
    // pub lifeline: WebsocketService,
}
