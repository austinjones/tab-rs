use crate::{bus::ClientBus, state::terminal::TerminalSizeState};
use anyhow::Context;
use crossterm::event::Event;
use std::time::Duration;
use tab_service::{Bus, Lifeline, Service};
pub struct TerminalEventService {
    _update: Lifeline,
}

impl Service for TerminalEventService {
    type Bus = ClientBus;
    type Lifeline = anyhow::Result<Self>;

    #[allow(unreachable_code)]
    fn spawn(bus: &ClientBus) -> Self::Lifeline {
        let tx = bus.tx::<TerminalSizeState>()?;

        let _update = Self::try_task("run", async move {
            loop {
                let size = crossterm::terminal::size().expect("get terminal size");
                tx.broadcast(TerminalSizeState(size))
                    .context("send TerminalStateSize")?;

                tokio::time::delay_for(Duration::from_millis(200)).await;
            }

            Ok(())
        });

        Ok(Self { _update })
    }
}

fn _block_for_event() -> Option<Event> {
    if crossterm::event::poll(Duration::from_millis(500)).unwrap_or(false) {
        crossterm::event::read().ok()
    } else {
        None
    }
}
