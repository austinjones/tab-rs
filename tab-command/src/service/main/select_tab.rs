use tab_api::tab::{normalize_name, TabId};

use crate::message::tabs::TabRecv;
use crate::{
    message::main::MainRecv, message::terminal::TerminalRecv, prelude::*,
    state::terminal::TerminalMode,
};

pub fn env_tab_id() -> Option<TabId> {
    if let Ok(id) = std::env::var("TAB_ID") {
        if let Ok(id) = id.parse() {
            return Some(TabId(id));
        }
    }

    None
}

pub struct MainSelectTabService {
    _run: Lifeline,
}

impl Service for MainSelectTabService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut rx = bus.rx::<MainRecv>()?;

        let mut tx_tab = bus.tx::<TabRecv>()?;
        let mut tx_terminal = bus.tx::<TerminalRecv>()?;

        let _run = Self::try_task("run", async move {
            while let Some(recv) = rx.recv().await {
                if let MainRecv::SelectTab(name) = recv {
                    Self::select_tab(name, &mut tx_tab, &mut tx_terminal).await?;
                }
            }

            Ok(())
        });

        Ok(Self { _run })
    }
}

impl MainSelectTabService {
    async fn select_tab(
        name: String,
        tx_tab: &mut impl Sender<TabRecv>,
        tx_terminal: &mut impl Sender<TerminalRecv>,
    ) -> anyhow::Result<()> {
        info!("MainRecv::SelectTab({}) running", &name);
        let name = normalize_name(name.as_str());
        let env_tab = env_tab_id();

        info!("selecting tab: {}", name);

        tx_terminal
            .send(TerminalRecv::Mode(TerminalMode::Echo(name.clone())))
            .await?;

        let message = TabRecv::SelectNamedTab { name, env_tab };
        tx_tab.send(message).await?;

        Ok(())
    }
}
