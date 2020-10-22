use crate::{
    message::main::MainRecv, message::main::MainShutdown, prelude::*,
    state::workspace::WorkspaceState, utils::await_state,
};

pub struct MainAutocompleteTabsService {
    _run: Lifeline,
}

impl Service for MainAutocompleteTabsService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut rx = bus.rx::<MainRecv>()?;
        let mut rx_workspace = bus.rx::<Option<WorkspaceState>>()?.into_inner();

        let mut tx_shutdown = bus.tx::<MainShutdown>()?;

        let _run = Self::try_task("run", async move {
            while let Some(msg) = rx.recv().await {
                if let MainRecv::AutocompleteTab = msg {
                    let workspace = await_state(&mut rx_workspace).await?;

                    let tabs = workspace.tabs.iter().map(|tab| &tab.name).collect();
                    Self::echo_completion(tabs);

                    tx_shutdown.send(MainShutdown {}).await.ok();
                }
            }

            Ok(())
        });

        Ok(Self { _run })
    }
}

impl MainAutocompleteTabsService {
    fn echo_completion(tabs: Vec<&String>) {
        debug!("echo completion: {:?}", tabs);

        for tab in tabs {
            println!("{}", tab);
        }
    }
}
