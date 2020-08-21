use crate::WebsocketConnection;
use futures::executor::block_on;
use tab_service::impl_storage_take;

#[derive(Debug)]
pub struct WebsocketResource(pub WebsocketConnection);

impl Drop for WebsocketResource {
    fn drop(&mut self) {
        block_on(self.0.close(None)).expect("websocket drop failed");
    }
}

impl_storage_take!(WebsocketResource);
