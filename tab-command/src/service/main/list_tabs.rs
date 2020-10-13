use crate::{
    message::main::MainRecv, message::main::MainShutdown, prelude::*, state::tabs::ActiveTabsState,
    state::workspace::WorkspaceState, state::workspace::WorkspaceTab, utils::await_state,
};

pub struct MainListTabsService {
    _run: Lifeline,
}

impl Service for MainListTabsService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut rx = bus.rx::<MainRecv>()?;
        let mut rx_active = bus.rx::<Option<ActiveTabsState>>()?.into_inner();
        let mut rx_workspace = bus.rx::<Option<WorkspaceState>>()?.into_inner();

        let mut tx_shutdown = bus.tx::<MainShutdown>()?;

        let _run = Self::try_task("run", async move {
            while let Some(msg) = rx.recv().await {
                if let MainRecv::ListTabs = msg {
                    let active_tabs = await_state(&mut rx_active).await?;
                    let workspace = await_state(&mut rx_workspace)
                        .await?
                        .with_active_tabs(&active_tabs);

                    Self::echo_tabs(&workspace.tabs);
                    tx_shutdown.send(MainShutdown {}).await.ok();
                    break;
                }
            }

            Ok(())
        });

        Ok(Self { _run })
    }
}

impl MainListTabsService {
    fn echo_tabs(tabs: &Vec<WorkspaceTab>) {
        debug!("echo tabs: {:?}", &tabs);

        if tabs.len() == 0 {
            println!("No active tabs.");
            return;
        }

        let len = tabs.iter().map(|tab| tab.name.len()).max().unwrap();
        let target_len = len + 4;

        println!("Available tabs:");

        for tab in tabs.iter() {
            let name = &tab.name;
            print!("    {}", name);

            if let Some(ref doc) = tab.doc {
                for _ in name.len()..target_len {
                    print!(" ");
                }
                println!("({})", doc);
            } else {
                println!("");
            }
        }
    }
}
