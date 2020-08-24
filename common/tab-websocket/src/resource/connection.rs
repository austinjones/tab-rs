use crate::WebsocketConnection;
use futures::executor::block_on;
use lifeline::impl_storage_take;
use log::error;

#[derive(Debug)]
pub struct WebsocketResource(pub WebsocketConnection);

impl Drop for WebsocketResource {
    fn drop(&mut self) {
        match block_on(self.0.close(None)) {
            Ok(_) => {}
            Err(err) => match err {
                tungstenite::Error::ConnectionClosed => {}
                tungstenite::Error::AlreadyClosed => {}
                _ => error!("failed to close websocket: {}", err),
            },
        }
    }
}

impl_storage_take!(WebsocketResource);
