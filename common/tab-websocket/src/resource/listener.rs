use lifeline::{impl_storage_clone, impl_storage_take};
use tokio::net::TcpListener;

/// A resource which wraps an established TCP listener.  Taken from the bus
#[derive(Debug)]
pub struct WebsocketListenerResource(pub TcpListener);

impl_storage_take!(WebsocketListenerResource);

/// A resource which defines an authentication token.  When present with a Some value,
/// connections must provide this token in the Authorization header.
#[derive(Debug, Clone)]
pub struct WebsocketAuthToken(pub Option<String>);

impl_storage_clone!(WebsocketAuthToken);

impl WebsocketAuthToken {
    pub fn new(token: Option<String>) -> Self {
        Self(token)
    }
}

impl From<&str> for WebsocketAuthToken {
    fn from(str: &str) -> Self {
        WebsocketAuthToken(Some(str.to_string()))
    }
}

impl From<String> for WebsocketAuthToken {
    fn from(str: String) -> Self {
        WebsocketAuthToken(Some(str))
    }
}

impl WebsocketAuthToken {
    pub fn unauthenticated() -> Self {
        Self(None)
    }
}
