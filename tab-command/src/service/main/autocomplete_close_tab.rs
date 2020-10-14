use crate::{
    message::main::MainRecv, message::main::MainShutdown, prelude::*, state::tabs::ActiveTabsState,
    utils::await_state,
};

pub struct MainAutocompleteCloseTabsService {
    _run: Lifeline,
}

impl Service for MainAutocompleteCloseTabsService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut rx = bus.rx::<MainRecv>()?;
        let mut rx_active = bus.rx::<Option<ActiveTabsState>>()?.into_inner();

        let mut tx_shutdown = bus.tx::<MainShutdown>()?;

        let _run = Self::try_task("run", async move {
            while let Some(msg) = rx.recv().await {
                if let MainRecv::AutocompleteCloseTab = msg {
                    let active_tabs = await_state(&mut rx_active).await?;

                    let tabs: Vec<String> =
                        active_tabs.tabs.into_iter().map(|tab| tab.1.name).collect();
                    Self::echo_completion(&tabs);

                    tx_shutdown.send(MainShutdown {}).await.ok();
                }
            }

            Ok(())
        });

        Ok(Self { _run })
    }
}

impl MainAutocompleteCloseTabsService {
    fn echo_completion(tabs: &Vec<String>) {
        debug!("echo completion: {:?}", tabs);

        for tab in tabs {
            println!("{}", tab);
        }
    }
}
