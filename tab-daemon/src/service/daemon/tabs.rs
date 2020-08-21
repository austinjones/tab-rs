use super::tab::TabService;
use crate::bus::TabBus;
use crate::{
    bus::DaemonBus,
    message::daemon::{CloseTab, CreateTab},
};
use std::{collections::HashMap, sync::atomic::AtomicUsize};
use tab_api::tab::{TabId, TabMetadata};
use tab_service::{Bus, Lifeline, Service};
use tokio::stream::StreamExt;

pub struct TabsService {
    _run: Lifeline,
}

enum TabEvent {
    Create(CreateTab),
    Close(CloseTab),
}

impl TabEvent {
    pub fn create(create: CreateTab) -> Self {
        TabEvent::Create(create)
    }

    pub fn close(close: CloseTab) -> Self {
        TabEvent::Close(close)
    }
}
impl Service for TabsService {
    type Bus = DaemonBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let rx_create = bus.rx::<CreateTab>()?;
        let rx_exit = bus.rx::<CloseTab>()?;
        let _run = Self::try_task("run", async move {
            let mut tabs: HashMap<TabId, TabMetadata> = HashMap::new();
            let mut lifelines: HashMap<TabId, TabService> = HashMap::new();

            let mut stream = rx_create
                .map(TabEvent::create)
                .merge(rx_exit.map(TabEvent::close));

            let tab_bus = TabBus::default();

            while let Some(msg) = stream.next().await {
                match msg {
                    TabEvent::Create(create) => {
                        if let Some(_) = tabs.values().find(|tab| create.0 == tab.name) {
                            continue;
                        }

                        let tab = TabService::spawn(&tab_bus)?;
                    }
                    TabEvent::Close(close) => {
                        tabs.remove(&close.0);
                        lifelines.remove(&close.0);
                    }
                }
            }

            Ok(())
        });

        Ok(Self { _run })
    }
}
