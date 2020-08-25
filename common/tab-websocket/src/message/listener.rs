use crate::bus::WebsocketConnectionBus;
use tungstenite::http::{Method, Uri};

#[derive(Debug)]
pub struct WebsocketConnectionMessage {
    pub bus: WebsocketConnectionBus,
    pub request: RequestMetadata,
}

#[derive(Debug, Clone)]
pub struct RequestMetadata {
    pub method: Method,
    pub uri: Uri,
}
