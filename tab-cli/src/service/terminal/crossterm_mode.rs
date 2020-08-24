use crate::bus::TerminalBus;

use lifeline::Service;

pub struct TerminalCrosstermService {}

impl Service for TerminalCrosstermService {
    type Bus = TerminalBus;
    type Lifeline = anyhow::Result<Self>;
    fn spawn(_bus: &Self::Bus) -> Self::Lifeline {
        Ok(Self {})
    }
}
