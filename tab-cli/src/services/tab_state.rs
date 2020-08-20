use crate::bus::client::ClientBus;
use crate::state::tab::{TabState, TabStateSelect};
use futures::select;
use log::{debug, info};
use tab_api::tab::{TabId, TabMetadata};
use tab_service::{Bus, Lifeline, Service};
use tokio::{
    stream::StreamExt,
    sync::{broadcast, mpsc, watch},
};
pub struct TabStateService {
    _lifeline: Lifeline,
}

pub struct TabStateRx {
    pub tab: watch::Receiver<TabStateSelect>,
    pub tab_metadata: broadcast::Receiver<TabMetadata>,
}

enum Event {
    Select(TabStateSelect),
    Metadata(TabMetadata),
}

impl Service for TabStateService {
    type Bus = ClientBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &ClientBus) -> Self::Lifeline {
        let rx_tab = bus.rx::<TabStateSelect>()?;
        let rx_tab_metadata = bus.rx::<TabMetadata>()?;
        let tx = bus.tx::<TabState>()?;

        let _lifeline = Self::task("run", async move {
            let mut state = TabState::None;

            let mut events = {
                let tabs = rx_tab.map(|elem| Event::Select(elem));
                let tab_metadatas = rx_tab_metadata.map(|elem| Event::Metadata(elem.unwrap()));
                tabs.merge(tab_metadatas)
            };

            while let Some(event) = events.next().await {
                match event {
                    Event::Select(select) => match select {
                        TabStateSelect::None => {}
                        TabStateSelect::Selected(name) => {
                            let name = name.as_str();

                            if state.is_selected_name(name) || state.is_awaiting(name) {
                                continue;
                            }

                            state = TabState::Awaiting(name.to_string());
                            tx.broadcast(state.clone()).expect("tab state broadcast");
                        }
                    },
                    Event::Metadata(metadata) => {
                        if state.is_awaiting(metadata.name.as_str()) {
                            state = TabState::Selected(TabId(metadata.id), metadata.name);
                            tx.broadcast(state.clone()).expect("tab state broadcast");
                        }
                    }
                }
            }
        });

        Ok(Self { _lifeline })
    }
}

// impl TabStateService {
//     pub fn find_tab<'a>(
//         tabs: &'a TabStateAvailable,
//         awaiting: &TabStateSelect,
//     ) -> Option<&'a TabMetadata> {
//         let tabs = &tabs.0;
//         let awaiting = &awaiting.0;

//         if !awaiting.is_some() {
//             return None;
//         }

//         let awaiting = awaiting.as_ref().unwrap();

//         tabs.iter().find(|metadata| &metadata.name == awaiting)
//     }
// }
