use std::time::Duration;

use tab_api::tab::normalize_name;
use tokio::time;

use crate::{
    message::main::MainRecv, message::main::MainShutdown, prelude::*, state::tabs::ActiveTabsState,
    utils::await_state,
};

pub struct MainCloseTabsService {
    _run: Lifeline,
}

impl Service for MainCloseTabsService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut rx = bus.rx::<MainRecv>()?;
        let mut rx_active = bus.rx::<Option<ActiveTabsState>>()?;

        let mut tx_request = bus.tx::<Request>()?;
        let mut tx_shutdown = bus.tx::<MainShutdown>()?;

        let _run = Self::try_task("run", async move {
            while let Some(msg) = rx.recv().await {
                if let MainRecv::CloseTabs(tabs) = msg {
                    let state = await_state(&mut rx_active).await?;
                    let exit_code = Self::close_tabs(tabs, state, &mut tx_request).await?;

                    time::sleep(Duration::from_millis(5)).await;
                    tx_shutdown.send(MainShutdown(exit_code)).await?;
                    break;
                }
            }

            Ok(())
        });

        Ok(Self { _run })
    }
}

impl MainCloseTabsService {
    async fn close_tabs(
        tabs: Vec<String>,
        state: ActiveTabsState,
        mut tx_websocket: impl Sink<Item = Request> + Unpin,
    ) -> anyhow::Result<i32> {
        if tabs.is_empty() {
            if let Ok(tab) = std::env::var("TAB_ID") {
                eprintln!(
                    "Closing current tab: {}",
                    std::env::var("TAB").unwrap_or_else(|_| "".to_string())
                );

                let id = tab.parse()?;
                tx_websocket.send(Request::CloseTab(id)).await?;
            } else {
                eprintln!("No arguments or current tab were detected.");
                return Ok(1);
            }

            return Ok(0);
        }

        let running_tabs = state.as_name_set();

        for tab in tabs {
            let name = normalize_name(tab.as_str());

            if !running_tabs.contains(&name) {
                eprintln!("Tab not running: {}", name);
                continue;
            }

            eprintln!("Closing tab: {}", name);

            let tab = state.find_name(name.as_str()).unwrap();
            tx_websocket.send(Request::CloseTab(tab.id)).await?;
        }

        Ok(0)
    }
}
