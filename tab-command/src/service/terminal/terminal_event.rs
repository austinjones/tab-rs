use crate::prelude::*;
use crate::{
    env::terminal_size, message::terminal::TerminalInput, state::terminal::TerminalSizeState,
};
use anyhow::Context;
use crossterm::event::Event;
use std::time::Duration;

/// Broadcasts the TerminalSizeState, and sends TerminalSend::Resize events.
pub struct TerminalEventService {
    _update: Lifeline,
}

impl Service for TerminalEventService {
    type Bus = TerminalBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &TerminalBus) -> Self::Lifeline {
        let mut tx = bus.tx::<TerminalSizeState>()?;
        let mut tx_send = bus.tx::<TerminalInput>()?;

        #[allow(unreachable_code)]
        let _update = Self::try_task("run", async move {
            let mut set_size = (0, 0);
            loop {
                let size = terminal_size().expect("get terminal size");
                if size != set_size {
                    set_size = size;

                    tx.send(TerminalSizeState(set_size))
                        .await
                        .context("send TerminalStateSize")?;

                    tx_send
                        .send(TerminalInput::Resize(set_size))
                        .await
                        .context("send TerminalStateSize")?;
                }

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
