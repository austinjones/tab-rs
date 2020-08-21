// mod session;

use crate::{bus::ConnectionBus, message::connection::ConnectionSend};
use tab_service::{Bus, Service};

pub struct ConnectionService {}

impl Service for ConnectionService {
    type Bus = ConnectionBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let _tx = bus.tx::<ConnectionSend>()?;
        Ok(ConnectionService {})
    }
}
