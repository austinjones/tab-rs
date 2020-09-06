use crate::bus::WebsocketConnectionBus;
use tungstenite::http::{Method, Uri};

/// A connection event on a websocket server listener.
/// Provides a bus, which can be carried into your bus (which must implement WebsocketConnectionBus),
/// as well as metadata about the request.
///
/// If the listener specified an auth token, this request has been authenticated.
#[derive(Debug)]
pub struct WebsocketConnectionMessage {
    pub bus: WebsocketConnectionBus,
    pub request: RequestMetadata,
}

/// Metadata about the HTTP request which initialized the connection.
#[derive(Debug, Clone)]
pub struct RequestMetadata {
    pub method: Method,
    pub uri: Uri,
}
