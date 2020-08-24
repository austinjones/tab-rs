use crate::state::terminal::TerminalMode;

use crate::bus::MainBus;
use crate::prelude::*;
use crate::{
    bus::TerminalBus,
    message::{
        main::MainShutdown,
        terminal::{TerminalRecv, TerminalSend},
    },
};
use crossterm_mode::TerminalCrosstermService;
use echo_mode::TerminalEchoService;
use lifeline::Task;
use lifeline::{dyn_bus::DynBus, Bus, Lifeline, Service};

mod crossterm_mode;
mod echo_mode;
mod terminal_event;

pub struct TerminalService {
    _main_terminal: MainTerminalCarrier,
    _terminal_mode: Lifeline,
}

enum ServiceLifeline {
    Echo(TerminalEchoService),
    Crossterm(TerminalCrosstermService),
    None,
}

impl Service for TerminalService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &MainBus) -> Self::Lifeline {
        let terminal_bus = TerminalBus::default();

        let _main_terminal = terminal_bus.carry_from(bus)?;

        let mut rx_terminal_mode = terminal_bus.rx::<TerminalMode>()?;
        // terminal_bus.take_tx::<TerminalSend, MainBus>(bus)?;
        // terminal_bus.take_channel::<TerminalRecv, MainBus>(bus)?;
        // terminal_bus.take_tx::<MainShutdown, MainBus>(bus)?;

        let _terminal_mode = Self::try_task("dispatch_mode", async move {
            let mut service = ServiceLifeline::None;

            while let Some(mode) = rx_terminal_mode.recv().await {
                service = match mode {
                    TerminalMode::Echo => {
                        if let ServiceLifeline::Echo(ref _echo) = service {
                            continue;
                        }

                        let service = TerminalEchoService::spawn(&terminal_bus)?;
                        ServiceLifeline::Echo(service)
                    }
                    TerminalMode::Crossterm => {
                        if let ServiceLifeline::Crossterm(ref _crossterm) = service {
                            continue;
                        }

                        let service = TerminalCrosstermService::spawn(&terminal_bus)?;
                        ServiceLifeline::Crossterm(service)
                    }
                }
            }

            Ok(())
        });

        Ok(Self {
            _main_terminal,
            _terminal_mode,
        })
    }
}
