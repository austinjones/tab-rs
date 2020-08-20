use crate::{bus::client::ClientBus, state::terminal::TerminalSizeState};
use crossterm::event::Event;
use std::time::Duration;
use tab_service::{Bus, Lifeline, Service};
pub struct TerminalEventService {
    _update: Lifeline,
}

impl Service for TerminalEventService {
    type Bus = ClientBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &ClientBus) -> Self::Lifeline {
        let tx = bus.tx::<TerminalSizeState>()?;

        let _update = Self::task("run", async move {
            let size = crossterm::terminal::size().expect("get terminal size");
            tx.broadcast(TerminalSizeState(size))
                .expect("failed to send terminal size");
        });

        Ok(Self { _update })
    }
}

fn block_for_event() -> Option<Event> {
    if crossterm::event::poll(Duration::from_millis(500)).unwrap_or(false) {
        crossterm::event::read().ok()
    } else {
        None
    }
}
