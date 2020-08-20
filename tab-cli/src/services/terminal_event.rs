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
            let mut size = crossterm::terminal::size().expect("get terminal size");
            tx.broadcast(TerminalSizeState(size))
                .expect("failed to send terminal size");

            // loop {
            //     let new_size = crossterm::terminal::size().expect("get terminal size");
            //     let msg = tokio::task::spawn_blocking(|| block_for_event())
            //         .await
            //         .expect("failed to get crossterm event");

            //     if !msg.is_some() {
            //         continue;
            //     }

            //     if let Event::Resize(width, height) = msg.unwrap() {
            //         let new_size = (height, width);
            //         if new_size != size {
            //             size = new_size;
            //             tx.size
            //                 .broadcast(TerminalSizeState(new_size))
            //                 .expect("send terminal size");
            //         }
            //     }
            // }
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
