use std::{
    io::Write,
    mem::discriminant,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::state::terminal::TerminalMode;

use crate::bus::MainBus;
use crate::prelude::*;

use crate::{
    bus::TerminalBus,
    message::terminal::{TerminalInput, TerminalOutput},
};

use echo_mode::TerminalEchoService;
use tab_api::env::is_raw_mode;
use terminal_event::TerminalEventService;

mod echo_input;
mod echo_mode;
mod fuzzy;
mod terminal_event;

use self::fuzzy::FuzzyFinderService;

static RESET_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn enable_raw_mode(reset_enabled: bool) {
    if is_raw_mode() {
        crossterm::terminal::enable_raw_mode().expect("failed to enable raw mode");
        if reset_enabled {
            RESET_ENABLED.store(true, Ordering::SeqCst);
            debug!("raw mode enabled");
        }
    }
}

pub fn disable_raw_mode() {
    crossterm::terminal::disable_raw_mode().expect("failed to disable raw mode");
    debug!("raw mode disabled");
}

pub fn reset_terminal_state() {
    if is_raw_mode() && RESET_ENABLED.load(Ordering::SeqCst) {
        let mut stdout = std::io::stdout();

        // fully reset the terminal state: ESC c
        // then clear the terminal: ESC [ 2 J
        stdout
            .write("\x1bc\x1b[2J".as_bytes())
            .expect("failed to queue reset command");

        stdout.flush().expect("failed to flush reset commands");

        RESET_ENABLED.store(false, Ordering::SeqCst);

        debug!("terminal state reset");
    }
}

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
            let mut current_mode = TerminalMode::None;
            let mut service = ServiceLifeline::None;

            while let Some(mode) = rx.recv().await {
                // here we need to deal with a lot of terminal state.
                // the spawned services hold stdin/stdout references, and have actor state.
                // and the mode itself represents some state (the selected tab)

                // if the mode discriminant has changed, we restart the service
                // if the new mode is not strictly equal, we clear the terminal.

                // this allows mode transitions (fuzzy to tab), as well as tab switches

                let restart = discriminant(&current_mode) != discriminant(&mode);
                let reset_terminal = mode != current_mode;

                if reset_terminal && restart {
                    drop(service);
                    reset_terminal_state();
                    Self::set_raw_mode(&mode);

                    service = Self::spawn_service(&mode, &terminal_bus)?;
                } else if reset_terminal {
                    reset_terminal_state();
                    Self::set_raw_mode(&mode);
                }

                current_mode = mode;
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

impl TerminalService {
    fn set_raw_mode(mode: &TerminalMode) {
        match mode {
            TerminalMode::Echo(_) => {
                enable_raw_mode(true);
            }
            TerminalMode::FuzzyFinder => {
                enable_raw_mode(false);
            }
            TerminalMode::None => {
                disable_raw_mode();
            }
        }
    }

    fn spawn_service(
        mode: &TerminalMode,
        terminal_bus: &TerminalBus,
    ) -> anyhow::Result<ServiceLifeline> {
        let service = match mode {
            TerminalMode::None => ServiceLifeline::None,
            TerminalMode::Echo(ref name) => {
                info!("TerminalService switching to echo mode for tab {}", name);

                let service = TerminalEchoService::spawn(&terminal_bus)?;
                ServiceLifeline::Echo(service)
            }
            TerminalMode::FuzzyFinder => {
                info!("TerminalService switching to fuzzy finder mode");

                let fuzzy_bus = FuzzyBus::default();
                let carrier = fuzzy_bus.carry_from(&terminal_bus)?;

                let service = FuzzyFinderService::spawn(&fuzzy_bus)?;
                ServiceLifeline::FuzzyFinder(service, carrier)
            }
        };

        Ok(service)
    }
}
