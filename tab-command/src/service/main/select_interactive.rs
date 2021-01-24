use crate::{
    message::main::MainRecv,
    message::tabs::TabRecv,
    message::{main::MainShutdown, terminal::TerminalRecv},
    prelude::*,
    state::{tab::TabMetadataState, terminal::TerminalMode},
};

pub struct MainSelectInteractiveService {
    _run: Lifeline,
}

impl Service for MainSelectInteractiveService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut rx = bus.rx::<MainRecv>()?.log(Level::Debug);
        let rx_tab_state = bus.rx::<TabMetadataState>()?;

        let mut tx_terminal = bus.tx::<TerminalRecv>()?;
        let mut tx_tab = bus.tx::<TabRecv>()?;
        let mut tx_shutdown = bus.tx::<MainShutdown>()?;

        let _run = Self::try_task("run", async move {
            while let Some(msg) = rx.recv().await {
                if let MainRecv::SelectInteractive = msg {
                    info!("MainRecv::SelectInteractive running");

                    // Prevent the fuzzy finder from being used within an active session.
                    // See https://github.com/austinjones/tab-rs/issues/262
                    // Soon this will be replaced with a retask of the outer command client.
                    if std::env::var("TAB_ID").is_ok() {
                        eprintln!(
                            "You are within an active session.  Press ctrl-T to open the fuzzy finder."
                        );

                        tx_shutdown.send(MainShutdown(1)).await?;
                        continue;
                    }

                    let back = match rx_tab_state.borrow().clone() {
                        TabMetadataState::Selected(metadata) => Some(metadata.name.clone()),
                        TabMetadataState::None => None,
                    };

                    tx_terminal
                        .send(TerminalRecv::Mode(TerminalMode::FuzzyFinder(back)))
                        .await?;

                    tx_tab.send(TabRecv::DeselectTab).await?;
                    tx_tab.send(TabRecv::ScanWorkspace).await?;
                }
            }

            Ok(())
        });

        Ok(Self { _run })
    }
}
