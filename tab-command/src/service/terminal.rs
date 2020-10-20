use crate::{state::terminal::TerminalMode};

use crate::bus::MainBus;
use crate::prelude::*;

use crate::{
    bus::TerminalBus,
    message::terminal::{TerminalInput, TerminalOutput},
};

use echo_mode::TerminalEchoService;
use terminal_event::TerminalEventService;

mod echo_mode;
mod fuzzy;
mod terminal_event;

pub use echo_mode::disable_raw_mode;
pub use echo_mode::reset_cursor;

use self::fuzzy::FuzzyFinderService;

/// Reads TerminalMode, and launches/cancels the TerminalEchoService / TerminalCrosstermService
pub struct TerminalService {
    _main_terminal: MainTerminalCarrier,
    _terminal_mode: Lifeline,
    _terminal_event: TerminalEventService,
}

enum ServiceLifeline {
    Echo(TerminalEchoService),
    FuzzyFinder(FuzzyFinderService, TerminalFuzzyCarrier),
    None,
}

impl Service for TerminalService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &MainBus) -> Self::Lifeline {
        let terminal_bus = TerminalBus::default();
        terminal_bus.capacity::<TerminalInput>(2048)?;
        terminal_bus.capacity::<TerminalOutput>(2048)?;

        let _main_terminal = terminal_bus.carry_from(bus)?;
        let _terminal_event = TerminalEventService::spawn(&terminal_bus)?;

        let mut rx = terminal_bus.rx::<TerminalMode>()?;

        let _terminal_mode = Self::try_task("dispatch_mode", async move {
            let mut service = ServiceLifeline::None;

            while let Some(mode) = rx.recv().await {
                service = match mode {
                    TerminalMode::None => ServiceLifeline::None,
                    TerminalMode::Echo => {
                        if let ServiceLifeline::Echo(ref _echo) = service {
                            continue;
                        }

                        info!("TerminalService switching to echo mode");

                        let service = TerminalEchoService::spawn(&terminal_bus)?;
                        ServiceLifeline::Echo(service)
                    }
                    TerminalMode::FuzzyFinder => {
                        if let ServiceLifeline::FuzzyFinder(ref _fuzzy, ref _carrier) = service {
                            continue;
                        }

                        info!("TerminalService switching to fuzzy finder mode");

                        let fuzzy_bus = FuzzyBus::default();
                        let carrier = fuzzy_bus.carry_from(&terminal_bus)?;

                        let service = FuzzyFinderService::spawn(&fuzzy_bus)?;
                        ServiceLifeline::FuzzyFinder(service, carrier)
                    }
                }
            }

            Ok(())
        });

        Ok(Self {
            _main_terminal,
            _terminal_mode,
            _terminal_event,
        })
    }
}
