use crate::bus::DaemonBus;
use tab_service::Service;
pub struct TabService {}

impl Service for TabService {
    type Bus = DaemonBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(_bus: &Self::Bus) -> Self::Lifeline {
        todo!()
    }
}
