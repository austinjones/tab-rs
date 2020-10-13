use crate::{
    message::main::MainRecv, message::terminal::TerminalRecv, message::terminal::TerminalSend,
    prelude::*, state::tabs::ActiveTabsState, state::terminal::TerminalMode,
    state::workspace::WorkspaceState, utils::await_message, utils::await_state,
};

pub struct MainSelectInteractiveService {
    _run: Lifeline,
}

impl Service for MainSelectInteractiveService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut rx = bus.rx::<MainRecv>()?;
        let mut rx_active_tabs = bus.rx::<Option<ActiveTabsState>>()?.into_inner();
        let mut rx_workspace = bus.rx::<Option<WorkspaceState>>()?.into_inner();
        let mut rx_terminal = bus.rx::<TerminalSend>()?;
        let mut tx_terminal = bus.tx::<TerminalRecv>()?;

        let mut tx_main = bus.tx::<MainRecv>()?;

        let _run = Self::try_task("run", async move {
            while let Some(msg) = rx.recv().await {
                if let MainRecv::SelectInteractive = msg {
                    tx_terminal
                        .send(TerminalRecv::Mode(TerminalMode::FuzzyFinder))
                        .await?;

                    let active_tabs = await_state(&mut rx_active_tabs).await?;
                    let workspace = await_state(&mut rx_workspace)
                        .await?
                        .with_active_tabs(&active_tabs);

                    tx_terminal
                        .send(TerminalRecv::FuzzyTabs(workspace.tabs))
                        .await?;

                    let tab = await_message(&mut rx_terminal, |msg| msg.fuzzy_selection()).await?;
                    tx_main.send(MainRecv::SelectTab(tab)).await?;
                }
            }

            Ok(())
        });

        Ok(Self { _run })
    }
}

impl MainSelectInteractiveService {
    pub async fn select_interactive(
        rx: &mut impl Receiver<TerminalSend>,
        tx: &mut impl Sender<MainRecv>,
    ) -> anyhow::Result<()> {
        // set fuzzy mode

        // await workspace tabs

        // send workspace tabs to fuzzy UI

        // await a response

        // send MainRecv::SelectTab

        // let running_tabs = Self::await_initialized(&mut rx_tabs_state).await;
        // let workspace_tabs = Self::await_workspace(&mut rx_workspace).await;
        // let tabs = Self::merge_tabs(running_tabs, workspace_tabs);
        // tx_fuzzy.send(FuzzyRecv { tabs }).await?;

        Ok(())
    }
}
