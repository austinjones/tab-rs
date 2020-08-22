use crate::bus::TerminalBus;
use tab_service::Service;

pub struct TerminalCrosstermService {}

impl Service for TerminalCrosstermService {
    type Bus = TerminalBus;
    type Lifeline = anyhow::Result<Self>;
    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        Ok(Self {})
    }
}
