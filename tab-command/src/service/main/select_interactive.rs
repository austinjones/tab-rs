use lifeline::barrier::barrier;

use crate::{
    message::main::MainRecv, message::tabs::TabRecv, message::terminal::TerminalRecv, prelude::*,
    state::tabs::ActiveTabsState, state::terminal::TerminalMode, state::workspace::WorkspaceState,
    utils::await_state,
};

pub struct MainSelectInteractiveService {
    _run: Lifeline,
}

impl Service for MainSelectInteractiveService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut rx = bus.rx::<MainRecv>()?.log();

        let mut tx_terminal = bus.tx::<TerminalRecv>()?;
        let mut tx_tab = bus.tx::<TabRecv>()?;

        info!("MainRecv::SelectInteractive spawning");
        let _run = Self::try_task("run", async move {
            while let Some(msg) = rx.recv().await {
                if let MainRecv::SelectInteractive = msg {
                    info!("MainRecv::SelectInteractive running");
                    tx_terminal
                        .send(TerminalRecv::Mode(TerminalMode::FuzzyFinder))
                        .await?;

                    tx_tab.send(TabRecv::ScanWorkspace).await?;
                }
            }

            Ok(())
        });

        Ok(Self { _run })
    }
}
