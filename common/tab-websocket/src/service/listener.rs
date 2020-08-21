use super::WebsocketService;
use crate::bus::WebsocketConnectionBus;
use crate::{
    bind,
    bus::WebsocketListenerBus,
    message::listener::WebsocketConnectionMessage,
    resource::{connection::WebsocketResource, listener::WebsocketListenerResource},
};
use log::{debug, error, info};
use tab_service::{dyn_bus::DynBus, Bus, Lifeline, Service};
use tokio::{net::TcpListener, sync::mpsc};
pub struct WebsocketListenerService {
    _accept: Lifeline,
}

impl Service for WebsocketListenerService {
    type Bus = WebsocketListenerBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let listener = bus.resource::<WebsocketListenerResource>()?;

        let tx = bus.tx::<WebsocketConnectionMessage>()?;
        let _accept = Self::task("accept", accept_connections(listener.0, tx));

        todo!()
    }
}

async fn accept_connections(
    mut listener: TcpListener,
    mut tx: mpsc::Sender<WebsocketConnectionMessage>,
) {
    loop {
        let connect = listener.accept().await;

        match connect {
            Ok((stream, _addr)) => {
                // TODO: only accept connections from loopback address
                debug!("connection opened from {:?}", _addr);

                let conn_bus = WebsocketConnectionBus::default();
                let bound = match bind(stream).await {
                    Ok(res) => res,
                    Err(e) => {
                        error!("error binding websocket: {}", e);
                        continue;
                    }
                };

                conn_bus.store_resource(WebsocketResource(bound));
                let service = WebsocketService::spawn(&conn_bus).expect("websocket spawn failed");

                let message = WebsocketConnectionMessage {
                    bus: conn_bus,
                    lifeline: service,
                };

                tx.send(message).await.expect("tx failed");
            }
            Err(e) => {
                error!("tcp connection failed: {}", e);
                break;
            }
        }
    }
}
