use lifeline::{impl_storage_clone, impl_storage_take, prelude::*};
use tokio::net::TcpListener;

#[derive(Debug)]
pub struct WebsocketListenerResource(pub TcpListener);

impl_storage_take!(WebsocketListenerResource);

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
