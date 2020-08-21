use tab_service::impl_storage_take;
use tokio::net::TcpListener;

#[derive(Debug)]
pub struct WebsocketListenerResource(pub TcpListener);

impl_storage_take!(WebsocketListenerResource);
