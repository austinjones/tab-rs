use crate::bus::DaemonBus;
use tab_service::Service;

pub struct TabsService {}

impl Service for TabsService {
    type Bus = DaemonBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        todo!()
    }
}
